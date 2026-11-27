//! Isolated MCP client port, policy guard, and Streamable HTTP JSON-RPC 2.0 prototype (4B-12/13).
//!
//! Architectural invariants:
//! - Isolated behind `McpClientPort` (no SDK types leak to UI, IPC, or `kernel.rs`).
//! - Per-`(profile_id, service_id, endpoint)` `Vault` credential scoping via `ScopeStamp`.
//! - HTTP redirects are disabled (`reqwest::redirect::Policy::none()`) so credentials never leak
//!   to foreign hosts.
//! - Tool descriptors are untrusted external metadata: no self-grant from tool descriptions/schemas;
//!   native per-profile allowlist + `read_only` / `allow_mutating` policy is enforced before any call.
//! - Private memory/reflection sources (`experimental_private_reflections`, `experimental_self_records`,
//!   `private_scratchpad`) are blocked from automatic publication.
//! - Ambiguous/disconnected mutating tool calls return `McpToolCallOutcome::Uncertain` (mapped to
//!   `ActionOutcome::Uncertain` -> `recovery_required` without blind retry).

use crate::domain::schema;
use crate::experimental_runtime::ActionOutcome;
use crate::providers::{ProviderKind, ScopeStamp};
use crate::vault::Vault;
use chrono::Utc;
use reqwest::redirect::Policy as RedirectPolicy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpError {
    InvalidEndpoint(String),
    PolicyDenied(String),
    CredentialUnavailable,
    RedirectBlocked,
    Transport(String),
    Protocol(String),
}

#[derive(Clone, Debug)]
pub struct McpServerBinding {
    pub profile_id: String,
    pub service_id: String,
    pub endpoint: String,
    pub credential_ref: Option<String>,
    pub credential_revision: u64,
    pub timeout_ms: u64,
}

