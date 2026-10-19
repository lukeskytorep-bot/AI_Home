// Adapted from AI RV Harness (MIT) adapters.rs and transport.rs.
// See docs/reuse/HARNESS_PROVIDER_LICENSE.txt. AI Home adds bounded bodies,
// shared call deadlines, sticky cancellation and no implicit retries/redirects.
use super::{ProtocolError, ProviderChatRequest, ProviderChatResponse, ProviderKind, ScopeStamp};
use reqwest::{
    header::{HeaderValue, AUTHORIZATION},
    Client, RequestBuilder,
};
use serde_json::Value;
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, SystemTime},
};
use tokio::{
    sync::watch,
    time::{timeout_at, Instant},
};

const MAX_BODY: usize = 8 * 1024 * 1024;
const MAX_REQUEST: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Storage,
    Configuration,
    Protocol(ProtocolError),
    CredentialMismatch,
    Cancelled,
    Timeout,
    CallLimit,
    ResponseTooLarge,
    RequestTooLarge,
    Network,
    BodyRead,
    InvalidJson,
    HttpStatus {
        status: u16,
        retry_after_ms: Option<u64>,
    },
    InvalidModelList,
    DiscoveryIncomplete,
}
impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never interpolate URLs, headers, provider body or reqwest error text.
        f.write_str(match self {
            Self::Storage => "storage_error",
            Self::Configuration => "provider_configuration",
            Self::Protocol(_) => "provider_protocol",
            Self::CredentialMismatch => "credential_binding_mismatch",
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::CallLimit => "physical_call_limit",
            Self::ResponseTooLarge => "provider_response_too_large",
            Self::RequestTooLarge => "provider_request_too_large",
            Self::Network => "provider_network",
            Self::BodyRead => "provider_body_read",
            Self::InvalidJson => "invalid_provider_json",
            Self::HttpStatus { .. } => "provider_http_status",
            Self::InvalidModelList => "invalid_model_list",
            Self::DiscoveryIncomplete => "model_discovery_incomplete",
        })
    }
}
impl std::error::Error for TransportError {}

/// Native-only credential. No Debug, Serialize or secret getter; OS vault comes next.
pub struct BoundCredential {
    secret: String,
    profile_id: String,
    connection_id: String,
    provider: ProviderKind,
    endpoint: String,
    revision: u64,
}
impl BoundCredential {
    pub fn new(scope: &ScopeStamp, secret: String) -> Result<Self, TransportError> {
        let endpoint = super::normalize_endpoint(scope.provider, &scope.normalized_endpoint)
            .map_err(TransportError::Protocol)?;
        if endpoint != scope.normalized_endpoint
            || scope.credential_revision == 0
            || [&scope.profile_id, &scope.connection_id]
                .iter()
                .any(|id| id.is_empty() || id.len() > 128)
            || secret.is_empty()
            || secret.len() > 8192
            || !secret.bytes().all(|b| (33..=126).contains(&b))
        {
            return Err(TransportError::Configuration);
        }
        Ok(Self {
            secret,
            profile_id: scope.profile_id.clone(),
            connection_id: scope.connection_id.clone(),
            provider: scope.provider,
            endpoint,
            revision: scope.credential_revision,
        })
    }
    fn matches(&self, scope: &ScopeStamp) -> bool {
        self.profile_id == scope.profile_id
            && self.connection_id == scope.connection_id
            && self.provider == scope.provider
            && self.endpoint == scope.normalized_endpoint
            && self.revision == scope.credential_revision
    }
}

