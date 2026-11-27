//! Per-profile Community 4B-12 social client: account management, public feed reading,
//! and ECDSA P-256 signed post publishing with durable uncertain-write reconciliation.
//!
//! Kept strictly separate from frozen/protected modules (`community_connector/mod.rs` and
//! `community_identity/{mod,crypto,vault}.rs`) so their boundary invariants stay untouched.

use crate::community_connector::{CommunityReader, ReadError, ReadLimits};
use crate::domain::schema;
use crate::providers::{ProviderKind, ScopeStamp};
use crate::vault::Vault;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use p256::{
    ecdsa::{
        signature::Signer,
        Signature, SigningKey,
    },
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
};
use rand_core::{OsRng, RngCore};
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT},
    Client, Url,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::time::Duration;
use zeroize::Zeroizing;

pub const ALGORITHM: &str = "openssl-ecdsa-p256-sha256-der/1";
pub const PUBLIC_FORMAT: &str = "spki-der-base64url/1";
pub const SIGNATURE_FORMAT: &str = "asn1-der-base64url/1";

const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const HTTP_TIMEOUT_SECS: u64 = 15;

const BLOCKED_REFLECTION_SOURCES: &[&str] = &[
    "reflection",
    "private_reflection",
    "dream",
    "self_review",
    "internal_private",
    "private",
    "private_state",
    "internal_reflection",
    "journal",
];

pub fn supports(command: &str) -> bool {
    matches!(
        command,
        "community.account.list"
            | "community.account.save"
            | "community.account.delete"
            | "community.feed.read"
            | "community.post.publish"
    )
}

pub fn mutates(command: &str) -> bool {
    matches!(
        command,
        "community.account.save" | "community.account.delete" | "community.post.publish"
    )
}

fn embedded_contract() -> Value {
    serde_json::from_str(include_str!("../../contracts/community.social.ipc.json"))
        .expect("embedded community.social.ipc.json")
}

fn storage(_: sqlx::Error) -> String {
    "STORAGE_ERROR".into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn spki_der_for_point(point: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0x30,
        (24 + point.len()) as u8,
        0x30,
        0x13,
        0x06,
        0x07,
        0x2a,
        0x86,
        0x48,
        0xce,
        0x3d,
        0x02,
        0x01,
        0x06,
        0x08,
        0x2a,
        0x86,
        0x48,
        0xce,
        0x3d,
        0x03,
        0x01,
        0x07,
        0x03,
        (point.len() + 1) as u8,
        0x00,
    ];
    v.extend_from_slice(point);
    v
}

fn generate_p256_key() -> Result<(SigningKey, String, String, Zeroizing<String>), String> {
    for _ in 0..16 {
        let mut bytes = Zeroizing::new([0u8; 32]);
        OsRng
            .try_fill_bytes(bytes.as_mut())
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        if let Ok(key) = SigningKey::from_slice(bytes.as_ref()) {
            let point = key.verifying_key().to_encoded_point(false);
            let public_spki = URL_SAFE_NO_PAD.encode(spki_der_for_point(point.as_bytes()));
            let mut fp_bytes = b"P256-POINT/1\0".to_vec();
            fp_bytes.extend_from_slice(&point.as_bytes()[1..]);
            let fingerprint = sha256_hex(&fp_bytes);
            let pkcs8 = key
                .to_pkcs8_der()
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let encoded = Zeroizing::new(URL_SAFE_NO_PAD.encode(pkcs8.as_bytes()));
            return Ok((key, public_spki, fingerprint, encoded));
        }
    }
    Err("STORAGE_ERROR".into())
}

fn decode_p256_key(encoded: &str) -> Result<SigningKey, String> {
    let bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| "STORAGE_ERROR".to_string())?,
    );
    SigningKey::from_pkcs8_der(&bytes).map_err(|_| "STORAGE_ERROR".to_string())
}