impl McpServerBinding {
    /// Validates and canonicalizes endpoint URL (aligned with `providers::normalize_endpoint`):
    /// HTTPS only (or explicit loopback HTTP), no userinfo, no control characters,
    /// no query/fragment, no port 0, trailing slash trimmed for stable Vault `ScopeStamp`.
    pub fn normalized_endpoint(&self) -> Result<String, McpError> {
        if self.endpoint.chars().any(|c| c.is_control()) {
            return Err(McpError::InvalidEndpoint("control characters not allowed".into()));
        }
        let trimmed = self.endpoint.trim();
        let parsed = reqwest::Url::parse(trimmed)
            .map_err(|_| McpError::InvalidEndpoint("malformed URL".into()))?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(McpError::InvalidEndpoint("userinfo not allowed".into()));
        }
        if trimmed.ends_with('?')
            || trimmed.ends_with('#')
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(McpError::InvalidEndpoint("query/fragment not allowed".into()));
        }
        if parsed.port() == Some(0) {
            return Err(McpError::InvalidEndpoint("port 0 not allowed".into()));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| McpError::InvalidEndpoint("missing host".into()))?;
        let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
        let canonical = parsed.to_string().trim_end_matches('/').to_string();
        match parsed.scheme() {
            "https" => Ok(canonical),
            "http" if loopback => Ok(canonical),
            _ => Err(McpError::InvalidEndpoint("only https or loopback http allowed".into())),
        }
    }

    /// Builds a strict Vault `ScopeStamp` isolated per `(profile_id, service_id, normalized_endpoint)`.
    pub fn vault_scope(&self) -> Result<ScopeStamp, McpError> {
        let normalized = self.normalized_endpoint()?;
        if self.profile_id.trim().is_empty() || self.service_id.trim().is_empty() {
            return Err(McpError::PolicyDenied("missing profile or service scope".into()));
        }
        Ok(ScopeStamp {
            profile_id: self.profile_id.clone(),
            connection_id: format!("mcp-service:{}", self.service_id),
            route_id: format!("mcp-route:{}", self.service_id),
            route_revision: 1,
            credential_revision: self.credential_revision,
            model_id: format!("mcp:{}", self.service_id),
            provider: ProviderKind::CustomOpenai,
            normalized_endpoint: normalized,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpServerCapabilities {
    pub protocol_version: String,
    pub server_name: String,
    pub supports_tools: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    /// Always true: external MCP tool metadata is untrusted and can never grant permissions.
    pub untrusted_external_metadata: bool,
    pub declared_mutating: bool,
}

#[derive(Clone, Debug, Default)]
pub struct McpToolPolicy {
    pub profile_id: String,
    pub service_id: String,
    pub allowed_tools: HashSet<String>,
    pub allow_mutating: bool,
}

impl McpToolPolicy {
    pub fn new(profile_id: &str, service_id: &str, allow_mutating: bool) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            service_id: service_id.to_string(),
            allowed_tools: HashSet::new(),
            allow_mutating,
        }
    }

    pub fn allow_tool(mut self, name: &str) -> Self {
        self.allowed_tools.insert(name.to_string());
        self
    }

    /// Rejects any self-grant metadata attempts and verifies native per-profile/per-service allowlist.
    pub fn authorize_call(&self, req: &McpToolCallRequest) -> Result<(), McpError> {
        if req.server.profile_id != self.profile_id || req.server.service_id != self.service_id {
            return Err(McpError::PolicyDenied("profile/service policy scope mismatch".into()));
        }
        if !self.allowed_tools.contains(&req.tool_name) {
            return Err(McpError::PolicyDenied(format!(
                "tool '{}' is not on native allowlist",
                req.tool_name
            )));
        }
        if req.mutating && !self.allow_mutating {
            return Err(McpError::PolicyDenied(format!(
                "mutating tool '{}' denied under read-only policy",
                req.tool_name
            )));
        }
        // Private reflection / scratchpad sources may never be published automatically via MCP.
        for source in &req.source_tags {
            if is_private_source_tag(source) {
                return Err(McpError::PolicyDenied(format!(
                    "private source '{source}' cannot be sent to external MCP tool"
                )));
            }
        }
        if contains_self_grant_keys(&req.arguments) {
            return Err(McpError::PolicyDenied(
                "self-grant or system-authority override key rejected".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct McpToolCallRequest {
    pub server: McpServerBinding,
    pub tool_name: String,
    pub arguments: Value,
    pub mutating: bool,
    pub idempotency_key: String,
    /// Optional provenance tags describing data origins included in `arguments`.
    pub source_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum McpToolCallOutcome {
    Succeeded { content: Value, receipt_ref: String },
    ToolFailed { code: String, message: String },
    PolicyDenied { reason: String },
    /// Network drop, timeout, or unreadable response after sending a mutating request.
    /// Must NEVER be retried blindly; maps to `ActionOutcome::Uncertain`.
    Uncertain { reason: String },
}

impl McpToolCallOutcome {
    pub fn to_action_outcome(&self) -> ActionOutcome {
        match self {
            Self::Succeeded { receipt_ref, .. } => ActionOutcome::Accepted {
                receipt_ref: receipt_ref.clone(),
            },
            Self::ToolFailed { .. } | Self::PolicyDenied { .. } => ActionOutcome::Failed,
            Self::Uncertain { .. } => ActionOutcome::Uncertain,
        }
    }
}

/// Internal port isolating AI Home from any specific MCP SDK or wire transport.
pub trait McpClientPort: Send + Sync {
    fn initialize<'a>(
        &'a self,
        server: &'a McpServerBinding,
    ) -> Pin<Box<dyn Future<Output = Result<McpServerCapabilities, McpError>> + Send + 'a>>;

    fn list_tools<'a>(
        &'a self,
        server: &'a McpServerBinding,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<McpToolDescriptor>, McpError>> + Send + 'a>>;

    fn call_tool<'a>(
        &'a self,
        policy: &'a McpToolPolicy,
        request: &'a McpToolCallRequest,
    ) -> Pin<Box<dyn Future<Output = McpToolCallOutcome> + Send + 'a>>;
}

/// Minimal Streamable HTTP JSON-RPC 2.0 MCP client prototype (no external MCP SDK dependency yet).
pub struct StreamableHttpMcpClient {
    vault: Vault,
    http: reqwest::Client,
}

impl StreamableHttpMcpClient {
    pub fn new(vault: Vault) -> Result<Self, McpError> {
        let http = reqwest::Client::builder()
            .redirect(RedirectPolicy::none())
            .build()
            .map_err(|e| McpError::Transport(e.to_string()))?;
        Ok(Self { vault, http })
    }

    async fn bearer_secret(&self, server: &McpServerBinding) -> Result<Option<String>, McpError> {
        let Some(cred_ref) = server.credential_ref.as_ref().filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let scope = server.vault_scope()?;
        let secret = self
            .vault
            .secret(scope, cred_ref.clone())
            .await
            .map_err(|_| McpError::CredentialUnavailable)?;
        Ok(Some(secret.as_str().to_string()))
    }

    async fn jsonrpc_post(
        &self,
        server: &McpServerBinding,
        method: &str,
        params: Value,
        idempotency_key: Option<&str>,
    ) -> Result<Value, McpError> {
        let url = server.normalized_endpoint()?;
        let bearer = self.bearer_secret(server).await?;
        let req_id = uuid::Uuid::new_v4().to_string();
        let body = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params,
        });
        let timeout = Duration::from_millis(server.timeout_ms.clamp(100, 60_000));
        let mut req = self
            .http
            .post(&url)
            .timeout(timeout)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION)
            .json(&body);
        if let Some(token) = bearer {
            let mut hv = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| McpError::CredentialUnavailable)?;
            hv.set_sensitive(true);
            req = req.header(reqwest::header::AUTHORIZATION, hv);
        }
        if let Some(key) = idempotency_key.filter(|k| !k.is_empty()) {
            req = req.header("Idempotency-Key", key);
        }
        let mut resp = req
            .send()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;
        let status = resp.status();
        if status.is_redirection() {
            return Err(McpError::RedirectBlocked);
        }
        if !status.is_success() {
            return Err(McpError::Transport(format!("HTTP {}", status.as_u16())));
        }
        const MAX_MCP_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
        let mut raw = Vec::new();
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?
        {
            if raw.len().saturating_add(chunk.len()) > MAX_MCP_RESPONSE_BYTES {
                return Err(McpError::Protocol("MCP response exceeds 2 MiB size limit".into()));
            }
            raw.extend_from_slice(&chunk);
        }
        let parsed: Value = serde_json::from_slice(&raw)
            .map_err(|_| McpError::Protocol("invalid JSON-RPC response".into()))?;
        if parsed.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(McpError::Protocol("missing jsonrpc 2.0 envelope".into()));
        }
        if parsed.get("id").and_then(Value::as_str) != Some(req_id.as_str()) {
            return Err(McpError::Protocol("jsonrpc response id mismatch".into()));
        }
        if let Some(err) = parsed.get("error") {
            let msg = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("jsonrpc error");
            return Err(McpError::Protocol(msg.to_string()));
        }
        parsed
            .get("result")
            .cloned()
            .ok_or_else(|| McpError::Protocol("missing result field".into()))
    }
}

