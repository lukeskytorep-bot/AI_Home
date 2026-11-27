//! Per-profile declarative Skills layer (4B-12/13) with strict no-self-grant policy.
//!
//! Skills are declarative instruction packages scoped to a single profile. They have
//! zero direct network, Vault, or OS-shell execution privileges and cannot self-grant
//! capabilities or bypass Autonomous execution fences or private-reflection boundaries.

use crate::domain::schema;
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeSet;

pub const ALLOWED_CAPABILITIES: &[&str] = &[
    "read_context",
    "draft_only",
    "mcp_read_only",
    "community_read_only",
    "mcp_mutating",
    "community_publish",
];

const MUTATING_CAPABILITIES: &[&str] = &["mcp_mutating", "community_publish"];

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
        "skills.list" | "skills.save" | "skills.delete" | "skills.policy.check"
    )
}

pub fn mutates(command: &str) -> bool {
    matches!(command, "skills.save" | "skills.delete")
}

fn embedded_contract() -> Value {
    serde_json::from_str(include_str!("../../contracts/skills.ipc.json"))
        .expect("embedded skills.ipc.json")
}

fn storage(_: sqlx::Error) -> String {
    "STORAGE_ERROR".into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn safe_id(value: &str) -> bool {
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
        BLOCKED_REFLECTION_SOURCES.iter().any(|blocked| {
            lower == *blocked
                || lower.starts_with(&format!("{blocked}:"))
                || lower.starts_with(&format!("{blocked}."))
        })
    })
}

fn normalize_capabilities(
    value: Option<&Value>,
    strict_allowlist: bool,
) -> Result<Vec<String>, String> {
    let Some(arr) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut set = BTreeSet::new();
    for item in arr {
        let Some(cap) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) else {
            return Err("VALIDATION_ERROR".into());
        };
        if strict_allowlist && !ALLOWED_CAPABILITIES.contains(&cap) {
            return Err("VALIDATION_ERROR".into());
        }
        set.insert(cap.to_string());
    }
    Ok(set.into_iter().collect())
}

pub struct SkillsService {
    pool: SqlitePool,
}

