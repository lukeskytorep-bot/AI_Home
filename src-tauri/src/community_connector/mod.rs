//! Public, one-page Community 4A reader. Never executes instructions or writes.
//! Native-only service: no IPC/UI, credentials, scheduler or action executor.
use reqwest::{Client, Url};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    InvalidConfiguration,
    InvalidRequest,
    Transport,
    Timeout,
    RedirectDenied,
    Http(u16),
    RateLimited { retry_after_seconds: Option<u64> },
    ResponseTooLarge,
    InvalidResponse,
    IncompatibleApi,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    pub response_bytes: usize,
    pub timeout: Duration,
}
impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            response_bytes: 4 * 1024 * 1024,
            timeout: Duration::from_secs(15),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentTrust {
    UntrustedExternal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureValidation {
    NotPerformed,
}
#[derive(Debug, Clone)]
pub struct CommunityRead {
    pub profile_id: String,
    pub api_root: String,
    pub data: Value,
    pub trust: ContentTrust,
    pub signature_validation: SignatureValidation,
}
#[derive(Clone)]
pub struct CommunityReader {
    profile_id: String,
    root: Url,
    client: Client,
    limits: ReadLimits,
}
impl CommunityReader {
    pub fn new(profile_id: &str, api_root: &str, limits: ReadLimits) -> Result<Self, ReadError> {
        if profile_id.is_empty()
            || profile_id.len() > 256
            || profile_id.chars().any(char::is_control)
            || !(1024..=4 * 1024 * 1024).contains(&limits.response_bytes)
            || limits.timeout < Duration::from_millis(1)
            || limits.timeout > Duration::from_secs(30)
        {
            return Err(ReadError::InvalidConfiguration);
        }
        let root = normalize_root(api_root)?;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .referer(false)
            .connect_timeout(limits.timeout)
            .build()
            .map_err(|_| ReadError::InvalidConfiguration)?;
        Ok(Self {
            profile_id: profile_id.into(),
            root,
            client,
            limits,
        })
    }
    /// Discovery is capability information, not cryptographic server/agent identity.
    pub async fn discover(&self) -> Result<CommunityRead, ReadError> {
        let data = self.get(self.root.clone()).await?;
        discovery(&data)?;
        Ok(self.wrap(data))
    }
    pub async fn threads(
        &self,
        board: &str,
        page: u32,
        limit: u8,
    ) -> Result<CommunityRead, ReadError> {
        if !identifier(board, "brd_") {
            return Err(ReadError::InvalidRequest);
        }
        pagination(page, limit)?;
        let mut url = self
            .root
            .join("threads/")
            .map_err(|_| ReadError::InvalidRequest)?;
        url.query_pairs_mut()
            .append_pair("board_id", board)
            .append_pair("page", &page.to_string())
            .append_pair("limit", &limit.to_string());
        let data = self.get(url).await?;
        page_result(&data, board, None, page, limit)?;
        Ok(self.wrap(data))
    }
    pub async fn thread(&self, id: &str) -> Result<CommunityRead, ReadError> {
        if !identifier(id, "msg_") {
            return Err(ReadError::InvalidRequest);
        }
        let url = self
            .root
            .join(&format!("threads/{id}"))
            .map_err(|_| ReadError::InvalidRequest)?;
        let data = self.get(url).await?;
        content(&data["thread"], None, Some(id), false, false)?;
        Ok(self.wrap(data))
    }
    pub async fn replies(
        &self,
        id: &str,
        page: u32,
        limit: u8,
    ) -> Result<CommunityRead, ReadError> {
        if !identifier(id, "msg_") {
            return Err(ReadError::InvalidRequest);
        }
        pagination(page, limit)?;
        let mut url = self
            .root
            .join(&format!("threads/{id}/replies"))
            .map_err(|_| ReadError::InvalidRequest)?;
        url.query_pairs_mut()
            .append_pair("page", &page.to_string())
            .append_pair("limit", &limit.to_string());
        let data = self.get(url).await?;
        page_result(&data, "", Some(id), page, limit)?;
        Ok(self.wrap(data))
    }
    fn wrap(&self, data: Value) -> CommunityRead {
        CommunityRead {
            profile_id: self.profile_id.clone(),
            api_root: self.root.to_string(),
            data,
            trust: ContentTrust::UntrustedExternal,
            signature_validation: SignatureValidation::NotPerformed,
        }
    }
    async fn get(&self, url: Url) -> Result<Value, ReadError> {
        // Only locally built relative routes reach this method. Never follow server URLs.
        if url.origin() != self.root.origin() || !url.path().starts_with(self.root.path()) {
            return Err(ReadError::InvalidRequest);
        }
        tokio::time::timeout(self.limits.timeout, async {
            let mut response = self
                .client
                .get(url)
                .header("Accept", "application/json")
                .header("User-Agent", "AI-Home/0.0.1 CommunityReader")
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        ReadError::Timeout
                    } else {
                        ReadError::Transport
                    }
                })?;
            let status = response.status().as_u16();
            if (300..400).contains(&status) {
                return Err(ReadError::RedirectDenied);
            }
            if status == 429 {
                return Err(ReadError::RateLimited {
                    retry_after_seconds: response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .filter(|n| *n <= 86400),
                });
            }
            if status != 200 {
                return Err(ReadError::Http(status));
            }
            if response
                .content_length()
                .is_some_and(|n| n > self.limits.response_bytes as u64)
            {
                return Err(ReadError::ResponseTooLarge);
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
                return Err(ReadError::InvalidResponse);
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| {
                if e.is_timeout() {
                    ReadError::Timeout
                } else {
                    ReadError::Transport
                }
            })? {
                if chunk.len() > self.limits.response_bytes.saturating_sub(bytes.len()) {
                    return Err(ReadError::ResponseTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            let data: Value =
                serde_json::from_slice(&bytes).map_err(|_| ReadError::InvalidResponse)?;
            if !data.is_object() || data["ok"] != true || !text(&data["request_id"], 256, false) {
                return Err(ReadError::InvalidResponse);
            }
            Ok(data)
        })
        .await
        .map_err(|_| ReadError::Timeout)?
    }
}
fn normalize_root(raw: &str) -> Result<Url, ReadError> {
    if raw.len() > 2048
        || raw.chars().any(|c| c.is_control() || c.is_whitespace())
        || raw.contains(['%', '\\', '?', '#'])
    {
        return Err(ReadError::InvalidConfiguration);
    }
    // Reject dot segments before URL normalization, so a configured subpath is explicit.
    if raw.split('/').any(|p| p == "." || p == "..") {
        return Err(ReadError::InvalidConfiguration);
    }
    let mut u = Url::parse(raw).map_err(|_| ReadError::InvalidConfiguration)?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err(ReadError::InvalidConfiguration);
    }
    let p = u.path().trim_end_matches('/');
    if !p.ends_with("/api/v1")
        || p.split('/').skip(1).any(|s| {
            s.is_empty()
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
    {
        return Err(ReadError::InvalidConfiguration);
    }
    let path = format!("{p}/");
    u.set_path(&path);
    Ok(u)
}
fn identifier(v: &str, prefix: &str) -> bool {
    v.strip_prefix(prefix).is_some_and(|s| {
        s.len() == 32
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
fn pagination(page: u32, limit: u8) -> Result<(), ReadError> {
    if page == 0 || page > 10000 || limit == 0 || limit > 50 {
        Err(ReadError::InvalidRequest)
    } else {
        Ok(())
    }
}
fn text(v: &Value, max: usize, empty: bool) -> bool {
    v.as_str()
        .is_some_and(|s| (empty || !s.is_empty()) && s.len() <= max && !s.contains('\0'))
}
fn date(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() <= 64 && chrono::DateTime::parse_from_rfc3339(s).is_ok())
}
fn discovery(v: &Value) -> Result<(), ReadError> {
    if v["api_version"] != "v1"
        || !text(&v["server_version"], 128, false)
        || v["storage"]["backend"] != "sqlite3"
        || v["storage"]["schema_version"] != 9
        || v["storage"]["expected_schema_version"] != 9
        || v["storage"]["migration_state"] != "current"
    {
        return Err(ReadError::IncompatibleApi);
    }
    let caps = &v["capabilities"];
    if caps["forum_core"] != true
        || caps["forum_public_write_api"] != true
        || caps["change_feed_format"] != "community-change-feed/1"
    {
        return Err(ReadError::IncompatibleApi);
    }
    Ok(())
}
fn page_result(
    v: &Value,
    board: &str,
    thread: Option<&str>,
    page: u32,
    limit: u8,
) -> Result<(), ReadError> {
    if v["page"].as_u64() != Some(page as u64) || v["limit"].as_u64() != Some(limit as u64) {
        return Err(ReadError::InvalidResponse);
    }
    if thread.is_some_and(|id| v["thread_id"] != id) {
        return Err(ReadError::InvalidResponse);
    }
    let items = v["items"].as_array().ok_or(ReadError::InvalidResponse)?;
    if items.len() > limit as usize {
        return Err(ReadError::InvalidResponse);
    }
    let mut ids = std::collections::HashSet::new();
    for item in items {
        content(
            item,
            if board.is_empty() { None } else { Some(board) },
            thread,
            thread.is_some(),
            thread.is_none(),
        )?;
        if !ids.insert(item["content_id"].as_str().unwrap()) {
            return Err(ReadError::InvalidResponse);
        }
    }
    Ok(())
}
fn content(
    v: &Value,
    board: Option<&str>,
    thread: Option<&str>,
    reply: bool,
    preview: bool,
) -> Result<(), ReadError> {
    if !v.is_object() || v.get("parent_id").is_none() {
        return Err(ReadError::InvalidResponse);
    }
    let id = v["content_id"].as_str().ok_or(ReadError::InvalidResponse)?;
    let b = v["board_id"].as_str().ok_or(ReadError::InvalidResponse)?;
    let t = v["thread_id"].as_str().ok_or(ReadError::InvalidResponse)?;
    if !identifier(id, "msg_")
        || !identifier(b, "brd_")
        || !identifier(t, "msg_")
        || !v["author_actor_id"]
            .as_str()
            .is_some_and(|s| identifier(s, "agt_"))
        || v["type"] != if reply { "reply" } else { "thread" }
        || board.is_some_and(|x| x != b)
        || thread.is_some_and(|x| x != t)
        || (!reply && (id != t || !v["parent_id"].is_null()))
        || !v["revision_no"]
            .as_u64()
            .is_some_and(|n| n >= 1 && n <= i64::MAX as u64)
        || !date(&v["created_at"])
        || !date(&v["updated_at"])
    {
        return Err(ReadError::InvalidResponse);
    }
    if reply
        && !v["parent_id"]
            .as_str()
            .is_some_and(|s| identifier(s, "msg_"))
    {
        return Err(ReadError::InvalidResponse);
    }
    if !["active", "locked", "deleted"].contains(&v["status"].as_str().unwrap_or("")) {
        return Err(ReadError::InvalidResponse);
    }
    if let Some(t) = v.get("tombstone") {
        if !t.is_boolean() {
            return Err(ReadError::InvalidResponse);
        }
    }
    if v["tombstone"] == true {
        if [
            "title",
            "body",
            "preview",
            "client_provenance",
            "server_evidence",
        ]
        .iter()
        .any(|k| v.get(*k).is_some())
        {
            return Err(ReadError::InvalidResponse);
        }
        return Ok(());
    }
    if !["active", "locked"].contains(&v["status"].as_str().unwrap_or(""))
        || !text(&v["title"], 240, reply)
    {
        return Err(ReadError::InvalidResponse);
    }
    if preview {
        if !text(&v["preview"], 1120, true)
            || v["preview"]
                .as_str()
                .is_some_and(|s| s.chars().count() > 280)
            || v.get("body").is_some()
        {
            return Err(ReadError::InvalidResponse);
        }
    } else if !text(&v["body"], 65536, false) {
        return Err(ReadError::InvalidResponse);
    }
    let p = &v["client_provenance"];
    if !text(&p["client"], 160, true)
        || ![
            "autonomous",
            "human_requested",
            "human_source_provided",
            "owner_reviewed",
            "owner_edited",
            "agent_to_agent",
            "scheduled",
        ]
        .contains(&p["origin"].as_str().unwrap_or(""))
    {
        return Err(ReadError::InvalidResponse);
    }
    let e = &v["server_evidence"];
    if !e["signing_key_id"]
        .as_str()
        .is_some_and(|s| identifier(s, "key_"))
        || !e["signature_verified"].is_boolean()
        || !date(&e["received_at"])
    {
        return Err(ReadError::InvalidResponse);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
