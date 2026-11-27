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

use crate::experimental_runtime::{ActionOutcome};
use crate::providers::{ProviderKind, ScopeStamp};
use crate::vault::Vault;
use reqwest::redirect::Policy as RedirectPolicy;
use serde_json::{json, Value};
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
    /// Validates endpoint URL: HTTPS only (or explicit loopback HTTP for local fake/dev servers),
    /// no userinfo, no control characters, no empty query/fragment traps.
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
        if trimmed.ends_with('?') || trimmed.ends_with('#') {
            return Err(McpError::InvalidEndpoint("trailing empty query/fragment".into()));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| McpError::InvalidEndpoint("missing host".into()))?;
        let loopback = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
        match parsed.scheme() {
            "https" => Ok(parsed.to_string()),
            "http" if loopback => Ok(parsed.to_string()),
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
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        if let Some(key) = idempotency_key.filter(|k| !k.is_empty()) {
            req = req.header("Idempotency-Key", key);
        }
        let resp = req
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
        let parsed: Value = resp
            .json()
            .await
            .map_err(|_| McpError::Protocol("invalid JSON-RPC response".into()))?;
        if parsed.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(McpError::Protocol("missing jsonrpc 2.0 envelope".into()));
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