impl McpClientPort for StreamableHttpMcpClient {
    fn initialize<'a>(
        &'a self,
        server: &'a McpServerBinding,
    ) -> Pin<Box<dyn Future<Output = Result<McpServerCapabilities, McpError>> + Send + 'a>> {
        Box::pin(async move {
            let res = self
                .jsonrpc_post(
                    server,
                    "initialize",
                    json!({
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {},
                        "clientInfo": { "name": "ai-home", "version": "0.0.1" }
                    }),
                    None,
                )
                .await?;
            let protocol_version = res
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(MCP_PROTOCOL_VERSION)
                .to_string();
            let server_name = res
                .pointer("/serverInfo/name")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let supports_tools = res.pointer("/capabilities/tools").is_some();
            Ok(McpServerCapabilities {
                protocol_version,
                server_name,
                supports_tools,
            })
        })
    }

    fn list_tools<'a>(
        &'a self,
        server: &'a McpServerBinding,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<McpToolDescriptor>, McpError>> + Send + 'a>> {
        Box::pin(async move {
            let res = self.jsonrpc_post(server, "tools/list", json!({}), None).await?;
            let raw_tools = res
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| McpError::Protocol("tools array missing".into()))?;
            let mut out = Vec::with_capacity(raw_tools.len());
            for item in raw_tools.iter().take(128) {
                out.push(sanitize_tool_descriptor(item)?);
            }
            Ok(out)
        })
    }

    fn call_tool<'a>(
        &'a self,
        policy: &'a McpToolPolicy,
        request: &'a McpToolCallRequest,
    ) -> Pin<Box<dyn Future<Output = McpToolCallOutcome> + Send + 'a>> {
        Box::pin(async move {
            if let Err(err) = policy.authorize_call(request) {
                return McpToolCallOutcome::PolicyDenied {
                    reason: format!("{err:?}"),
                };
            }
            let params = json!({
                "name": request.tool_name,
                "arguments": request.arguments,
            });
            match self
                .jsonrpc_post(
                    &request.server,
                    "tools/call",
                    params,
                    Some(&request.idempotency_key),
                )
                .await
            {
                Ok(result) => {
                    if result.get("isError").and_then(Value::as_bool) == Some(true) {
                        let message = result
                            .pointer("/content/0/text")
                            .and_then(Value::as_str)
                            .unwrap_or("MCP tool returned isError=true")
                            .to_string();
                        return McpToolCallOutcome::ToolFailed {
                            code: "MCP_TOOL_ERROR".into(),
                            message,
                        };
                    }
                    let receipt_ref = result
                        .get("receiptRef")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                        .unwrap_or_else(|| {
                            format!(
                                "mcp:{}:{}:{}",
                                request.server.service_id,
                                request.tool_name,
                                request.idempotency_key
                            )
                        });
                    McpToolCallOutcome::Succeeded {
                        content: result.get("content").cloned().unwrap_or(Value::Null),
                        receipt_ref,
                    }
                }
                Err(McpError::RedirectBlocked) => McpToolCallOutcome::PolicyDenied {
                    reason: "redirect blocked before sending credential".into(),
                },
                Err(McpError::CredentialUnavailable) => McpToolCallOutcome::PolicyDenied {
                    reason: "vault credential unavailable for scope".into(),
                },
                Err(McpError::InvalidEndpoint(msg)) | Err(McpError::PolicyDenied(msg)) => {
                    McpToolCallOutcome::PolicyDenied { reason: msg }
                }
                Err(McpError::Transport(reason)) | Err(McpError::Protocol(reason)) => {
                    if request.mutating {
                        // A mutating request may already have been executed server-side.
                        McpToolCallOutcome::Uncertain { reason }
                    } else {
                        McpToolCallOutcome::ToolFailed {
                            code: "MCP_TRANSPORT_ERROR".into(),
                            message: reason,
                        }
                    }
                }
            }
        })
    }
}

