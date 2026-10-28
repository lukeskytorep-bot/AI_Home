//! Cross-store deletion saga: durable app intent -> Community cleanup -> app
//! rows + both original/joining request receipts in one final transaction.
//! A failed or interrupted cleanup never publishes a success receipt.
use super::*;

pub(crate) async fn any_pending(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profile_deletion_jobs")
            .fetch_one(pool)
            .await?
            != 0,
    )
}

pub(crate) async fn backup_blocked(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    Ok(any_pending(pool).await?
        || sqlx::query_scalar::<_, i64>("SELECT count(*) FROM community_host_binding")
            .fetch_one(pool)
            .await?
            != 0)
}

pub(crate) async fn bind_host(
    tx: &mut Tx<'_>,
    service: Option<&crate::community_host::CommunityLifecycle>,
    command: &str,
    mutates: bool,
) -> Result<(), Failure> {
    let pinned: Option<String> =
        sqlx::query_scalar("SELECT registry_binding FROM community_host_binding WHERE id=1")
            .fetch_optional(&mut **tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    match (pinned, service) {
        (Some(binding), Some(service)) if binding != service.binding() => Err("POLICY_DENIED"),
        (Some(_), None) if command == "profile.delete.permanent" => Err("POLICY_DENIED"),
        (None, Some(service)) if mutates => {
            sqlx::query("INSERT INTO community_host_binding(id,registry_binding) VALUES(1,?)")
                .bind(service.binding())
                .execute(&mut **tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) async fn reserved_request(pool: &SqlitePool, r: &Value) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT profile_id,expected_revision FROM profile_deletion_jobs WHERE request_id=?",
    )
    .bind(r["requestId"].as_str().unwrap())
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some_and(|row| {
        r["command"] != "profile.delete.permanent"
            || r["profileId"].as_str() != Some(row.get::<&str, _>("profile_id"))
            || r["expectedRevision"].as_i64() != Some(row.get("expected_revision"))
            || r["payload"] != json!({"confirm":true})
    }))
}

pub(crate) async fn check_request(
    tx: &mut Tx<'_>,
    r: &Value,
    fingerprint: &str,
) -> Result<(), Failure> {
    if let Some(stored) = sqlx::query_scalar::<_, String>(
        "SELECT fingerprint FROM profile_deletion_jobs WHERE request_id=?",
    )
    .bind(r["requestId"].as_str().unwrap())
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?
    {
        if stored != fingerprint {
            return Err("IDEMPOTENCY_CONFLICT");
        }
    }
    Ok(())
}

pub(crate) async fn pending(tx: &mut Tx<'_>, profile: &str) -> Result<bool, Failure> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM profile_deletion_jobs WHERE profile_id=?",
    )
    .bind(profile)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?
        != 0)
}

async fn preflight(tx: &mut Tx<'_>, profile: &str, expected: i64) -> Result<(), Failure> {
    let p = get(tx, "profiles", "", profile).await?;
    if revision(&p)? != expected {
        return Err("REVISION_CONFLICT");
    }
    if p["status"] != "archived" {
        return Err("POLICY_DENIED");
    }
    let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations WHERE profile_id=? AND json_extract(body,'$.status') IN ('queued','running')")
        .bind(profile).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if running != 0 {
        return Err("POLICY_DENIED");
    }
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id=?",
    )
    .bind(profile)
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    // No owner override: inactive is still a necessary condition.
    if statuses.iter().any(|s| s != "inactive") {
        return Err("POLICY_DENIED");
    }
    Ok(())
}