#[derive(Clone)]
pub struct Cancellation {
    sender: Arc<watch::Sender<bool>>,
}
impl Default for Cancellation {
    fn default() -> Self {
        let (sender, _) = watch::channel(false);
        Self {
            sender: Arc::new(sender),
        }
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.sender.send_replace(true);
    }
    async fn cancelled(&self) {
        let mut receiver = self.sender.subscribe();
        loop {
            if *receiver.borrow_and_update() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}
/// Shared by all physical calls of one operation, including discovery pages.
/// Errors, retries and later recovery must use this same budget, never a reset.
/// The application may commit a reservation before any HTTP bytes are sent.
/// Providers stay independent of SQLite and application operation DTOs.
pub trait CallRecorder: Send + Sync {
    fn reserved(
        &self,
        count: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>;
}
#[derive(Clone)]
pub struct CallBudget {
    deadline: Instant,
    maximum: u8,
    used: Arc<AtomicU8>,
    cancellation: Cancellation,
    recorder: Option<Arc<dyn CallRecorder>>,
}
impl CallBudget {
    pub fn new(
        duration: Duration,
        maximum: u8,
        cancellation: Cancellation,
    ) -> Result<Self, TransportError> {
        if duration.is_zero() || duration > Duration::from_secs(180) || !(1..=4).contains(&maximum)
        {
            return Err(TransportError::Configuration);
        }
        Ok(Self {
            deadline: Instant::now() + duration,
            maximum,
            used: Arc::new(AtomicU8::new(0)),
            cancellation,
            recorder: None,
        })
    }
    pub fn standard(cancellation: Cancellation) -> Self {
        Self::new(Duration::from_secs(180), 4, cancellation).expect("constant limits")
    }
    pub fn with_recorder(mut self, recorder: Arc<dyn CallRecorder>) -> Self {
        self.recorder = Some(recorder);
        self
    }
    pub(crate) fn deadline(&self) -> Instant { self.deadline }
    pub(crate) async fn cancelled(&self) { self.cancellation.cancelled().await }
    pub fn physical_calls(&self) -> u8 {
        self.used.load(Ordering::SeqCst)
    }
    pub(crate) fn ensure_active(&self) -> Result<(), TransportError> {
        if *self.cancellation.sender.borrow() {
            return Err(TransportError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(TransportError::Timeout);
        }
        Ok(())
    }
    pub(super) async fn wait_for_retry(&self, delay: Duration) -> Result<(), TransportError> {
        self.ensure_active()?;
        if self.physical_calls() >= self.maximum {
            return Err(TransportError::CallLimit);
        }
        let wake = Instant::now()
            .checked_add(delay)
            .ok_or(TransportError::Timeout)?;
        if wake >= self.deadline {
            return Err(TransportError::Timeout);
        }
        tokio::select! {biased;
            _=self.cancellation.cancelled()=>Err(TransportError::Cancelled),
            _=tokio::time::sleep_until(wake)=>self.ensure_active(),
        }
    }
    fn reserve(&self) -> Result<(), TransportError> {
        self.ensure_active()?;
        let mut used = self.used.load(Ordering::SeqCst);
        loop {
            if used >= self.maximum {
                return Err(TransportError::CallLimit);
            }
            match self.used.compare_exchange_weak(
                used,
                used + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(()),
                Err(current) => used = current,
            }
        }
    }
}

pub struct HttpTransport {
    client: Client,
    body_limit: usize,
}
impl HttpTransport {
    pub fn new() -> Result<Self, TransportError> {
        Self::with_body_limit(MAX_BODY)
    }
    fn with_body_limit(body_limit: usize) -> Result<Self, TransportError> {
        if body_limit == 0 || body_limit > MAX_BODY {
            return Err(TransportError::Configuration);
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .user_agent(concat!("AI-Home/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| TransportError::Configuration)?;
        Ok(Self { client, body_limit })
    }
    pub async fn execute_chat(
        &self,
        request: &ProviderChatRequest,
        credential: &BoundCredential,
        budget: &CallBudget,
    ) -> Result<ProviderChatResponse, TransportError> {
        super::executor::run(budget, || self.chat(request, credential, budget)).await
    }
    pub(super) async fn chat(
        &self,
        request: &ProviderChatRequest,
        credential: &BoundCredential,
        budget: &CallBudget,
    ) -> Result<ProviderChatResponse, TransportError> {
        // Route/provider/endpoint and replay checks precede credential use and dispatch.
        let prepared = super::prepare_chat(request, &request.scope.normalized_endpoint)
            .map_err(TransportError::Protocol)?;
        if !credential.matches(&request.scope) {
            return Err(TransportError::CredentialMismatch);
        }
        let bytes =
            serde_json::to_vec(&prepared.body).map_err(|_| TransportError::Configuration)?;
        if bytes.len() > MAX_REQUEST {
            return Err(TransportError::RequestTooLarge);
        }
        let builder = authenticated(
            self.client
                .post(prepared.endpoint)
                .header("content-type", "application/json")
                .body(bytes),
            request.provider,
            &credential.secret,
        )?;
        let payload = self.send(builder, budget).await?;
        let parsed =
            super::parse_response(request.provider, payload).map_err(TransportError::Protocol)?;
        budget.ensure_active()?;
        Ok(parsed)
    }
    pub(super) async fn execute_model_page(
        &self,
        scope: &ScopeStamp,
        credential: &BoundCredential,
        query: &[(String, String)],
        budget: &CallBudget,
    ) -> Result<Value, TransportError> {
        super::executor::run(budget, || self.model_page(scope, credential, query, budget)).await
    }
    pub(super) async fn model_page(
        &self,
        scope: &ScopeStamp,
        credential: &BoundCredential,
        query: &[(String, String)],
        budget: &CallBudget,
    ) -> Result<Value, TransportError> {
        let base = super::normalize_endpoint(scope.provider, &scope.normalized_endpoint)
            .map_err(TransportError::Protocol)?;
        if base != scope.normalized_endpoint || !credential.matches(scope) {
            return Err(TransportError::CredentialMismatch);
        }
        let mut url = reqwest::Url::parse(&super::adapters::endpoint(&base, "models"))
            .map_err(|_| TransportError::Configuration)?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (k, v)));
        }
        let builder = authenticated(self.client.get(url), scope.provider, &credential.secret)?;
        self.send(builder, budget).await
    }
    async fn send(
        &self,
        builder: RequestBuilder,
        budget: &CallBudget,
    ) -> Result<Value, TransportError> {
        // Construct/validate the request before charging the physical-call budget.
        let request = builder.build().map_err(|_| TransportError::Configuration)?;
        budget.reserve()?;
        tokio::select! {
            biased;
            _ = budget.cancellation.cancelled() => Err(TransportError::Cancelled),
            result = timeout_at(budget.deadline, async {
                if let Some(recorder) = &budget.recorder { recorder.reserved(budget.physical_calls()).await?; }
                let mut response = self.client.execute(request).await.map_err(network_error)?;
                let status = response.status();
                if !status.is_success() {
                    // Error body is never read, exposed or logged (may contain secrets).
                    let retry_after_ms = response.headers().get("retry-after")
                        .and_then(|v| v.to_str().ok()).and_then(|v| retry_after_ms_at(v, SystemTime::now()));
                    return Err(TransportError::HttpStatus { status: status.as_u16(), retry_after_ms });
                }
                if response.content_length().is_some_and(|length| length > self.body_limit as u64) {
                    return Err(TransportError::ResponseTooLarge);
                }
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await.map_err(|_| TransportError::BodyRead)? {
                    if chunk.len() > self.body_limit - body.len() { return Err(TransportError::ResponseTooLarge); }
                    body.extend_from_slice(&chunk);
                }
                serde_json::from_slice(&body).map_err(|_| TransportError::InvalidJson)
            }) => result.map_err(|_| TransportError::Timeout)?,
        }
    }
}
fn network_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else {
        TransportError::Network
    }
}
fn authenticated(
    builder: RequestBuilder,
    provider: ProviderKind,
    secret: &str,
) -> Result<RequestBuilder, TransportError> {
    let (name, value) = match provider {
        ProviderKind::Google => ("x-goog-api-key", secret.to_string()),
        ProviderKind::Anthropic => ("x-api-key", secret.to_string()),
        _ => (AUTHORIZATION.as_str(), format!("Bearer {secret}")),
    };
    let mut header = HeaderValue::from_str(&value).map_err(|_| TransportError::Configuration)?;
    header.set_sensitive(true);
    let builder = builder.header(name, header);
    Ok(match provider {
        ProviderKind::Anthropic => builder.header("anthropic-version", "2023-06-01"),
        // Do not attribute AI Home to the Harness product/page.
        ProviderKind::Openrouter => builder.header("X-OpenRouter-Title", "AI Home"),
        _ => builder,
    })
}
/// Harness policy: seconds or IMF-fixdate, capped at 30 s; metadata only,
/// this transport never automatically replays a paid chat call.
fn retry_after_ms_at(value: &str, now: SystemTime) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.saturating_mul(1000).min(30_000));
    }
    let date = chrono::NaiveDateTime::parse_from_str(value, "%a, %d %b %Y %H:%M:%S GMT")
        .ok()?
        .and_utc();
    let target = std::time::UNIX_EPOCH
        .checked_add(Duration::from_secs(date.timestamp().try_into().ok()?))?;
    Some(
        target
            .duration_since(now)
            .unwrap_or_default()
            .as_millis()
            .min(30_000) as u64,
    )
}
#[cfg(test)]
mod tests;