impl SkillsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
            "skills.list" => self.list_skills(profile_id).await,
            "skills.save" => self.save_skill(profile_id, request_id, payload).await,
            "skills.delete" => self.delete_skill(profile_id, request_id, payload).await,
            "skills.policy.check" => self.check_policy(profile_id, payload).await,
            _ => Err("UNKNOWN_COMMAND".into()),
        }
    }

    async fn require_active_profile(&self, profile_id: &str) -> Result<(), String> {
        let status: Option<Option<String>> =
            sqlx::query_scalar("SELECT json_extract(body, '$.status') FROM profiles WHERE id=?")
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

    async fn list_skills(&self, profile_id: &str) -> Result<Value, String> {
        self.require_active_profile(profile_id).await?;
        let rows = sqlx::query("SELECT body FROM profile_skills WHERE profile_id=? ORDER BY id ASC")
            .bind(profile_id)
            .fetch_all(&self.pool)
            .await
            .map_err(storage)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            items.push(public_skill_view(&body));
        }
        Ok(json!({
            "profileId": profile_id,
            "items": items
        }))
    }

    async fn save_skill(
        &self,
        profile_id: &str,
        request_id: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let instructions_raw = payload["instructions"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let instructions_sha = sha256_hex(instructions_raw.as_bytes());
        let fingerprint = sha256_hex(
            json!([
                "skills.save",
                profile_id,
                payload.get("skillId"),
                payload.get("name"),
                payload.get("description"),
                instructions_sha,
                payload.get("source"),
                payload.get("requestedCapabilities"),
                payload.get("grantedCapabilities"),
                payload.get("enabled"),
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
            .filter(|s| !s.is_empty() && s.len() <= 128 && !s.contains('\0'))
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if description.contains('\0') {
            return Err("VALIDATION_ERROR".into());
        }
        if instructions_raw.trim().is_empty() || instructions_raw.contains('\0') {
            return Err("VALIDATION_ERROR".into());
        }
        let source = payload
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("user");

        // Strict no-self-grant invariant: imported skills can NEVER arrive enabled or with
        // pre-granted capabilities. They must be imported disabled with empty grants and
        // explicitly approved by the user in a separate `source: "user"` save.
        if source == "imported" {
            let wants_enabled = payload.get("enabled").and_then(Value::as_bool) == Some(true);
            let wants_grants = payload
                .get("grantedCapabilities")
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty());
            if wants_enabled || wants_grants {
                return Err("POLICY_DENIED".into());
            }
        }

        let skill_id = payload
            .get("skillId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                format!(
                    "skill_{}",
                    &sha256_hex(format!("{profile_id}:{request_id}").as_bytes())[..16]
                )
            });
        if !safe_id(&skill_id) {
            return Err("VALIDATION_ERROR".into());
        }

        let existing_row =
            sqlx::query("SELECT revision, body FROM profile_skills WHERE profile_id=? AND id=?")
                .bind(profile_id)
                .bind(&skill_id)
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

        let requested = if payload.get("requestedCapabilities").is_some() {
            normalize_capabilities(payload.get("requestedCapabilities"), false)?
        } else {
            existing
                .as_ref()
                .and_then(|(_, b)| b.get("requestedCapabilities"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };

        let granted = if source == "imported" {
            // Re-importing never preserves or grants capabilities automatically.
            Vec::new()
        } else if payload.get("grantedCapabilities").is_some() {
            normalize_capabilities(payload.get("grantedCapabilities"), true)?
        } else {
            existing
                .as_ref()
                .and_then(|(_, b)| b.get("grantedCapabilities"))
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default()
        };

        let enabled = if source == "imported" {
            false
        } else {
            payload
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|(_, b)| b["enabled"].as_bool())
                        .unwrap_or(false)
                })
        };

        let now = Utc::now().to_rfc3339();
        let revision = existing.as_ref().map(|(r, _)| r + 1).unwrap_or(1);
        let created_at = existing
            .as_ref()
            .and_then(|(_, b)| b["createdAt"].as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| now.clone());

        let body = json!({
            "schemaVersion": 1,
            "id": skill_id,
            "profileId": profile_id,
            "revision": revision,
            "name": name,
            "description": description,
            "instructions": instructions_raw,
            "instructionsSha256": instructions_sha,
            "source": source,
            "requestedCapabilities": requested,
            "grantedCapabilities": granted,
            "enabled": enabled,
            "createdAt": created_at,
            "updatedAt": now,
        });

        if existing.is_some() {
            sqlx::query(
                "UPDATE profile_skills SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(revision)
            .bind(body.to_string())
            .bind(profile_id)
            .bind(&skill_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        } else {
            sqlx::query(
                "INSERT INTO profile_skills(profile_id, id, revision, body) VALUES(?,?,?,?)",
            )
            .bind(profile_id)
            .bind(&skill_id)
            .bind(revision)
            .bind(body.to_string())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }

        let result = json!({
            "profileId": profile_id,
            "skill": public_skill_view(&body)
        });
        self.save_receipt(request_id, &fingerprint, &result).await?;
        Ok(result)
    }

    async fn delete_skill(
        &self,
        profile_id: &str,
        request_id: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let skill_id = payload["skillId"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let fingerprint = sha256_hex(
            json!(["skills.delete", profile_id, skill_id])
                .to_string()
                .as_bytes(),
        );
        if let Some(replayed) = self.check_receipt(request_id, &fingerprint).await? {
            return Ok(replayed);
        }
        self.require_active_profile(profile_id).await?;

        let rows_affected = sqlx::query("DELETE FROM profile_skills WHERE profile_id=? AND id=?")
            .bind(profile_id)
            .bind(skill_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?
            .rows_affected();

        let result = json!({
            "profileId": profile_id,
            "skillId": skill_id,
            "deleted": rows_affected > 0
        });
        self.save_receipt(request_id, &fingerprint, &result).await?;
        Ok(result)
    }

    async fn check_policy(&self, profile_id: &str, payload: &Value) -> Result<Value, String> {
        self.require_active_profile(profile_id).await?;
        let skill_id = payload["skillId"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;
        let capability = payload["capability"]
            .as_str()
            .ok_or_else(|| "VALIDATION_ERROR".to_string())?;

        let row = sqlx::query("SELECT body FROM profile_skills WHERE profile_id=? AND id=?")
            .bind(profile_id)
            .bind(skill_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let Some(row) = row else {
            return Err("NOT_FOUND".into());
        };
        let body: Value = serde_json::from_str(row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;

        let enabled = body["enabled"].as_bool() == Some(true);
        let granted: Vec<String> = body["grantedCapabilities"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .filter(|c| ALLOWED_CAPABILITIES.contains(c))
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        if !enabled {
            return Ok(json!({
                "profileId": profile_id,
                "skillId": skill_id,
                "capability": capability,
                "allowed": false,
                "reason": "skill is disabled",
                "effectiveCapabilities": []
            }));
        }

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
            return Ok(json!({
                "profileId": profile_id,
                "skillId": skill_id,
                "capability": capability,
                "allowed": false,
                "reason": "private reflection content cannot be accessed by skills",
                "effectiveCapabilities": granted
            }));
        }

        // Strict no-self-grant: requestedCapabilities are NEVER honored; only grantedCapabilities count.
        if !granted.iter().any(|c| c == capability) {
            return Ok(json!({
                "profileId": profile_id,
                "skillId": skill_id,
                "capability": capability,
                "allowed": false,
                "reason": "capability not explicitly granted",
                "effectiveCapabilities": granted
            }));
        }

        if MUTATING_CAPABILITIES.contains(&capability) {
            if let Some(reason) = self.autonomous_execution_fence(profile_id).await? {
                return Ok(json!({
                    "profileId": profile_id,
                    "skillId": skill_id,
                    "capability": capability,
                    "allowed": false,
                    "reason": reason,
                    "effectiveCapabilities": granted
                }));
            }
        }

        Ok(json!({
            "profileId": profile_id,
            "skillId": skill_id,
            "capability": capability,
            "allowed": true,
            "reason": "allowed",
            "effectiveCapabilities": granted
        }))
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
        .map_err(storage)?;
        if unresolved_actions + unresolved_mcp + unresolved_community > 0 {
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

    async fn check_receipt(
        &self,
        request_id: &str,
        fingerprint: &str,
    ) -> Result<Option<Value>, String> {
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

fn public_skill_view(body: &Value) -> Value {
    json!({
        "id": body["id"],
        "revision": body["revision"],
        "name": body["name"],
        "description": body["description"],
        "instructions": body["instructions"],
        "instructionsSha256": body["instructionsSha256"],
        "source": body["source"],
        "requestedCapabilities": body["requestedCapabilities"],
        "grantedCapabilities": body["grantedCapabilities"],
        "enabled": body["enabled"],
        "createdAt": body["createdAt"],
        "updatedAt": body["updatedAt"],
    })
}