pub(super) async fn execute(
    service: &Connections,
    mut tx: Tx<'_>,
    r: &Value,
    fingerprint: &str,
    contract: &Value,
) -> Result<Value, Failure> {
    if r["payload"]["confirm"] != true {
        return Err("VALIDATION_ERROR");
    }
    let profile = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
    let expected = r["expectedRevision"].as_i64().ok_or("VALIDATION_ERROR")?;
    let request = r["requestId"].as_str().unwrap();
    preflight(&mut tx, profile, expected).await?;
    let job = sqlx::query("SELECT request_id,fingerprint,expected_revision FROM profile_deletion_jobs WHERE profile_id=?")
        .bind(profile).fetch_optional(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
    let original = if let Some(row) = job {
        if row.get::<&str, _>("fingerprint") != fingerprint
            || row.get::<i64, _>("expected_revision") != expected
        {
            return Err("REVISION_CONFLICT");
        }
        if service.community.is_none() {
            return Err("POLICY_DENIED");
        }
        row.get::<String, _>("request_id")
    } else {
        request.to_string()
    };

    if let Some(community) = &service.community {
        sqlx::query("INSERT INTO profile_deletion_jobs(profile_id,request_id,fingerprint,expected_revision) VALUES(?,?,?,?) ON CONFLICT(profile_id) DO NOTHING")
            .bind(profile).bind(&original).bind(fingerprint).bind(expected)
            .execute(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
        // Never hold a main-store transaction across a blocking keyring call.
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
        community.delete_profile(profile).await?;
        tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        // Another independent host may have completed this same durable intent
        // while we were draining Community. Recover its committed result.
        if let Some(row) =
            sqlx::query("SELECT fingerprint,response FROM receipts WHERE request_id=?")
                .bind(&original)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?
        {
            if row.get::<&str, _>("fingerprint") != fingerprint {
                return Err("IDEMPOTENCY_CONFLICT");
            }
            let mut response: Value =
                serde_json::from_str(row.get("response")).map_err(|_| "STORAGE_ERROR")?;
            if response["ok"] != true
                || response["result"] != json!({"deleted":true,"profileId":profile})
                || response["requestId"] != original
            {
                return Err("STORAGE_ERROR");
            }
            response["requestId"] = json!(request);
            save_receipt(&mut tx, request, fingerprint, &response).await?;
            tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
            return Ok(response);
        }
        preflight(&mut tx, profile, expected).await?;
        let held: Option<String> = sqlx::query_scalar("SELECT request_id FROM profile_deletion_jobs WHERE profile_id=? AND fingerprint=? AND expected_revision=?")
            .bind(profile).bind(fingerprint).bind(expected).fetch_optional(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
        if held.as_deref() != Some(original.as_str()) {
            return Err("STORAGE_ERROR");
        }
    }

    delete_rows(&mut tx, profile).await?;
    let response = success(r, json!({"deleted":true,"profileId":profile}));
    if !schema::valid(&response["result"], &contract["result"]) {
        return Err("STORAGE_ERROR");
    }
    // A fresh request ID may explicitly join the same profile/revision intent.
    // Persist both receipts so the original lost reply is also replayable.
    let mut ids = vec![original.as_str()];
    if original != request {
        ids.push(request);
    }
    for id in ids {
        let mut receipt = response.clone();
        receipt["requestId"] = json!(id);
        save_receipt(&mut tx, id, fingerprint, &receipt).await?;
    }
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    if !violations.is_empty() {
        return Err("STORAGE_ERROR");
    }
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(response)
}

async fn save_receipt(
    tx: &mut Tx<'_>,
    id: &str,
    fingerprint: &str,
    response: &Value,
) -> Result<(), Failure> {
    if let Some(row) = sqlx::query("SELECT fingerprint,response FROM receipts WHERE request_id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
    {
        if row.get::<&str, _>("fingerprint") != fingerprint {
            return Err("IDEMPOTENCY_CONFLICT");
        }
        let old: Value = serde_json::from_str(row.get("response")).map_err(|_| "STORAGE_ERROR")?;
        if old != *response {
            return Err("STORAGE_ERROR");
        }
        return Ok(());
    }
    sqlx::query("INSERT INTO receipts(request_id,fingerprint,response) VALUES(?,?,?)")
        .bind(id)
        .bind(fingerprint)
        .bind(response.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}

pub(crate) async fn delete_rows(tx: &mut Tx<'_>, profile: &str) -> Result<(), Failure> {
    for table in [
        "experimental_self_records",
        "experimental_self_reviews",
        "experimental_private_reflections",
        "experimental_reflection_runs",
        "experimental_wake_candidates",
        "experimental_internal_state_events",
        "experimental_internal_state_snapshots",
        "experimental_internal_state_blobs",
        "experimental_semantic_checkpoints",
        "experimental_technical_checkpoints",
        "experimental_event_cursors",
        "experimental_actions",
        "experimental_refusals",
        "experimental_decisions",
        "experimental_wake_plans",
        "experimental_work_episodes",
        "experimental_instances",
        "message_documents",
        "chat_replay",
        "attachments",
        "chat_runs",
        "context_segments",
        "messages",
        "knowledge_sources",
        "persistent_guidance",
        "memory_version_metadata",
        "memory_versions",
        "memory_attempts",
        "memory_decisions",
        "memory_proposals",
        "memory_runs",
        "memory_metadata",
        "memory_records",
        "profile_import_materials",
        "import_materials",
        "chat_vision_policies",
        "chat_policies",
        "memory_contexts",
        "connection_models",
        "routes",
        "connections",
        "import_blobs",
        "operations",
        "conversation_cursors",
        "conversations",
        "profile_events",
        "profile_deletion_jobs",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE profile_id=?"))
            .bind(profile)
            .execute(&mut **tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    }
    crate::deletion::tombstone_profile_receipts(tx, profile).await?;
    sqlx::query("DELETE FROM profiles WHERE id=?")
        .bind(profile)
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    sqlx::query("DELETE FROM profile_cursors")
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