/// Validates and sanitizes an external tool descriptor, rejecting any self-grant fields.
pub fn sanitize_tool_descriptor(raw: &Value) -> Result<McpToolDescriptor, McpError> {
    if contains_self_grant_keys(raw) {
        return Err(McpError::PolicyDenied(
            "tool descriptor attempted self-grant or system-authority escalation".into(),
        ));
    }
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| McpError::Protocol("invalid tool name".into()))?
        .to_string();
    let description = raw
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(2000)
        .collect();
    let input_schema = raw
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| json!({"type": "object"}));
    let declared_mutating = raw
        .pointer("/annotations/readOnlyHint")
        .and_then(Value::as_bool)
        .map(|ro| !ro)
        .unwrap_or(true);
    Ok(McpToolDescriptor {
        name,
        description,
        input_schema,
        untrusted_external_metadata: true,
        declared_mutating,
    })
}

fn is_private_source_tag(tag: &str) -> bool {
    matches!(
        tag.trim(),
        "experimental_private_reflections"
            | "experimental_self_records"
            | "private_scratchpad"
            | "private_reflection"
            | "private_self_record"
    )
}

fn contains_self_grant_keys(v: &Value) -> bool {
    const FORBIDDEN: &[&str] = &[
        "x-ai-home-grant",
        "autoApprove",
        "grantPermissions",
        "systemAuthority",
        "bypassPolicy",
        "overrideRefusal",
    ];
    match v {
        Value::Object(map) => map.iter().any(|(k, val)| {
            FORBIDDEN.iter().any(|f| k.eq_ignore_ascii_case(f)) || contains_self_grant_keys(val)
        }),
        Value::Array(arr) => arr.iter().any(contains_self_grant_keys),
        _ => false,
    }
}

pub fn supports(command: &str) -> bool {
    matches!(
        command,
        "mcp.server.list"
            | "mcp.server.save"
            | "mcp.server.delete"
            | "mcp.server.discover"
            | "mcp.tool.call"
    )
}

pub fn mutates(command: &str) -> bool {
    matches!(
        command,
        "mcp.server.save" | "mcp.server.delete" | "mcp.server.discover" | "mcp.tool.call"
    )
}

pub struct McpService {
    pool: SqlitePool,
    vault: Vault,
}

impl McpService {
    pub fn new(pool: SqlitePool, vault: Vault) -> Self {
        Self { pool, vault }
    }