/// Canonicalizes a community API root URL.
/// Allows `https://` for any host and `http://` only for loopback (`127.0.0.1`, `localhost`, `[::1]`).
/// Rejects credentials, query strings, fragments, dot segments, control characters, and port 0.
pub fn normalize_api_root(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 1024
        || trimmed.chars().any(|c| c.is_control() || c.is_whitespace())
        || trimmed.contains(['\\', '?', '#'])
    {
        return Err("VALIDATION_ERROR".into());
    }
    if trimmed.split('/').any(|s| s == "." || s == "..") {
        return Err("VALIDATION_ERROR".into());
    }
    let mut url = Url::parse(trimmed).map_err(|_| "VALIDATION_ERROR".to_string())?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("VALIDATION_ERROR".into());
    }
    if url.port() == Some(0) {
        return Err("VALIDATION_ERROR".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "VALIDATION_ERROR".to_string())?
        .to_ascii_lowercase();
    let is_loopback = matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]" | "::1");
    match url.scheme() {
        "https" => {}
        "http" if is_loopback => {}
        _ => return Err("VALIDATION_ERROR".into()),
    }
    let clean_path = url.path().trim_end_matches('/').to_string();
    if clean_path.is_empty() {
        url.set_path("/");
    } else {
        url.set_path(&clean_path);
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn safe_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && value != "."
        && value != ".."
}

fn has_blocked_reflection_tag(tags: &[String]) -> bool {
    tags.iter().any(|tag| {
        let lower = tag.trim().to_ascii_lowercase();
        BLOCKED_REFLECTION_SOURCES
            .iter()
            .any(|blocked| lower == *blocked || lower.starts_with(&format!("{blocked}:")) || lower.starts_with(&format!("{blocked}.")))
    })
}

pub fn canonical_post_statement(
    api_root: &str,
    board_id: &str,
    thread_id: &str,
    title: &str,
    body_sha256: &str,
    request_id: &str,
) -> String {
    format!(
        "COMMUNITY-POST/1\n{api_root}\n{board_id}\n{thread_id}\n{title}\n{body_sha256}\n{request_id}"
    )
}

fn community_scope(profile_id: &str, service_id: String, api_root: &str, revision: u64) -> ScopeStamp {
    ScopeStamp {
        profile_id: profile_id.to_string(),
        connection_id: service_id,
        provider: ProviderKind::CustomOpenai,
        normalized_endpoint: api_root.to_string(),
        credential_revision: revision,
        route_id: String::new(),
        route_revision: 0,
        model_id: String::new(),
    }
}

pub struct CommunitySocialService {
    pool: SqlitePool,
    vault: Vault,
}

impl CommunitySocialService {
    pub fn new(pool: SqlitePool, vault: Vault) -> Self {
        Self { pool, vault }
    }

