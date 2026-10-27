//! Explicit native registration/start. No complete POST, token custody, IPC or
//! background retry. A durable fingerprint precedes every POST; the invite does not.
use super::*;
use reqwest::{Client, Url};

const RESPONSE_LIMIT: usize = 16384;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    InvalidConfiguration,
    Registry(RegistryError),
    Identity(IdentityError),
    Transport,
    Timeout,
    RedirectDenied,
    Http(u16),
    RateLimited { retry_after_seconds: Option<u64> },
    ResponseTooLarge,
    InvalidResponse,
}
impl From<RegistryError> for StartError {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}
impl From<IdentityError> for StartError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
impl From<sqlx::Error> for StartError {
    fn from(_: sqlx::Error) -> Self {
        Self::Registry(RegistryError::Storage)
    }
}
impl StartError {
    fn safe_code(self) -> &'static str {
        match self {
            Self::Transport => "transport",
            Self::Timeout => "timeout",
            Self::RedirectDenied => "redirect_denied",
            Self::Http(_) => "http",
            Self::RateLimited { .. } => "rate_limited",
            Self::ResponseTooLarge => "response_too_large",
            Self::InvalidResponse => "invalid_response",
            Self::Identity(_) => "invalid_context",
            Self::Registry(_) => "storage",
            Self::InvalidConfiguration => "configuration",
        }
    }
}
/// Public context only; a challenge is not registration success or a bearer token.
#[derive(Debug, Clone)]
pub enum StartOutcome {
    Challenge(ValidatedRegistration),
    RecoveryRequired { registration_id: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProgress {
    pub attempts: i64,
    pub has_context: bool,
    /// Fixed local code, never a server message, URL, request hash or response body.
    pub last_failure: Option<String>,
}

#[derive(Clone)]
pub struct RegistrationStart {
    client: Client,
    timeout: Duration,
    #[cfg(test)]
    loopback: Option<Url>,
}
impl RegistrationStart {
    pub fn new(timeout: Duration) -> Result<Self, StartError> {
        if timeout < Duration::from_millis(1) || timeout > Duration::from_secs(30) {
            return Err(StartError::InvalidConfiguration);
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .referer(false)
            .connect_timeout(timeout)
            .build()
            .map_err(|_| StartError::InvalidConfiguration)?;
        Ok(Self {
            client,
            timeout,
            #[cfg(test)]
            loopback: None,
        })
    }

    /// Explicit call only. After an ambiguous result the caller may resubmit the
    /// SAME invite, using the durable original ID/keys. There is no automatic POST.
    pub async fn start(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        invite: &str,
    ) -> Result<StartOutcome, StartError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation);
        let keys = &guarded_keys;
        let intent = registry.load_intent(keys, scope).await?;
        let request = intent.start_request(invite)?;
        let fingerprint = super::super::crypto::digest(request.as_bytes());
        let (attempt, saved) = begin(registry, scope, &fingerprint).await?;
        if let Some(saved) = saved {
            return validate(&intent, &saved, Utc::now());
        }
        let result = self.post(scope, &request).await.and_then(|response| {
            let outcome = validate(&intent, &response, Utc::now())?;
            Ok((outcome, project(&response)))
        });
        match result {
            Ok((outcome, response)) => {
                // A late response cannot overwrite the result of a newer attempt.
                let n = sqlx::query("UPDATE community_registration_starts SET response=?,last_failure=NULL WHERE scope_binding=? AND attempts=? AND response IS NULL")
                    .bind(response.to_string()).bind(scope.binding()).bind(attempt)
                    .execute(&registry.pool).await?.rows_affected();
                if n != 1 {
                    return Err(RegistryError::Conflict.into());
                }
                Ok(outcome)
            }
            Err(error) => {
                sqlx::query("UPDATE community_registration_starts SET last_failure=? WHERE scope_binding=? AND attempts=? AND response IS NULL")
                    .bind(error.safe_code()).bind(scope.binding()).bind(attempt)
                    .execute(&registry.pool).await?;
                Err(error)
            }
        }
    }

    /// No network or generation. Revalidates the saved canonical context against
    /// the original profile/scope, current vault keys and current time after restart.
    pub async fn cached(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<Option<StartOutcome>, StartError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation);
        let keys = &guarded_keys;
        let intent = registry.load_intent(keys, scope).await?;
        let saved: Option<Option<String>> = sqlx::query_scalar(
            "SELECT response FROM community_registration_starts WHERE scope_binding=?",
        )
        .bind(scope.binding())
        .fetch_optional(&registry.pool)
        .await?;
        match saved.flatten() {
            None => Ok(None),
            Some(raw) => {
                let value = parse_saved(&raw)?;
                Ok(Some(validate(&intent, &value, Utc::now())?))
            }
        }
    }