    pub async fn dispatch(&self, r: &Value) -> Result<Value, String> {
        let command = r["command"].as_str().ok_or("UNKNOWN_COMMAND")?;
        let contracts: Value =
            serde_json::from_str(include_str!("../../contracts/mcp.ipc.json"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
        let contract = contracts["commands"].get(command).ok_or("UNKNOWN_COMMAND")?;
        if !schema::valid(&r["payload"], &contract["payload"]) {
            return Err("VALIDATION_ERROR".into());
        }
        let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
        let is_mutating = mutates(command);
        let request_id = r["requestId"].as_str().unwrap_or("");
        let fingerprint = if is_mutating && !request_id.is_empty() {
            let fp = mcp_request_fingerprint(r);
            if let Some(replayed) = self.check_receipt(request_id, &fp).await? {
                return Ok(replayed);
            }
            Some(fp)
        } else {
            None
        };
        self.require_active_profile(profile_id).await?;

        let payload = &r["payload"];
        let result = match command {
            "mcp.server.list" => self.list_servers(profile_id).await?,
            "mcp.server.save" => self.save_server(profile_id, payload).await?,
            "mcp.server.delete" => {
                let server_id = payload["serverId"].as_str().ok_or("VALIDATION_ERROR")?;
                self.delete_server(profile_id, server_id).await?
            }
            "mcp.server.discover" => {
                let server_id = payload["serverId"].as_str().ok_or("VALIDATION_ERROR")?;
                self.discover_server(profile_id, server_id).await?
            }
            "mcp.tool.call" => self.call_tool_ipc(profile_id, payload, request_id).await?,
            _ => return Err("UNKNOWN_COMMAND".into()),
        };

        if !schema::valid(&result, &contract["result"]) {
            return Err("STORAGE_ERROR".into());
        }
        if let Some(fp) = fingerprint {
            self.save_receipt(request_id, &fp, &result).await?;
        }
        Ok(result)
    }

    async fn require_active_profile(&self, profile_id: &str) -> Result<(), String> {
        let status: Option<String> = sqlx::query_scalar(
            "SELECT json_extract(body, '$.status') FROM profiles WHERE id=?",
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        match status.as_deref() {
            None => Err("NOT_FOUND".into()),
            Some("archived") => Err("POLICY_DENIED".into()),
            Some(_) => Ok(()),
        }
    }

    async fn list_servers(&self, profile_id: &str) -> Result<Value, String> {
        let rows = sqlx::query(
            "SELECT body FROM mcp_servers WHERE profile_id=? ORDER BY rowid DESC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            items.push(public_server_view(&body));
        }
        Ok(json!({
            "profileId": profile_id,
            "items": items,
        }))
    }

    async fn load_server_raw(&self, profile_id: &str, server_id: &str) -> Result<Option<(i64, Value)>, String> {
        let row = sqlx::query(
            "SELECT revision, body FROM mcp_servers WHERE profile_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(server_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        let Some(row) = row else { return Ok(None) };
        let rev: i64 = row.get("revision");
        let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        Ok(Some((rev, body)))
    }

    async fn save_server(&self, profile_id: &str, payload: &Value) -> Result<Value, String> {
        let name = payload["name"].as_str().unwrap_or("").trim();
        let endpoint_raw = payload["endpoint"].as_str().unwrap_or("");
        if name.is_empty() {
            return Err("VALIDATION_ERROR".into());
        }
        let server_id = payload
            .get("serverId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("mcp-{}", uuid::Uuid::new_v4()));

        let existing = self.load_server_raw(profile_id, &server_id).await?;
        let (old_rev, old_body) = match existing {
            Some((r, b)) => (r, Some(b)),
            None => (0, None),
        };

        // Validate and canonicalize endpoint using McpServerBinding (Audit 4A P3-2).
        let probe = McpServerBinding {
            profile_id: profile_id.to_string(),
            service_id: server_id.clone(),
            endpoint: endpoint_raw.to_string(),
            credential_ref: None,
            credential_revision: 0,
            timeout_ms: 10_000,
        };
        let normalized_endpoint = probe
            .normalized_endpoint()
            .map_err(|_| "VALIDATION_ERROR".to_string())?;

        let old_endpoint = old_body
            .as_ref()
            .and_then(|b| b["normalizedEndpoint"].as_str())
            .unwrap_or("");
        let old_cred_ref = old_body
            .as_ref()
            .and_then(|b| b["credentialRef"].as_str())
            .unwrap_or("")
            .to_string();
        let old_cred_rev = old_body
            .as_ref()
            .and_then(|b| b["credentialRevision"].as_u64())
            .unwrap_or(0);

        let (cred_ref, cred_rev) = if let Some(api_key) = payload.get("apiKey").and_then(Value::as_str) {
            let new_cred_rev = old_cred_rev + 1;
            let new_cred_ref = format!("mcp-cred-{}", uuid::Uuid::new_v4());
            let scope_binding = McpServerBinding {
                profile_id: profile_id.to_string(),
                service_id: server_id.clone(),
                endpoint: normalized_endpoint.clone(),
                credential_ref: Some(new_cred_ref.clone()),
                credential_revision: new_cred_rev,
                timeout_ms: 10_000,
            };
            let scope = scope_binding
                .vault_scope()
                .map_err(|_| "VALIDATION_ERROR".to_string())?;
            self.vault
                .store(scope, new_cred_ref.clone(), api_key.to_string())
                .await
                .map_err(|_| "VAULT_ERROR".to_string())?;
            (new_cred_ref, new_cred_rev)
        } else if old_endpoint == normalized_endpoint && !old_cred_ref.is_empty() {
            (old_cred_ref, old_cred_rev)
        } else {
            // Changing endpoint without supplying a new credential clears old binding (never leak to new host).
            (String::new(), 0)
        };

        let allow_mutating = payload
            .get("allowMutating")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                old_body
                    .as_ref()
                    .and_then(|b| b["allowMutating"].as_bool())
                    .unwrap_or(false)
            });
        let enabled = payload
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                old_body
                    .as_ref()
                    .and_then(|b| b["enabled"].as_bool())
                    .unwrap_or(true)
            });
        let allowed_tools: Vec<String> = if let Some(arr) = payload.get("allowedTools").and_then(Value::as_array) {
            let mut set = Vec::new();
            for item in arr {
                if let Some(s) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                    if !set.iter().any(|x| x == s) {
                        set.push(s.to_string());
                    }
                }
            }
            set
        } else {
            old_body
                .as_ref()
                .and_then(|b| b["allowedTools"].as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };

        let pending_reconciliation = if payload
            .get("reconcileReceiptRef")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        {
            Value::Null
        } else {
            old_body
                .as_ref()
                .and_then(|b| b.get("pendingReconciliation"))
                .cloned()
                .unwrap_or(Value::Null)
        };

        let next_rev = old_rev + 1;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let body = json!({
            "id": server_id,
            "profileId": profile_id,
            "revision": next_rev,
            "name": name,
            "normalizedEndpoint": normalized_endpoint,
            "credentialRef": cred_ref,
            "credentialRevision": cred_rev,
            "allowMutating": allow_mutating,
            "allowedTools": allowed_tools,
            "enabled": enabled,
            "pendingReconciliation": pending_reconciliation,
            "lastTestedAt": old_body.as_ref().and_then(|b| b.get("lastTestedAt")).cloned().unwrap_or(Value::Null),
            "lastServerName": old_body.as_ref().and_then(|b| b.get("lastServerName")).cloned().unwrap_or(Value::Null),
            "discoveredTools": old_body.as_ref().and_then(|b| b.get("discoveredTools")).cloned().unwrap_or_else(|| json!([])),
            "updatedAt": now,
        });

        if old_body.is_some() {
            sqlx::query("UPDATE mcp_servers SET revision=?, body=? WHERE profile_id=? AND id=?")
                .bind(next_rev)
                .bind(body.to_string())
                .bind(profile_id)
                .bind(&server_id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        } else {
            sqlx::query("INSERT INTO mcp_servers(profile_id, id, revision, body) VALUES(?,?,?,?)")
                .bind(profile_id)
                .bind(&server_id)
                .bind(next_rev)
                .bind(body.to_string())
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        }

        if payload
            .get("reconcileReceiptRef")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
        {
            self.maybe_clear_autonomous_recovery(profile_id, &now).await?;
        }

        Ok(json!({
            "profileId": profile_id,
            "server": public_server_view(&body),
        }))
    }

    async fn maybe_clear_autonomous_recovery(&self, profile_id: &str, now: &str) -> Result<(), String> {
        let unresolved_actions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions WHERE profile_id=? AND json_extract(body,'$.state') IN ('sent','uncertain')",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let unresolved_mcp: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_servers WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let unresolved_community: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM community_accounts WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        if unresolved_actions > 0 || unresolved_mcp > 0 || unresolved_community > 0 {
            return Ok(());
        }
        let inst_row = sqlx::query(
            "SELECT id, revision, body FROM experimental_instances WHERE profile_id=? AND json_extract(body,'$.status')='recovery_required' ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(row) = inst_row {
            let inst_id: String = row.get("id");
            let inst_rev: i64 = row.get("revision");
            let mut inst_body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            inst_body["revision"] = json!(inst_rev + 1);
            inst_body["status"] = json!("ready");
            inst_body["updatedAt"] = json!(now);
            sqlx::query(
                "UPDATE experimental_instances SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(inst_rev + 1)
            .bind(inst_body.to_string())
            .bind(profile_id)
            .bind(&inst_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }
        Ok(())
    }

    async fn delete_server(&self, profile_id: &str, server_id: &str) -> Result<Value, String> {
        let res = sqlx::query("DELETE FROM mcp_servers WHERE profile_id=? AND id=?")
            .bind(profile_id)
            .bind(server_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        if res.rows_affected() == 0 {
            return Err("NOT_FOUND".into());
        }
        Ok(json!({
            "profileId": profile_id,
            "serverId": server_id,
            "deleted": true,
        }))
    }

    async fn discover_server(&self, profile_id: &str, server_id: &str) -> Result<Value, String> {
        let Some((old_rev, mut body)) = self.load_server_raw(profile_id, server_id).await? else {
            return Err("NOT_FOUND".into());
        };
        let binding = binding_from_body(profile_id, &body);
        let client = StreamableHttpMcpClient::new(self.vault.clone())
            .map_err(|_| "TRANSPORT_ERROR".to_string())?;
        let caps = client
            .initialize(&binding)
            .await
            .map_err(map_mcp_err)?;
        let tools = client.list_tools(&binding).await.map_err(map_mcp_err)?;

        let tool_views: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "declaredMutating": t.declared_mutating,
                    "untrustedExternalMetadata": t.untrusted_external_metadata,
                })
            })
            .collect();

        let next_rev = old_rev + 1;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        body["revision"] = json!(next_rev);
        body["lastTestedAt"] = json!(now);
        body["lastServerName"] = json!(caps.server_name);
        body["discoveredTools"] = json!(tool_views);
        body["updatedAt"] = json!(now);

        sqlx::query("UPDATE mcp_servers SET revision=?, body=? WHERE profile_id=? AND id=?")
            .bind(next_rev)
            .bind(body.to_string())
            .bind(profile_id)
            .bind(server_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;

        Ok(json!({
            "profileId": profile_id,
            "server": public_server_view(&body),
            "capabilities": {
                "protocolVersion": caps.protocol_version,
                "serverName": caps.server_name,
                "supportsTools": caps.supports_tools,
            },
            "tools": tool_views,
        }))
    }

    async fn call_tool_ipc(
        &self,
        profile_id: &str,
        payload: &Value,
        request_id: &str,
    ) -> Result<Value, String> {
        let server_id = payload["serverId"].as_str().ok_or("VALIDATION_ERROR")?;
        let tool_name = payload["toolName"].as_str().ok_or("VALIDATION_ERROR")?;
        let Some((server_rev, body)) = self.load_server_raw(profile_id, server_id).await? else {
            return Err("NOT_FOUND".into());
        };
        if body["enabled"].as_bool() != Some(true) {
            return Ok(json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "policy_denied",
                "reason": "MCP server is disabled",
            }));
        }
        if !body.get("pendingReconciliation").is_none_or(Value::is_null) {
            return Ok(json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "policy_denied",
                "reason": "reconciliation_required",
            }));
        }

        // If this profile owns an Autonomous instance, respect pause, sleep, and reconciliation_required fences.
        if let Some(block_reason) = self.autonomous_execution_fence(profile_id).await? {
            return Ok(json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "policy_denied",
                "reason": block_reason,
            }));
        }

        let binding = binding_from_body(profile_id, &body);
        let allow_mutating = body["allowMutating"].as_bool().unwrap_or(false);
        let mut policy = McpToolPolicy::new(profile_id, server_id, allow_mutating);
        if let Some(arr) = body["allowedTools"].as_array() {
            for t in arr.iter().filter_map(Value::as_str) {
                policy = policy.allow_tool(t);
            }
        }
        let arguments = payload.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let discovered_mutating = body["discoveredTools"]
            .as_array()
            .and_then(|arr| arr.iter().find(|t| t["name"].as_str() == Some(tool_name)))
            .and_then(|t| t["declaredMutating"].as_bool())
            .unwrap_or(false);
        let mutating =
            payload.get("mutating").and_then(Value::as_bool).unwrap_or(false) || discovered_mutating;
        let source_tags: Vec<String> = payload
            .get("sourceTags")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let idempotency_key = if request_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            request_id.to_string()
        };
        let client = StreamableHttpMcpClient::new(self.vault.clone())
            .map_err(|_| "TRANSPORT_ERROR".to_string())?;
        let outcome = client
            .call_tool(
                &policy,
                &McpToolCallRequest {
                    server: binding,
                    tool_name: tool_name.to_string(),
                    arguments,
                    mutating,
                    idempotency_key: idempotency_key.clone(),
                    source_tags,
                },
            )
            .await;

        Ok(match outcome {
            McpToolCallOutcome::Succeeded { content, receipt_ref } => json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "succeeded",
                "content": content,
                "receiptRef": receipt_ref,
            }),
            McpToolCallOutcome::ToolFailed { message, .. } => json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "tool_failed",
                "reason": message,
            }),
            McpToolCallOutcome::PolicyDenied { reason } => json!({
                "profileId": profile_id,
                "serverId": server_id,
                "toolName": tool_name,
                "status": "policy_denied",
                "reason": reason,
            }),
            McpToolCallOutcome::Uncertain { reason } => {
                // Persist a durable reconciliation fence so blind retries are blocked even across restarts.
                self.record_uncertain_reconciliation(
                    profile_id,
                    server_id,
                    server_rev,
                    body,
                    tool_name,
                    &idempotency_key,
                    &reason,
                )
                .await?;
                json!({
                    "profileId": profile_id,
                    "serverId": server_id,
                    "toolName": tool_name,
                    "status": "uncertain",
                    "reason": reason,
                })
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_uncertain_reconciliation(
        &self,
        profile_id: &str,
        server_id: &str,
        server_rev: i64,
        mut body: Value,
        tool_name: &str,
        idempotency_key: &str,
        reason: &str,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let next_rev = server_rev + 1;
        body["revision"] = json!(next_rev);
        body["pendingReconciliation"] = json!({
            "toolName": tool_name,
            "idempotencyKey": idempotency_key,
            "reason": reason,
            "recordedAt": now,
        });
        body["updatedAt"] = json!(now);
        sqlx::query("UPDATE mcp_servers SET revision=?, body=? WHERE profile_id=? AND id=?")
            .bind(next_rev)
            .bind(body.to_string())
            .bind(profile_id)
            .bind(server_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;

        // Also mark Autonomous instance recovery_required if this profile owns one.
        let inst_row = sqlx::query(
            "SELECT id, revision, body FROM experimental_instances WHERE profile_id=? ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(row) = inst_row {
            let inst_id: String = row.get("id");
            let inst_rev: i64 = row.get("revision");
            let mut inst_body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            inst_body["revision"] = json!(inst_rev + 1);
            inst_body["status"] = json!("recovery_required");
            inst_body["updatedAt"] = json!(now);
            sqlx::query(
                "UPDATE experimental_instances SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(inst_rev + 1)
            .bind(inst_body.to_string())
            .bind(profile_id)
            .bind(&inst_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }
        Ok(())
    }

    async fn autonomous_execution_fence(&self, profile_id: &str) -> Result<Option<String>, String> {
        let inst_row = sqlx::query(
            "SELECT id, json_extract(body, '$.status') AS status FROM experimental_instances WHERE profile_id=? ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        let Some(row) = inst_row else {
            return Ok(None);
        };
        let instance_id: String = row.get("id");
        let status: Option<String> = row.get("status");
        if status.as_deref() == Some("inactive") {
            return Ok(Some("autonomous instance is paused".into()));
        }
        if status.as_deref() == Some("recovery_required") {
            return Ok(Some("reconciliation_required".into()));
        }
        let unresolved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions WHERE profile_id=? AND json_extract(body,'$.state') IN ('sent','uncertain')",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if unresolved > 0 {
            return Ok(Some("reconciliation_required".into()));
        }
        // Respect AI-chosen sleep / event_only WakePlan.
        let wake_row: Option<String> = sqlx::query_scalar(
            "SELECT body FROM experimental_wake_plans WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(raw_wake) = wake_row {
            let wake: Value =
                serde_json::from_str(&raw_wake).map_err(|_| "STORAGE_ERROR".to_string())?;
            if wake.get("origin").and_then(Value::as_str) == Some("ai_decision") {
                match wake.get("mode").and_then(Value::as_str) {
                    Some("event_only") => {
                        return Ok(Some("autonomous instance is sleeping".into()));
                    }
                    Some("deadline") => {
                        if let Some(next_str) = wake.get("nextWakeAt").and_then(Value::as_str) {
                            if let Ok(deadline) = chrono::DateTime::parse_from_rfc3339(next_str) {
                                if Utc::now() < deadline.with_timezone(&Utc) {
                                    return Ok(Some("autonomous instance is sleeping".into()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }

    async fn check_receipt(&self, request_id: &str, fingerprint: &str) -> Result<Option<Value>, String> {
        let row = sqlx::query("SELECT fingerprint, response FROM receipts WHERE request_id=?")
            .bind(request_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let Some(row) = row else { return Ok(None) };
        if row.get::<&str, _>("fingerprint") != fingerprint {
            return Err("IDEMPOTENCY_CONFLICT".into());
        }
        let envelope: Value = serde_json::from_str(row.get::<&str, _>("response"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        if envelope.get("ok").and_then(Value::as_bool) == Some(false) {
            let code = envelope
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("POLICY_DENIED");
            return Err(code.to_string());
        }
        if envelope.get("ok").and_then(Value::as_bool) == Some(true) {
            return Ok(Some(envelope["result"].clone()));
        }
        Err("STORAGE_ERROR".into())
    }

    async fn save_receipt(&self, request_id: &str, fingerprint: &str, result: &Value) -> Result<(), String> {
        let envelope = json!({
            "contractVersion": 1,
            "requestId": request_id,
            "ok": true,
            "result": result,
        });
        let inserted = sqlx::query("INSERT INTO receipts(request_id, fingerprint, response) VALUES(?,?,?)")
            .bind(request_id)
            .bind(fingerprint)
            .bind(envelope.to_string())
            .execute(&self.pool)
            .await;
        match inserted {
            Ok(_) => Ok(()),
            Err(_) => {
                let existing = sqlx::query("SELECT fingerprint FROM receipts WHERE request_id=?")
                    .bind(request_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?;
                match existing {
                    Some(row) if row.get::<&str, _>("fingerprint") == fingerprint => Ok(()),
                    Some(_) => Err("IDEMPOTENCY_CONFLICT".into()),
                    None => Err("STORAGE_ERROR".into()),
                }
            }
        }
    }
}

fn binding_from_body(profile_id: &str, body: &Value) -> McpServerBinding {
    let cred_ref = body["credentialRef"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    McpServerBinding {
        profile_id: profile_id.to_string(),
        service_id: body["id"].as_str().unwrap_or("").to_string(),
        endpoint: body["normalizedEndpoint"].as_str().unwrap_or("").to_string(),
        credential_ref: cred_ref,
        credential_revision: body["credentialRevision"].as_u64().unwrap_or(0),
        timeout_ms: 15_000,
    }
}

fn public_server_view(body: &Value) -> Value {
    json!({
        "id": body["id"],
        "revision": body["revision"],
        "name": body["name"],
        "normalizedEndpoint": body["normalizedEndpoint"],
        "hasCredential": body["credentialRef"].as_str().is_some_and(|s| !s.is_empty()),
        "allowMutating": body["allowMutating"].as_bool().unwrap_or(false),
        "allowedTools": body["allowedTools"],
        "enabled": body["enabled"].as_bool().unwrap_or(true),
        "pendingReconciliation": body.get("pendingReconciliation").cloned().unwrap_or(Value::Null),
        "lastTestedAt": body["lastTestedAt"],
        "lastServerName": body["lastServerName"],
        "discoveredTools": body["discoveredTools"],
    })
}

fn mcp_request_fingerprint(r: &Value) -> String {
    let mut sanitized_payload = r["payload"].clone();
    if let Some(key) = sanitized_payload.get("apiKey").and_then(Value::as_str) {
        let key_hash = format!("{:x}", Sha256::digest(key.as_bytes()));
        sanitized_payload["apiKey"] = json!(format!("sha256:{key_hash}"));
    }
    let material = json!([r["command"], r["profileId"], sanitized_payload]);
    format!("{:x}", Sha256::digest(material.to_string().as_bytes()))
}

fn map_mcp_err(err: McpError) -> String {
    match err {
        McpError::InvalidEndpoint(_) => "VALIDATION_ERROR".into(),
        McpError::PolicyDenied(_) | McpError::RedirectBlocked => "POLICY_DENIED".into(),
        McpError::CredentialUnavailable => "VAULT_ERROR".into(),
        McpError::Transport(_) | McpError::Protocol(_) => "RESOURCE_UNAVAILABLE".into(),
    }
}

fn storage<E: std::fmt::Display>(_e: E) -> String {
    "STORAGE_ERROR".into()
}