    pub async fn dispatch(&self, request: &Value) -> Result<Value, String> {
        let command = request["command"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let contract = embedded_contract();
        let spec = contract["commands"]
            .get(command)
            .ok_or_else(|| "UNKNOWN_COMMAND".to_string())?;
        if !schema::valid(&request["payload"], &spec["payload"]) {
            return Err("VALIDATION_ERROR".into());
        }
        let profile_id = request["profileId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let request_id = request["requestId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let payload = &request["payload"];

        match command {
            "community.account.list" => self.list_accounts(profile_id).await,
            "community.account.save" => self.save_account(profile_id, request_id, payload).await,
            "community.account.delete" => self.delete_account(profile_id, request_id, payload).await,
            "community.feed.read" => self.read_feed(profile_id, payload).await,
            "community.post.publish" => self.publish_post(profile_id, request_id, payload).await,
            _ => Err("UNKNOWN_COMMAND".into()),
        }
    }

    async fn require_active_profile(&self, profile_id: &str) -> Result<(), String> {
        let status: Option<Option<String>> = sqlx::query_scalar(
            "SELECT json_extract(body, '$.status') FROM profiles WHERE id=?",
        )
        .bind(profile_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        let Some(status_val) = status else {
            return Err("NOT_FOUND".into());
        };
        if status_val.as_deref() == Some("archived") {
            return Err("POLICY_DENIED".into());
        }
        let pending_delete: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM profile_deletion_jobs WHERE profile_id=?")
                .bind(profile_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
        if pending_delete.is_some() {
            return Err("POLICY_DENIED".into());
        }
        Ok(())
    }

    async fn list_accounts(&self, profile_id: &str) -> Result<Value, String> {
        self.require_active_profile(profile_id).await?;
        let rows = sqlx::query(
            "SELECT body FROM community_accounts WHERE profile_id=? ORDER BY id ASC",
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            items.push(public_account_view(&body));
        }
        Ok(json!({
            "profileId": profile_id,
            "items": items,
            "onlineStatusKnown": false
        }))
    }

    async fn save_account(
        &self,
        profile_id: &str,
        request_id: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let token_hash = payload
            .get("apiToken")
            .and_then(Value::as_str)
            .map(|t| sha256_hex(t.as_bytes()));
        let fingerprint = sha256_hex(
            json!([
                "community.account.save",
                profile_id,
                payload.get("accountId"),
                payload.get("name"),
                payload.get("apiRoot"),
                payload.get("boardId"),
                token_hash,
                payload.get("allowWrites"),
                payload.get("enabled"),
                payload.get("reconcileReceiptRef"),
            ])
            .to_string()
            .as_bytes(),
        );
        if let Some(replayed) = self.check_receipt(request_id, &fingerprint).await? {
            return Ok(replayed);
        }
        self.require_active_profile(profile_id).await?;

        let name = payload["name"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let normalized_root = normalize_api_root(payload["apiRoot"].as_str().unwrap_or(""))?;
        let board_id = payload
            .get("boardId")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if !board_id.is_empty() && !safe_path_segment(&board_id) {
            return Err("VALIDATION_ERROR".into());
        }

        let account_id = payload
            .get("accountId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("comm_{}", &sha256_hex(format!("{profile_id}:{request_id}").as_bytes())[..16]));
        if !safe_path_segment(&account_id) {
            return Err("VALIDATION_ERROR".into());
        }

        let existing_row = sqlx::query(
            "SELECT revision, body FROM community_accounts WHERE profile_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(&account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;

        let existing: Option<(i64, Value)> = match existing_row {
            Some(row) => {
                let rev: i64 = row.get("revision");
                let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                    .map_err(|_| "STORAGE_ERROR".to_string())?;
                Some((rev, body))
            }
            None => None,
        };

        let now = Utc::now().to_rfc3339();
        let revision = existing.as_ref().map(|(r, _)| r + 1).unwrap_or(1);
        let created_at = existing
            .as_ref()
            .and_then(|(_, b)| b["createdAt"].as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| now.clone());

        let endpoint_changed = existing
            .as_ref()
            .and_then(|(_, b)| b["apiRoot"].as_str())
            .is_some_and(|old| old != normalized_root);

        // Handle optional Bearer API token in Vault, scoped to (profile_id, service_id, api_root).
        let token_service_id = format!("community_token:{account_id}");
        let mut cred_ref = existing
            .as_ref()
            .and_then(|(_, b)| b["credentialRef"].as_str())
            .map(ToString::to_string)
            .unwrap_or_default();
        let mut cred_rev = existing
            .as_ref()
            .and_then(|(_, b)| b["credentialRevision"].as_u64())
            .unwrap_or(0);

        if endpoint_changed && !cred_ref.is_empty() {
            cred_ref.clear();
        }
        if let Some(token) = payload.get("apiToken").and_then(Value::as_str) {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                return Err("VALIDATION_ERROR".into());
            }
            let next_rev = cred_rev + 1;
            let scope = community_scope(profile_id, token_service_id, &normalized_root, next_rev);
            let new_ref = format!("comm-tok:{profile_id}:{account_id}:r{next_rev}:{}", uuid::Uuid::new_v4().simple());
            self.vault
                .store(scope, new_ref.clone(), trimmed.to_string())
                .await
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            cred_ref = new_ref;
            cred_rev = next_rev;
        }

        // Handle persistent ECDSA P-256 signing key in Vault, scoped to (profile_id, service_id, api_root).
        let key_service_id = format!("community_key:{account_id}");
        let mut signing_key_ref = existing
            .as_ref()
            .and_then(|(_, b)| b["signingKeyRef"].as_str())
            .map(ToString::to_string)
            .unwrap_or_default();
        let mut signing_key_rev = existing
            .as_ref()
            .and_then(|(_, b)| b["signingKeyRevision"].as_u64())
            .unwrap_or(0);
        let mut public_key_spki = existing
            .as_ref()
            .and_then(|(_, b)| b["publicKeySpki"].as_str())
            .map(ToString::to_string)
            .unwrap_or_default();
        let mut public_key_fp = existing
            .as_ref()
            .and_then(|(_, b)| b["publicKeyFingerprint"].as_str())
            .map(ToString::to_string)
            .unwrap_or_default();

        if endpoint_changed && !signing_key_ref.is_empty() {
            signing_key_ref.clear();
            public_key_spki.clear();
            public_key_fp.clear();
        }
        if signing_key_ref.is_empty() {
            let (_key, spki, fp, encoded_priv) = generate_p256_key()?;
            let next_key_rev = signing_key_rev + 1;
            let scope = community_scope(profile_id, key_service_id, &normalized_root, next_key_rev);
            let new_key_ref = format!("comm-key:{profile_id}:{account_id}:r{next_key_rev}:{}", uuid::Uuid::new_v4().simple());
            self.vault
                .store(scope, new_key_ref.clone(), encoded_priv.to_string())
                .await
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            signing_key_ref = new_key_ref;
            signing_key_rev = next_key_rev;
            public_key_spki = spki;
            public_key_fp = fp;
        }

        let allow_writes = payload
            .get("allowWrites")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                existing
                    .as_ref()
                    .and_then(|(_, b)| b["allowWrites"].as_bool())
                    .unwrap_or(false)
            });
        let enabled = payload
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                existing
                    .as_ref()
                    .and_then(|(_, b)| b["enabled"].as_bool())
                    .unwrap_or(true)
            });

        let mut pending_reconciliation = existing
            .as_ref()
            .map(|(_, b)| b["pendingReconciliation"].clone())
            .unwrap_or(Value::Null);
        let mut reconciled_now = false;
        if let Some(reconcile_ref) = payload.get("reconcileReceiptRef").and_then(Value::as_str) {
            if let Some(pending_obj) = pending_reconciliation.as_object() {
                let expected_ref = pending_obj
                    .get("receiptRef")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !expected_ref.is_empty() && expected_ref != reconcile_ref {
                    return Err("VALIDATION_ERROR".into());
                }
                pending_reconciliation = Value::Null;
                reconciled_now = true;
            }
        }

        let body = json!({
            "schemaVersion": 1,
            "id": account_id,
            "profileId": profile_id,
            "revision": revision,
            "name": name,
            "apiRoot": normalized_root,
            "boardId": board_id,
            "credentialRef": cred_ref,
            "credentialRevision": cred_rev,
            "signingKeyRef": signing_key_ref,
            "signingKeyRevision": signing_key_rev,
            "publicKeySpki": public_key_spki,
            "publicKeyFingerprint": public_key_fp,
            "algorithm": ALGORITHM,
            "allowWrites": allow_writes,
            "enabled": enabled,
            "pendingReconciliation": pending_reconciliation,
            "createdAt": created_at,
            "updatedAt": now,
        });

        if existing.is_some() {
            sqlx::query(
                "UPDATE community_accounts SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(revision)
            .bind(body.to_string())
            .bind(profile_id)
            .bind(&account_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        } else {
            sqlx::query(
                "INSERT INTO community_accounts(profile_id, id, revision, body) VALUES(?,?,?,?)",
            )
            .bind(profile_id)
            .bind(&account_id)
            .bind(revision)
            .bind(body.to_string())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }

        if reconciled_now {
            self.maybe_clear_autonomous_recovery(profile_id).await?;
        }

        let result = json!({
            "profileId": profile_id,
            "account": public_account_view(&body),
            "onlineStatusKnown": false
        });
        self.save_receipt(request_id, &fingerprint, &result).await?;
        Ok(result)
    }

    async fn delete_account(
        &self,
        profile_id: &str,
        request_id: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let account_id = payload["accountId"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let fingerprint = sha256_hex(
            json!(["community.account.delete", profile_id, account_id])
                .to_string()
                .as_bytes(),
        );
        if let Some(replayed) = self.check_receipt(request_id, &fingerprint).await? {
            return Ok(replayed);
        }
        self.require_active_profile(profile_id).await?;

        let existing = sqlx::query(
            "SELECT body FROM community_accounts WHERE profile_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;

        let deleted = if existing.is_some() {
            sqlx::query("DELETE FROM community_accounts WHERE profile_id=? AND id=?")
                .bind(profile_id)
                .bind(account_id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
            self.maybe_clear_autonomous_recovery(profile_id).await?;
            true
        } else {
            false
        };

        let result = json!({
            "profileId": profile_id,
            "accountId": account_id,
            "deleted": deleted
        });
        self.save_receipt(request_id, &fingerprint, &result).await?;
        Ok(result)
    }

    async fn load_account(&self, profile_id: &str, account_id: &str) -> Result<Value, String> {
        let row = sqlx::query(
            "SELECT body FROM community_accounts WHERE profile_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        let Some(row) = row else {
            return Err("NOT_FOUND".into());
        };
        serde_json::from_str(row.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())
    }

    async fn read_feed(&self, profile_id: &str, payload: &Value) -> Result<Value, String> {
        self.require_active_profile(profile_id).await?;
        let account_id = payload["accountId"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let body = self.load_account(profile_id, account_id).await?;
        if body["enabled"].as_bool() != Some(true) {
            return Err("POLICY_DENIED".into());
        }
        let api_root = body["apiRoot"]
            .as_str()
            .ok_or_else(|| "STORAGE_ERROR".to_string())?;
        let board_id = payload
            .get("boardId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| body["boardId"].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("");
        if !board_id.is_empty() && !safe_path_segment(board_id) {
            return Err("VALIDATION_ERROR".into());
        }
        let page = payload.get("page").and_then(Value::as_u64).unwrap_or(1) as u32;
        let limit = payload.get("limit").and_then(Value::as_u64).unwrap_or(20) as u8;

        // Use the strict Community 4A `CommunityReader` whenever the configured endpoint
        // matches the Community 4A HTTPS `/api/v1` contract.
        if api_root.starts_with("https://") && api_root.ends_with("/api/v1") && board_id.starts_with("brd_") {
            let reader = CommunityReader::new(profile_id, api_root, ReadLimits::default())
                .map_err(|_| "VALIDATION_ERROR".to_string())?;
            let read = reader
                .threads(board_id, page, limit)
                .await
                .map_err(map_read_error)?;
            return Ok(json!({
                "profileId": profile_id,
                "accountId": account_id,
                "apiRoot": read.api_root,
                "untrustedExternalContent": true,
                "signatureValidation": "not_performed",
                "data": read.data,
            }));
        }

        // Generic compatible community read-only GET (no credentials sent on public feed read).
        let base = format!("{api_root}/");
        let base_url = Url::parse(&base).map_err(|_| "VALIDATION_ERROR".to_string())?;
        let mut target = base_url
            .join("threads")
            .map_err(|_| "VALIDATION_ERROR".to_string())?;
        {
            let mut qp = target.query_pairs_mut();
            if !board_id.is_empty() {
                qp.append_pair("board_id", board_id);
            }
            qp.append_pair("page", &page.to_string());
            qp.append_pair("limit", &limit.to_string());
        }
        if target.origin() != base_url.origin() {
            return Err("VALIDATION_ERROR".into());
        }
        let client = build_http_client()?;
        let data = tokio::time::timeout(Duration::from_secs(HTTP_TIMEOUT_SECS), async {
            let mut resp = client
                .get(target)
                .header(ACCEPT, "application/json")
                .header(USER_AGENT, "AI-Home/0.0.1 CommunitySocial")
                .send()
                .await
                .map_err(|_| "RESOURCE_UNAVAILABLE".to_string())?;
            let status = resp.status().as_u16();
            if (300..400).contains(&status) {
                return Err("POLICY_DENIED".into());
            }
            if status != 200 {
                return Err("RESOURCE_UNAVAILABLE".into());
            }
            let bytes = read_bounded_bytes(&mut resp).await?;
            serde_json::from_slice::<Value>(&bytes).map_err(|_| "RESOURCE_UNAVAILABLE".to_string())
        })
        .await
        .map_err(|_| "RESOURCE_UNAVAILABLE".to_string())??;

        Ok(json!({
            "profileId": profile_id,
            "accountId": account_id,
            "apiRoot": api_root,
            "untrustedExternalContent": true,
            "signatureValidation": "not_performed",
            "data": data,
        }))
    }

    async fn publish_post(
        &self,
        profile_id: &str,
        request_id: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let account_id = payload["accountId"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let content = payload["content"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let content_sha = sha256_hex(content.as_bytes());
        let fingerprint = sha256_hex(
            json!([
                "community.post.publish",
                profile_id,
                account_id,
                payload.get("boardId"),
                payload.get("threadId"),
                payload.get("title"),
                content_sha,
                payload.get("sourceTags"),
            ])
            .to_string()
            .as_bytes(),
        );
        if let Some(replayed) = self.check_receipt(request_id, &fingerprint).await? {
            return Ok(replayed);
        }
        self.require_active_profile(profile_id).await?;

        let body = self.load_account(profile_id, account_id).await?;

        // Guard 1: account enabled & writes explicitly allowed.
        if body["enabled"].as_bool() != Some(true) {
            let result = json!({
                "profileId": profile_id,
                "accountId": account_id,
                "status": "policy_denied",
                "signed": false,
                "reason": "community account is disabled"
            });
            self.save_receipt(request_id, &fingerprint, &result).await?;
            return Ok(result);
        }
        if body["allowWrites"].as_bool() != Some(true) {
            let result = json!({
                "profileId": profile_id,
                "accountId": account_id,
                "status": "policy_denied",
                "signed": false,
                "reason": "writes are disabled for this community account"
            });
            self.save_receipt(request_id, &fingerprint, &result).await?;
            return Ok(result);
        }

        // Guard 2: private reflection source tags cannot be published externally.
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
        if has_blocked_reflection_tag(&source_tags) {
            let result = json!({
                "profileId": profile_id,
                "accountId": account_id,
                "status": "policy_denied",
                "signed": false,
                "reason": "private reflection content cannot be published"
            });
            self.save_receipt(request_id, &fingerprint, &result).await?;
            return Ok(result);
        }

        // Guard 3: durable pendingReconciliation fence on this account.
        if !body["pendingReconciliation"].is_null() {
            let result = json!({
                "profileId": profile_id,
                "accountId": account_id,
                "status": "policy_denied",
                "signed": false,
                "reason": "reconciliation_required"
            });
            self.save_receipt(request_id, &fingerprint, &result).await?;
            return Ok(result);
        }

        // Guard 4: Autonomous instance execution fence (paused, recovery_required, unresolved actions, AI sleep).
        if let Some(reason) = self.autonomous_execution_fence(profile_id).await? {
            let result = json!({
                "profileId": profile_id,
                "accountId": account_id,
                "status": "policy_denied",
                "signed": false,
                "reason": reason
            });
            self.save_receipt(request_id, &fingerprint, &result).await?;
            return Ok(result);
        }

        let api_root = body["apiRoot"]
            .as_str()
            .ok_or_else(|| "STORAGE_ERROR".to_string())?;
        let board_id = payload
            .get("boardId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| body["boardId"].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("");
        if !board_id.is_empty() && !safe_path_segment(board_id) {
            return Err("VALIDATION_ERROR".into());
        }
        let thread_id = payload
            .get("threadId")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if !thread_id.is_empty() && !safe_path_segment(thread_id) {
            return Err("VALIDATION_ERROR".into());
        }
        let title = payload
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");

        // Load scoped ECDSA P-256 signing key from Vault.
        let key_ref = body["signingKeyRef"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "STORAGE_ERROR".to_string())?;
        let key_rev = body["signingKeyRevision"].as_u64().unwrap_or(1);
        let key_scope = community_scope(profile_id, format!("community_key:{account_id}"), api_root, key_rev);
        let encoded_key = Zeroizing::new(
            self.vault
                .secret(key_scope, key_ref.to_string())
                .await
                .map_err(|_| "STORAGE_ERROR".to_string())?,
        );
        let signing_key = decode_p256_key(&encoded_key)?;

        // Sign canonical statement.
        let statement =
            canonical_post_statement(api_root, board_id, thread_id, title, &content_sha, request_id);
        let sig: Signature = signing_key.sign(statement.as_bytes());
        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.to_der().as_bytes());

        // Optional scoped Bearer token from Vault.
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("AI-Home/0.0.1 CommunitySocial"),
        );
        if let Ok(idem) = HeaderValue::from_str(request_id) {
            headers.insert("Idempotency-Key", idem);
        }
        if let Some(cred_ref) = body["credentialRef"].as_str().filter(|s| !s.is_empty()) {
            let cred_rev = body["credentialRevision"].as_u64().unwrap_or(1);
            let cred_scope = community_scope(profile_id, format!("community_token:{account_id}"), api_root, cred_rev);
            let token = Zeroizing::new(
                self.vault
                    .secret(cred_scope, cred_ref.to_string())
                    .await
                    .map_err(|_| "STORAGE_ERROR".to_string())?,
            );
            let mut auth_val = HeaderValue::from_str(&format!("Bearer {}", token.as_str()))
                .map_err(|_| "VALIDATION_ERROR".to_string())?;
            auth_val.set_sensitive(true);
            headers.insert(AUTHORIZATION, auth_val);
        }

        let base = format!("{api_root}/");
        let base_url = Url::parse(&base).map_err(|_| "VALIDATION_ERROR".to_string())?;
        let rel = if thread_id.is_empty() {
            "threads".to_string()
        } else {
            format!("threads/{thread_id}/replies")
        };
        let target = base_url
            .join(&rel)
            .map_err(|_| "VALIDATION_ERROR".to_string())?;
        if target.origin() != base_url.origin() {
            return Err("VALIDATION_ERROR".into());
        }

        let post_body = json!({
            "boardId": board_id,
            "threadId": if thread_id.is_empty() { Value::Null } else { json!(thread_id) },
            "title": if title.is_empty() { Value::Null } else { json!(title) },
            "content": content,
            "bodySha256": content_sha,
            "publicKeySpki": body["publicKeySpki"],
            "publicKeyFingerprint": body["publicKeyFingerprint"],
            "algorithm": ALGORITHM,
            "publicKeyFormat": PUBLIC_FORMAT,
            "signatureFormat": SIGNATURE_FORMAT,
            "signature": sig_b64,
            "requestId": request_id,
        });

        let client = build_http_client()?;
        let send_outcome = tokio::time::timeout(Duration::from_secs(HTTP_TIMEOUT_SECS), async {
            let mut resp = match client.post(target).headers(headers).json(&post_body).send().await {
                Ok(r) => r,
                Err(_) => return PublishHttpOutcome::Uncertain("transport_uncertain".into()),
            };
            let status = resp.status().as_u16();
            if (300..400).contains(&status) {
                return PublishHttpOutcome::Failed("redirect_denied".into());
            }
            if (400..500).contains(&status) {
                return PublishHttpOutcome::Failed(format!("http_{status}"));
            }
            if status >= 500 {
                return PublishHttpOutcome::Uncertain(format!("http_{status}_uncertain"));
            }
            let bytes = match read_bounded_bytes(&mut resp).await {
                Ok(b) => b,
                Err(_) => return PublishHttpOutcome::Uncertain("response_read_uncertain".into()),
            };
            let parsed: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
            let remote_id = parsed
                .get("id")
                .or_else(|| parsed.pointer("/post/id"))
                .or_else(|| parsed.pointer("/thread/id"))
                .or_else(|| parsed.pointer("/reply/id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            PublishHttpOutcome::Published(remote_id)
        })
        .await
        .unwrap_or_else(|_| PublishHttpOutcome::Uncertain("timeout_uncertain".into()));

        let result = match send_outcome {
            PublishHttpOutcome::Published(remote_id) => {
                let mut res = json!({
                    "profileId": profile_id,
                    "accountId": account_id,
                    "status": "published",
                    "receiptRef": request_id,
                    "bodySha256": content_sha,
                    "signed": true,
                });
                if !remote_id.is_empty() {
                    res["remoteId"] = json!(remote_id);
                }
                res
            }
            PublishHttpOutcome::Failed(reason) => {
                json!({
                    "profileId": profile_id,
                    "accountId": account_id,
                    "status": "failed",
                    "receiptRef": request_id,
                    "bodySha256": content_sha,
                    "signed": true,
                    "reason": reason,
                })
            }
            PublishHttpOutcome::Uncertain(reason) => {
                self.mark_pending_reconciliation(
                    profile_id,
                    account_id,
                    request_id,
                    &content_sha,
                    &reason,
                )
                .await?;
                json!({
                    "profileId": profile_id,
                    "accountId": account_id,
                    "status": "uncertain",
                    "receiptRef": request_id,
                    "bodySha256": content_sha,
                    "signed": true,
                    "reason": reason,
                })
            }
        };

        self.save_receipt(request_id, &fingerprint, &result).await?;
        Ok(result)
    }

    async fn mark_pending_reconciliation(
        &self,
        profile_id: &str,
        account_id: &str,
        receipt_ref: &str,
        body_sha256: &str,
        reason: &str,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let row = sqlx::query(
            "SELECT revision, body FROM community_accounts WHERE profile_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(row) = row {
            let rev: i64 = row.get("revision");
            let mut body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            body["revision"] = json!(rev + 1);
            body["updatedAt"] = json!(now);
            body["pendingReconciliation"] = json!({
                "receiptRef": receipt_ref,
                "bodySha256": body_sha256,
                "reason": reason,
                "occurredAt": now,
            });
            sqlx::query(
                "UPDATE community_accounts SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(rev + 1)
            .bind(body.to_string())
            .bind(profile_id)
            .bind(account_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }

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

    async fn maybe_clear_autonomous_recovery(&self, profile_id: &str) -> Result<(), String> {
        let unresolved_actions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions WHERE profile_id=? AND json_extract(body,'$.state') IN ('sent','uncertain')",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if unresolved_actions > 0 {
            return Ok(());
        }
        let unresolved_mcp: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_servers WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if unresolved_mcp > 0 {
            return Ok(());
        }
        let unresolved_community: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM community_accounts WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if unresolved_community > 0 {
            return Ok(());
        }

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
            if inst_body.get("status").and_then(Value::as_str) == Some("recovery_required") {
                inst_body["revision"] = json!(inst_rev + 1);
                inst_body["status"] = json!("ready");
                inst_body["updatedAt"] = json!(Utc::now().to_rfc3339());
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

    async fn save_receipt(
        &self,
        request_id: &str,
        fingerprint: &str,
        result: &Value,
    ) -> Result<(), String> {
        let envelope = json!({
            "contractVersion": 1,
            "requestId": request_id,
            "ok": true,
            "result": result,
        });
        let inserted =
            sqlx::query("INSERT INTO receipts(request_id, fingerprint, response) VALUES(?,?,?)")
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

enum PublishHttpOutcome {
    Published(String),
    Failed(String),
    Uncertain(String),
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .referer(false)
        .connect_timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|_| "STORAGE_ERROR".to_string())
}

async fn read_bounded_bytes(resp: &mut reqwest::Response) -> Result<Vec<u8>, String> {
    if resp
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err("RESOURCE_UNAVAILABLE".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|_| "RESOURCE_UNAVAILABLE".to_string())?
    {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err("RESOURCE_UNAVAILABLE".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn map_read_error(err: ReadError) -> String {
    match err {
        ReadError::InvalidConfiguration | ReadError::InvalidRequest => "VALIDATION_ERROR".into(),
        ReadError::RedirectDenied => "POLICY_DENIED".into(),
        _ => "RESOURCE_UNAVAILABLE".into(),
    }
}

fn public_account_view(body: &Value) -> Value {
    json!({
        "id": body["id"],
        "revision": body["revision"],
        "name": body["name"],
        "apiRoot": body["apiRoot"],
        "boardId": body["boardId"],
        "hasCredential": body["credentialRef"].as_str().is_some_and(|s| !s.is_empty()),
        "publicKeySpki": body["publicKeySpki"],
        "publicKeyFingerprint": body["publicKeyFingerprint"],
        "algorithm": body["algorithm"],
        "allowWrites": body["allowWrites"],
        "enabled": body["enabled"],
        "pendingReconciliation": body["pendingReconciliation"],
        "createdAt": body["createdAt"],
        "updatedAt": body["updatedAt"],
    })
}