    pub async fn progress(
        &self,
        registry: &RegistrationRegistry,
        scope: &CommunityScope,
    ) -> Result<Option<StartProgress>, StartError> {
        // Enforce scope integrity before exposing its coordination record.
        registry.get(scope).await?;
        let row = sqlx::query("SELECT attempts,response,last_failure FROM community_registration_starts WHERE scope_binding=?")
            .bind(scope.binding()).fetch_optional(&registry.pool).await?;
        row.map(|row| {
            let attempts: i64 = row.try_get("attempts")?;
            let response: Option<String> = row.try_get("response")?;
            let failure: Option<String> = row.try_get("last_failure")?;
            if attempts <= 0
                || failure.as_deref().is_some_and(|s| {
                    ![
                        "transport",
                        "timeout",
                        "redirect_denied",
                        "http",
                        "rate_limited",
                        "response_too_large",
                        "invalid_response",
                        "invalid_context",
                        "storage",
                        "configuration",
                    ]
                    .contains(&s)
                })
            {
                return Err(RegistryError::Corrupt.into());
            }
            // This is a presence indicator, not cryptographic or expiry validation.
            Ok(StartProgress {
                attempts,
                has_context: response.is_some(),
                last_failure: failure,
            })
        })
        .transpose()
    }

    async fn post(
        &self,
        scope: &CommunityScope,
        request: &StartRequest,
    ) -> Result<Value, StartError> {
        self.exchange(
            scope,
            reqwest::Method::POST,
            "registration/start/",
            Some(request.as_bytes()),
            &[],
        )
        .await
    }
    #[cfg(test)]
    pub(super) fn set_loopback(&mut self, url: Url) {
        self.loopback = Some(url);
    }
    pub(super) async fn exchange(
        &self,
        scope: &CommunityScope,
        method: reqwest::Method,
        path: &str,
        body: Option<&[u8]>,
        headers: &[(&str, &str)],
    ) -> Result<Value, StartError> {
        let root = Url::parse(&scope.api_root).map_err(|_| StartError::InvalidConfiguration)?;
        let url = root
            .join(path)
            .map_err(|_| StartError::InvalidConfiguration)?;
        if url.origin() != root.origin()
            || !url.path().starts_with(root.path())
            || body.is_some_and(|b| b.len() > 8192)
        {
            return Err(StartError::InvalidConfiguration);
        }
        #[cfg(test)]
        let url = if let Some(mut local) = self.loopback.clone() {
            local.set_path(url.path());
            local.set_query(url.query());
            local
        } else {
            url
        };
        tokio::time::timeout(self.timeout, async {
            let mut builder = self
                .client
                .request(method, url)
                .header("Accept", "application/json");
            if let Some(body) = body {
                builder = builder
                    .header("Content-Type", "application/json")
                    .body(body.to_vec());
            }
            for (name, value) in headers {
                builder = builder.header(*name, *value);
            }
            let mut response = builder.send().await.map_err(transport_error)?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                return Err(StartError::RedirectDenied);
            }
            if status == 429 {
                return Err(StartError::RateLimited {
                    retry_after_seconds: response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .filter(|n| *n <= 86400),
                });
            }
            if status != 200 {
                return Err(StartError::Http(status));
            }
            if response
                .content_length()
                .is_some_and(|n| n > RESPONSE_LIMIT as u64)
            {
                return Err(StartError::ResponseTooLarge);
            }
            let mime = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !mime
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
            {
                return Err(StartError::InvalidResponse);
            }
            let mut bytes = Zeroizing::new(Vec::new());
            while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
                if chunk.len() > RESPONSE_LIMIT - bytes.len() {
                    return Err(StartError::ResponseTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|_| StartError::InvalidResponse)
        })
        .await
        .map_err(|_| StartError::Timeout)?
    }
}
fn transport_error(e: reqwest::Error) -> StartError {
    // reqwest error strings can contain the URL. Never return them across the boundary.
    if e.is_timeout() {
        StartError::Timeout
    } else {
        StartError::Transport
    }
}
fn validate(
    intent: &RegistrationIntent,
    response: &Value,
    now: DateTime<Utc>,
) -> Result<StartOutcome, StartError> {
    // In particular committed responses must pass ALL canonical bindings too.
    let context = intent.validate_context(response, now, true)?;
    if response["registration"]["state"] == "committed" {
        Ok(StartOutcome::RecoveryRequired {
            registration_id: context.registration_id,
        })
    } else {
        Ok(StartOutcome::Challenge(context))
    }
}
fn project(response: &Value) -> Value {
    let mut r = serde_json::Map::new();
    for key in [
        "registration_id",
        "state",
        "nonce",
        "expires_at",
        "community_origin",
        "community_instance",
        "profile_format",
        "profile_sha256",
        "signature_algorithm",
        "public_key_format",
        "signature_format",
        "protocol_version",
        "statement",
        "retry",
    ] {
        r.insert(key.into(), response["registration"][key].clone());
    }
    // Discard unknown fields and arbitrary server request IDs, including any echo
    // of an invite, error text, token or attacker-provided extra data.
    json!({"ok":true,"request_id":"stored-start-context","registration":r})
}
fn parse_saved(raw: &str) -> Result<Value, StartError> {
    if raw.len() > RESPONSE_LIMIT {
        return Err(RegistryError::Corrupt.into());
    }
    serde_json::from_str(raw).map_err(|_| RegistryError::Corrupt.into())
}
async fn begin(
    registry: &RegistrationRegistry,
    scope: &CommunityScope,
    fingerprint: &str,
) -> Result<(i64, Option<Value>), StartError> {
    // First statement obtains SQLite write ownership before reading. Simultaneous
    // explicit calls may resend an idempotent start, never different request bytes.
    let mut tx = registry.pool.begin().await?;
    let inserted=sqlx::query("INSERT INTO community_registration_starts(scope_binding,request_sha256,attempts) VALUES(?,?,1) ON CONFLICT DO NOTHING")
        .bind(scope.binding()).bind(fingerprint).execute(&mut *tx).await?.rows_affected();
    let row=sqlx::query("SELECT request_sha256,attempts,response FROM community_registration_starts WHERE scope_binding=?")
        .bind(scope.binding()).fetch_one(&mut *tx).await?;
    let stored: String = row.try_get("request_sha256")?;
    if stored != fingerprint {
        return Err(RegistryError::Conflict.into());
    }
    let attempts: i64 = row.try_get("attempts")?;
    let response: Option<String> = row.try_get("response")?;
    if attempts <= 0 {
        return Err(RegistryError::Corrupt.into());
    }
    if let Some(raw) = response {
        let parsed = parse_saved(&raw)?;
        tx.commit().await?;
        return Ok((attempts, Some(parsed)));
    }
    let next = if inserted == 1 {
        1
    } else {
        attempts.checked_add(1).ok_or(RegistryError::Corrupt)?
    };
    sqlx::query("UPDATE community_registration_starts SET attempts=?,last_failure=NULL WHERE scope_binding=?")
        .bind(next).bind(scope.binding()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok((next, None))
}

#[cfg(test)]
mod tests;
