//! Profile-scoped configuration and durable connection tests. No SQL in the UI.
use crate::{
    domain::schema,
    providers::{
        self,
        transport::{BoundCredential, CallBudget, Cancellation, HttpTransport, TransportError},
        ProviderKind, ScopeStamp,
    },
    vault::Vault,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
#[path = "conversations.rs"]
mod conversation_storage;
type Failure = &'static str;
type Tx<'a> = Transaction<'a, Sqlite>;
const COMMANDS: &[&str] = &[
    "conversation.create",
    "conversation.draft.set",
    "conversation.get",
    "conversation.list",
    "conversation.messages.list",
    "profile.create",
    "profile.list",
    "profile.snapshot",
    "connection.create",
    "connection.update",
    "connection.credential.set",
    "connection.test",
    "connection.models.list",
    "connection.test.latest",
    "route.save",
    "route.assign",
    "operation.get",
    "operation.cancel",
];
pub fn supports(command: &str) -> bool {
    COMMANDS.contains(&command)
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
fn error(r: &Value, code: Failure) -> Value {
    json!({"contractVersion":1,"requestId":r["requestId"],"ok":false,"error":{"code":code,"messageKey":format!("error.{}",code.to_lowercase()),"retryable":false}})
}
fn success(r: &Value, result: Value) -> Value {
    json!({"contractVersion":1,"requestId":r["requestId"],"ok":true,"result":result})
}
fn kind(value: &Value) -> Result<ProviderKind, Failure> {
    match value.as_str() {
        Some("openrouter") => Ok(ProviderKind::Openrouter),
        Some("openai") => Ok(ProviderKind::Openai),
        Some("google") => Ok(ProviderKind::Google),
        Some("anthropic") => Ok(ProviderKind::Anthropic),
        Some("custom_openai") => Ok(ProviderKind::CustomOpenai),
        _ => Err("CONNECTION_INVALID"),
    }
}
fn stamp(c: &Value) -> Result<ScopeStamp, Failure> {
    Ok(ScopeStamp {
        profile_id: c["profileId"].as_str().ok_or("STORAGE_ERROR")?.into(),
        connection_id: c["id"].as_str().ok_or("STORAGE_ERROR")?.into(),
        provider: kind(&c["providerKind"])?,
        normalized_endpoint: c["normalizedEndpoint"]
            .as_str()
            .ok_or("STORAGE_ERROR")?
            .into(),
        credential_revision: c["credentialRevision"].as_u64().ok_or("STORAGE_ERROR")?,
        route_id: "connection-test".into(),
        route_revision: 1,
        model_id: "discovery".into(),
    })
}
fn revision(v: &Value) -> Result<i64, Failure> {
    v["revision"].as_i64().ok_or("STORAGE_ERROR")
}
fn bump(v: &mut Value) -> Result<(), Failure> {
    v["revision"] = json!(revision(v)?
        .checked_add(1)
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or("VALIDATION_ERROR")?);
    Ok(())
}
fn invalidate(c: &mut Value) {
    c["status"] = json!("unverified");
    for key in ["verifiedRevision", "verifiedCredentialRevision", "testedAt"] {
        c.as_object_mut().unwrap().remove(key);
    }
}
fn verified(c: &Value) -> bool {
    c["status"] == "verified"
        && c["verifiedRevision"] == c["revision"]
        && c["verifiedCredentialRevision"] == c["credentialRevision"]
}
fn validate_record(table: &str, v: &Value) -> bool {
    schema::dto(
        match table {
            "profiles" => "Profile",
            "connections" => "ProviderConnection",
            "routes" => "ModelRoute",
            "conversations" => "Conversation",
            "messages" => "Message",
            _ => "Operation",
        },
        v,
    )
}
async fn get(tx: &mut Tx<'_>, table: &str, profile: &str, record: &str) -> Result<Value, Failure> {
    let sql = if table == "profiles" {
        "SELECT revision,body FROM profiles WHERE id=?".to_string()
    } else {
        format!("SELECT revision,body FROM {table} WHERE id=? AND profile_id=?")
    };
    let q = sqlx::query(&sql).bind(record);
    let q = if table == "profiles" {
        q
    } else {
        q.bind(profile)
    };
    let row = q
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
        .ok_or("NOT_FOUND")?;
    let v: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
    if !validate_record(table, &v)
        || v["id"] != record
        || revision(&v)? != row.get::<i64, _>("revision")
        || table != "profiles" && v["profileId"] != profile
    {
        return Err("STORAGE_ERROR");
    }
    Ok(v)
}
async fn put(
    tx: &mut Tx<'_>,
    table: &str,
    v: &Value,
    new: bool,
    expected: i64,
) -> Result<(), Failure> {
    if !validate_record(table, v) {
        return Err("STORAGE_ERROR");
    }
    let profile = v["profileId"].as_str().unwrap_or("");
    let record = v["id"].as_str().unwrap();
    let rev = revision(v)?;
    let changed=match (table,new){
        ("profiles",true)=>sqlx::query("INSERT INTO profiles(id,revision,body) VALUES(?,?,?)").bind(record).bind(rev).bind(v.to_string()).execute(&mut **tx).await,
        ("profiles",false)=>sqlx::query("UPDATE profiles SET revision=?,body=? WHERE id=? AND revision=?").bind(rev).bind(v.to_string()).bind(record).bind(expected).execute(&mut **tx).await,
        ("routes",true)=>sqlx::query("INSERT INTO routes(profile_id,id,connection_id,revision,body) VALUES(?,?,?,?,?)").bind(profile).bind(record).bind(v["connectionId"].as_str().unwrap()).bind(rev).bind(v.to_string()).execute(&mut **tx).await,
        ("routes",false)=>sqlx::query("UPDATE routes SET connection_id=?,revision=?,body=? WHERE profile_id=? AND id=? AND revision=?").bind(v["connectionId"].as_str().unwrap()).bind(rev).bind(v.to_string()).bind(profile).bind(record).bind(expected).execute(&mut **tx).await,
        (_,true)=>sqlx::query(&format!("INSERT INTO {table}(profile_id,id,revision,body) VALUES(?,?,?,?)")).bind(profile).bind(record).bind(rev).bind(v.to_string()).execute(&mut **tx).await,
        (_,false)=>sqlx::query(&format!("UPDATE {table} SET revision=?,body=? WHERE profile_id=? AND id=? AND revision=?")).bind(rev).bind(v.to_string()).bind(profile).bind(record).bind(expected).execute(&mut **tx).await,
    }.map_err(|_|"STORAGE_ERROR")?;
    if changed.rows_affected() != 1 {
        return Err("REVISION_CONFLICT");
    }
    Ok(())
}
async fn event(tx: &mut Tx<'_>, profile: &str, typ: &str, payload: &Value) -> Result<(), Failure> {
    let seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?",
    )
    .bind(profile)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    let mut value = json!({"contractVersion":1,"eventId":id(),"profileId":profile,"sequence":seq,"eventType":typ,"occurredAt":now(),"payload":payload});
    if typ == "operation.changed" {
        value["operationId"] = payload["id"].clone();
    }
    if !schema::dto("EventEnvelope", &value) {
        return Err("STORAGE_ERROR");
    }
    sqlx::query("INSERT INTO profile_events(profile_id,sequence,body) VALUES(?,?,?)")
        .bind(profile)
        .bind(seq)
        .bind(value.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
#[derive(Clone)]
pub struct Connections {
    pool: SqlitePool,
    vault: Vault,
    gate: Arc<Mutex<()>>,
    active: Arc<Mutex<HashMap<String, (Cancellation, CallBudget)>>>,
}
impl Connections {
    pub fn new(pool: SqlitePool, vault: Vault) -> Self {
        Self {
            pool,
            vault,
            gate: Arc::new(Mutex::new(())),
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub async fn dispatch(&self, r: Value) -> Value {
        match self.execute(&r).await {
            Ok(v) => v,
            Err(code) => error(&r, code),
        }
    }
    async fn execute(&self, r: &Value) -> Result<Value, Failure> {
        let _guard = self.gate.lock().await;
        let command = r["command"].as_str().ok_or("VALIDATION_ERROR")?;
        let catalog: Value =
            serde_json::from_str(include_str!("../../contracts/ipc.catalog.json")).unwrap();
        let extra: Value =
            serde_json::from_str(include_str!("../../contracts/connections.ipc.json")).unwrap();
        let conversations: Value =
            serde_json::from_str(include_str!("../../contracts/conversations.ipc.json")).unwrap();
        let contract = catalog["commands"]
            .get(command)
            .or_else(|| extra["commands"].get(command))
            .or_else(|| conversations["commands"].get(command))
            .ok_or("UNKNOWN_COMMAND")?;
        if !schema::dto("CommandEnvelope", r)
            || !schema::valid(&r["payload"], &contract["payload"])
            || r.to_string().len() > 128 * 1024
        {
            return Err("VALIDATION_ERROR");
        }
        let scoped = contract["scope"] == "profile";
        let profile = r["profileId"].as_str().unwrap_or("");
        if scoped == profile.is_empty() {
            return Err("PROFILE_SCOPE_MISMATCH");
        }
        if contract["requiresExpectedRevision"] == true && r["expectedRevision"].as_i64().is_none()
        {
            return Err("VALIDATION_ERROR");
        }
        let payload = &r["payload"];
        let expected = r["expectedRevision"].as_i64().unwrap_or(0);
        let mut fingerprint_payload = payload.clone();
        fingerprint_payload
            .as_object_mut()
            .unwrap()
            .remove("credential");
        let secret_command =
            command == "connection.create" || command == "connection.credential.set";
        let hmac_message = json!([
            "ai-home-idempotency:v1",
            command,
            profile,
            expected,
            payload
        ])
        .to_string();
        let mut fingerprint = format!(
            "{:x}",
            Sha256::digest(
                json!([command, profile, expected, fingerprint_payload])
                    .to_string()
                    .as_bytes()
            )
        );
        let mut tx = self.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        if contract["mutates"] == true {
            if let Some(row) =
                sqlx::query("SELECT fingerprint,response FROM receipts WHERE request_id=?")
                    .bind(r["requestId"].as_str().unwrap())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|_| "STORAGE_ERROR")?
            {
                if secret_command && row.get::<&str, _>("fingerprint").starts_with("hmac-v1:") {
                    fingerprint = self
                        .secret_fingerprint(&mut tx, &hmac_message, false)
                        .await?;
                }
                if row.get::<&str, _>("fingerprint") != fingerprint {
                    return Err("IDEMPOTENCY_CONFLICT");
                }
                let result: Value =
                    serde_json::from_str(row.get("response")).map_err(|_| "STORAGE_ERROR")?;
                if result["ok"] != true
                    || result["requestId"] != r["requestId"]
                    || !schema::valid(&result["result"], &contract["result"])
                {
                    return Err("STORAGE_ERROR");
                }
                if command == "connection.create" || command == "connection.credential.set" {
                    let c = &result["result"];
                    let secret = self
                        .vault
                        .secret(stamp(c)?, c["credentialRef"].as_str().unwrap().into())
                        .await
                        .map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
                    if payload["credential"]["secret"].as_str() != Some(secret.as_str()) {
                        return Err("IDEMPOTENCY_CONFLICT");
                    }
                }
                return Ok(result);
            }
        }
        let mut launch = None;
        let mut cancel_after = Vec::new();
        let result = match command {
            "profile.create" => {
                let name = payload["displayName"].as_str().unwrap().trim();
                if name.is_empty() || name.len() > 256 {
                    return Err("VALIDATION_ERROR");
                }
                let defaults: Value =
                    serde_json::from_str(include_str!("../../contracts/defaults.json")).unwrap();
                let v = json!({"schemaVersion":1,"id":id(),"revision":1,"createdAt":now(),"displayName":name,"status":"draft","mode":"standard","scribeMode":"automatic","memorySettings":defaults["memorySettings"],"contextBudget":defaults["contextBudget"],"languagePolicy":"preserve_source"});
                put(&mut tx, "profiles", &v, true, 0).await?;
                event(&mut tx, v["id"].as_str().unwrap(), "profile.changed", &v).await?;
                v
            }
            "profile.list" => {
                let limit = payload["limit"].as_i64().unwrap();
                let mut after = String::new();
                if let Some(cursor) = payload["cursor"].as_str() {
                    let row =
                        sqlx::query("SELECT after_id,page_limit FROM profile_cursors WHERE id=?")
                            .bind(cursor)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(|_| "STORAGE_ERROR")?
                            .ok_or("VALIDATION_ERROR")?;
                    if row.get::<i64, _>("page_limit") != limit {
                        return Err("VALIDATION_ERROR");
                    }
                    after = row.get("after_id");
                }
                let rows = sqlx::query("SELECT id FROM profiles WHERE id>? ORDER BY id LIMIT ?")
                    .bind(after)
                    .bind(limit + 1)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|_| "STORAGE_ERROR")?;
                let mut items = vec![];
                for row in rows.iter().take(limit as usize) {
                    items.push(get(&mut tx, "profiles", "", row.get("id")).await?)
                }
                let cursor = if rows.len() > limit as usize {
                    let last = items.last().unwrap()["id"].as_str().unwrap();
                    let token = format!(
                        "{:x}",
                        Sha256::digest(format!("profile-list:v1:{limit}:{last}").as_bytes())
                    );
                    sqlx::query("INSERT OR IGNORE INTO profile_cursors(id,after_id,page_limit) VALUES(?,?,?)").bind(&token).bind(last).bind(limit).execute(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                    token
                } else {
                    String::new()
                };
                json!({"items":items,"nextCursor":cursor})
            }
            _ => {
                let mut p = get(&mut tx, "profiles", "", profile).await?;
                if p["status"] == "archived" {
                    return Err("POLICY_DENIED");
                }
                match command {
                    c if c.starts_with("conversation.") => {
                        conversation_storage::execute(&mut tx, c, profile, payload, expected)
                            .await?
                    }
                    "profile.snapshot" => {
                        let mut all = vec![];
                        for table in ["connections", "routes"] {
                            let rows = sqlx::query(&format!(
                                "SELECT id FROM {table} WHERE profile_id=? ORDER BY id"
                            ))
                            .bind(profile)
                            .fetch_all(&mut *tx)
                            .await
                            .map_err(|_| "STORAGE_ERROR")?;
                            let mut values = vec![];
                            for row in rows {
                                values.push(get(&mut tx, table, profile, row.get("id")).await?)
                            }
                            all.push(values);
                        }
                        let seq:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM profile_events WHERE profile_id=?").bind(profile).fetch_one(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                        json!({"profile":p,"connections":all[0],"routes":all[1],"memoryRecords":[],"lastEventSequence":seq})
                    }
                    "connection.create" => {
                        let draft = &payload["connection"];
                        let endpoint = providers::normalize_endpoint(
                            kind(&draft["providerKind"])?,
                            draft["endpoint"].as_str().unwrap(),
                        )
                        .map_err(|_| "CONNECTION_INVALID")?;
                        let label = draft["label"].as_str().unwrap().trim();
                        if label.is_empty() || label.len() > 256 {
                            return Err("VALIDATION_ERROR");
                        }
                        let v = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"providerKind":draft["providerKind"],"normalizedEndpoint":endpoint,"credentialRef":id(),"credentialRevision":1,"label":label,"status":"unverified"});
                        self.vault
                            .store(
                                stamp(&v)?,
                                v["credentialRef"].as_str().unwrap().into(),
                                payload["credential"]["secret"].as_str().unwrap().into(),
                            )
                            .await
                            .map_err(vault_write_code)?;
                        put(&mut tx, "connections", &v, true, 0).await?;
                        event(&mut tx, profile, "connection.changed", &v).await?;
                        v
                    }
                    "connection.update" | "connection.credential.set" => {
                        let mut c = get(
                            &mut tx,
                            "connections",
                            profile,
                            payload["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        if revision(&c)? != expected {
                            return Err("REVISION_CONFLICT");
                        }
                        bump(&mut c)?;
                        invalidate(&mut c);
                        if command == "connection.update" {
                            let draft = &payload["connection"];
                            c["providerKind"] = draft["providerKind"].clone();
                            c["normalizedEndpoint"] = json!(providers::normalize_endpoint(
                                kind(&draft["providerKind"])?,
                                draft["endpoint"].as_str().unwrap()
                            )
                            .map_err(|_| "CONNECTION_INVALID")?);
                            let label = draft["label"].as_str().unwrap().trim();
                            if label.is_empty() || label.len() > 256 {
                                return Err("VALIDATION_ERROR");
                            }
                            c["label"] = json!(label);
                        } else {
                            let rev = c["credentialRevision"]
                                .as_i64()
                                .unwrap()
                                .checked_add(1)
                                .filter(|n| *n <= 9_007_199_254_740_991)
                                .ok_or("VALIDATION_ERROR")?;
                            c["credentialRevision"] = json!(rev);
                            c["credentialRef"] = json!(id());
                            self.vault
                                .store(
                                    stamp(&c)?,
                                    c["credentialRef"].as_str().unwrap().into(),
                                    payload["credential"]["secret"].as_str().unwrap().into(),
                                )
                                .await
                                .map_err(vault_write_code)?;
                        }
                        put(&mut tx, "connections", &c, false, expected).await?;
                        sqlx::query(
                            "DELETE FROM connection_models WHERE profile_id=? AND connection_id=?",
                        )
                        .bind(profile)
                        .bind(c["id"].as_str().unwrap())
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| "STORAGE_ERROR")?;
                        event(&mut tx, profile, "connection.changed", &c).await?;
                        refresh_ready(&mut tx, &mut p).await?;
                        let pending=sqlx::query("SELECT id FROM operations WHERE profile_id=? AND json_extract(body,'$.recordRefs[0].recordId')=? AND json_extract(body,'$.status') IN ('queued','running')").bind(profile).bind(c["id"].as_str().unwrap()).fetch_all(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                        for row in pending {
                            cancel_after.push(row.get::<String, _>("id"));
                        }
                        c
                    }
                    "connection.test" => {
                        let c = get(
                            &mut tx,
                            "connections",
                            profile,
                            payload["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        if revision(&c)? != expected {
                            return Err("REVISION_CONFLICT");
                        }
                        let op = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"kind":"connection_test","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":expected,"limits":{"maxPhysicalCalls":4,"maxElapsedMs":180000},"recordRefs":[{"profileId":profile,"recordKind":"connection","recordId":c["id"]}]});
                        put(&mut tx, "operations", &op, true, 0).await?;
                        event(&mut tx, profile, "operation.changed", &op).await?;
                        launch = Some((c, op.clone()));
                        json!({"operation":op})
                    }
                    "connection.test.latest" => {
                        let c = get(
                            &mut tx,
                            "connections",
                            profile,
                            payload["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        let row=sqlx::query("SELECT id FROM operations WHERE profile_id=? AND json_extract(body,'$.recordRefs[0].recordId')=? ORDER BY rowid DESC LIMIT 1").bind(profile).bind(c["id"].as_str().unwrap()).fetch_optional(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                        match row {
                            Some(row) => {
                                json!({"found":true,"operation":get(&mut tx,"operations",profile,row.get("id")).await?})
                            }
                            None => json!({"found":false}),
                        }
                    }
                    "connection.models.list" => {
                        let c = get(
                            &mut tx,
                            "connections",
                            profile,
                            payload["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        if !verified(&c) {
                            return Err("CONNECTION_UNVERIFIED");
                        }
                        let row=sqlx::query("SELECT body FROM connection_models WHERE profile_id=? AND connection_id=? AND connection_revision=? AND credential_revision=?").bind(profile).bind(c["id"].as_str().unwrap()).bind(revision(&c)?).bind(c["credentialRevision"].as_i64().unwrap()).fetch_optional(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?.ok_or("CONNECTION_UNVERIFIED")?;
                        json!({"items":serde_json::from_str::<Value>(row.get("body")).map_err(|_|"STORAGE_ERROR")?,"connectionRevision":c["revision"],"credentialRevision":c["credentialRevision"]})
                    }
                    "route.save" => {
                        let draft = &payload["route"];
                        let c = get(
                            &mut tx,
                            "connections",
                            profile,
                            draft["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        if !verified(&c) {
                            return Err("CONNECTION_UNVERIFIED");
                        }
                        if draft.get("reasoningValue").is_some() {
                            return Err("MODEL_CAPABILITY_UNKNOWN");
                        }
                        if draft["modelId"].as_str().unwrap().len() > 512
                            || draft["modelId"]
                                .as_str()
                                .unwrap()
                                .chars()
                                .any(char::is_control)
                            || draft["outputLimitTokens"].as_u64().unwrap() > u32::MAX as u64
                        {
                            return Err("VALIDATION_ERROR");
                        }
                        let mut route_stamp = stamp(&c)?;
                        route_stamp.model_id = draft["modelId"].as_str().unwrap().into();
                        let probe = providers::ProviderChatRequest {
                            provider: route_stamp.provider,
                            model_id: route_stamp.model_id.clone(),
                            scope: route_stamp,
                            messages: vec![providers::ProviderMessage::text(
                                "user",
                                "configuration validation",
                            )],
                            temperature: draft["temperature"].as_f64(),
                            max_output_tokens: Some(
                                draft["outputLimitTokens"].as_u64().unwrap() as u32
                            ),
                            reasoning_effort: None,
                            reasoning_max_tokens: None,
                            reasoning_transport_kind: None,
                            reasoning_transport_value: None,
                            custom_output_token_field: None,
                            provider_routing: None,
                        };
                        providers::prepare_chat(&probe, &probe.scope.normalized_endpoint)
                            .map_err(|_| "VALIDATION_ERROR")?;

                        let old = if let Some(route) = payload["routeId"].as_str() {
                            Some(get(&mut tx, "routes", profile, route).await?)
                        } else {
                            None
                        };
                        if old
                            .as_ref()
                            .map(revision)
                            .transpose()?
                            .unwrap_or(revision(&p)?)
                            != expected
                        {
                            return Err("REVISION_CONFLICT");
                        }
                        let mut v=old.clone().unwrap_or(json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now()}));
                        if old.is_some() {
                            bump(&mut v)?;
                        }
                        for key in ["role", "connectionId", "modelId", "outputLimitTokens"] {
                            v[key] = draft[key].clone();
                        }
                        v.as_object_mut().unwrap().remove("temperature");
                        if let Some(temp) = draft.get("temperature") {
                            v["temperature"] = temp.clone();
                        }
                        if old.as_ref().is_some_and(|o| o["role"] != v["role"]) {
                            return Err("VALIDATION_ERROR");
                        }
                        put(&mut tx, "routes", &v, old.is_none(), expected).await?;
                        let prev = revision(&p)?;
                        bump(&mut p)?;
                        put(&mut tx, "profiles", &p, false, prev).await?;
                        event(&mut tx, profile, "profile.changed", &p).await?;
                        v
                    }
                    "route.assign" => {
                        if revision(&p)? != expected {
                            return Err("REVISION_CONFLICT");
                        }
                        let route = get(
                            &mut tx,
                            "routes",
                            profile,
                            payload["routeId"].as_str().unwrap(),
                        )
                        .await?;
                        if route["role"] != payload["role"] {
                            return Err("VALIDATION_ERROR");
                        }
                        let c = get(
                            &mut tx,
                            "connections",
                            profile,
                            route["connectionId"].as_str().unwrap(),
                        )
                        .await?;
                        if !verified(&c) {
                            return Err("CONNECTION_UNVERIFIED");
                        }
                        let key = if payload["role"] == "primary" {
                            "primaryRouteId"
                        } else {
                            "scribeRouteId"
                        };
                        p[key] = route["id"].clone();
                        bump(&mut p)?;
                        p["status"] = json!(if routes_ready(&mut tx, &p).await? {
                            "ready"
                        } else {
                            "draft"
                        });
                        put(&mut tx, "profiles", &p, false, expected).await?;
                        event(&mut tx, profile, "profile.changed", &p).await?;
                        p
                    }
                    "operation.get" => {
                        get(
                            &mut tx,
                            "operations",
                            profile,
                            payload["operationId"].as_str().unwrap(),
                        )
                        .await?
                    }
                    "operation.cancel" => {
                        let mut op = get(
                            &mut tx,
                            "operations",
                            profile,
                            payload["operationId"].as_str().unwrap(),
                        )
                        .await?;
                        if matches!(op["status"].as_str(), Some("queued") | Some("running")) {
                            if let Some((cancel, budget)) =
                                self.active.lock().await.get(op["id"].as_str().unwrap())
                            {
                                let _ = cancel;
                                op["physicalCallCount"] = json!(budget.physical_calls());
                                cancel_after.push(op["id"].as_str().unwrap().to_string());
                            }
                            let old = revision(&op)?;
                            terminal(&mut op, "cancelled", "OPERATION_CANCELLED")?;
                            put(&mut tx, "operations", &op, false, old).await?;
                            event(&mut tx, profile, "operation.changed", &op).await?;
                        }
                        op
                    }
                    _ => return Err("UNKNOWN_COMMAND"),
                }
            }
        };
        if !schema::valid(&result, &contract["result"]) {
            return Err("STORAGE_ERROR");
        }
        let response = success(r, result);
        if contract["mutates"] == true {
            if secret_command {
                fingerprint = self
                    .secret_fingerprint(&mut tx, &hmac_message, true)
                    .await?;
            }
            sqlx::query("INSERT INTO receipts(request_id,fingerprint,response) VALUES(?,?,?)")
                .bind(r["requestId"].as_str().unwrap())
                .bind(fingerprint)
                .bind(response.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
        }
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
        {
            let active = self.active.lock().await;
            for opid in cancel_after {
                if let Some((cancel, _)) = active.get(&opid) {
                    cancel.cancel();
                }
            }
        }
        if let Some((c, op)) = launch {
            let cancel = Cancellation::default();
            let budget =
                CallBudget::standard(cancel.clone()).with_recorder(Arc::new(RecordAttempt {
                    pool: self.pool.clone(),
                    gate: self.gate.clone(),
                    profile: op["profileId"].as_str().unwrap().into(),
                    operation: op["id"].as_str().unwrap().into(),
                }));
            self.active
                .lock()
                .await
                .insert(op["id"].as_str().unwrap().into(), (cancel, budget.clone()));
            let service = self.clone();
            tokio::spawn(async move {
                let opid = op["id"].as_str().unwrap().to_string();
                let profile = op["profileId"].as_str().unwrap().to_string();
                if service.test(c, op, budget).await.is_err() {
                    let _ = service.fail_operation(&profile, &opid).await;
                }
                service.active.lock().await.remove(&opid);
            });
        }
        Ok(response)
    }
    async fn secret_fingerprint(
        &self,
        tx: &mut Tx<'_>,
        message: &str,
        create: bool,
    ) -> Result<String, Failure> {
        let existing: Option<String> =
            sqlx::query_scalar("SELECT credential_ref FROM installation_hmac WHERE id=1")
                .fetch_optional(&mut **tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
        let is_new = existing.is_none();
        if is_new && !create {
            return Err("CREDENTIAL_UNAVAILABLE");
        }
        let reference = existing.unwrap_or_else(id);
        let digest = self
            .vault
            .installation_fingerprint(reference.clone(), message.into(), is_new && create)
            .await
            .map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
        if is_new {
            sqlx::query("INSERT INTO installation_hmac(id,credential_ref) VALUES(1,?)")
                .bind(reference)
                .execute(&mut **tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
        }
        Ok(digest)
    }
    async fn fail_operation(&self, profile: &str, id: &str) -> Result<(), Failure> {
        let _guard = self.gate.lock().await;
        let mut tx = self.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        let mut op = get(&mut tx, "operations", profile, id).await?;
        if matches!(op["status"].as_str(), Some("queued") | Some("running")) {
            let old = revision(&op)?;
            terminal(&mut op, "failed", "STORAGE_ERROR")?;
            put(&mut tx, "operations", &op, false, old).await?;
            event(&mut tx, profile, "operation.changed", &op).await?;
        }
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
        Ok(())
    }
    async fn test(&self, c: Value, mut op: Value, budget: CallBudget) -> Result<(), Failure> {
        let profile = op["profileId"].as_str().unwrap().to_string();
        let opid = op["id"].as_str().unwrap().to_string();
        {
            let _guard = self.gate.lock().await;
            let mut tx = self.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
            op = get(&mut tx, "operations", &profile, &opid).await?;
            if op["status"] != "queued" {
                return Ok(());
            }
            let rev = revision(&op)?;
            bump(&mut op)?;
            op["status"] = json!("running");
            put(&mut tx, "operations", &op, false, rev).await?;
            event(&mut tx, &profile, "operation.changed", &op).await?;
            tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
        }
        let attempt = async {
            let scope = stamp(&c)?;
            let secret = self
                .vault
                .secret(scope.clone(), c["credentialRef"].as_str().unwrap().into())
                .await
                .map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
            let bound =
                BoundCredential::new(&scope, secret).map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
            let transport = HttpTransport::new().map_err(|_| "PROVIDER_ERROR")?;
            transport
                .discover_models(&scope, &bound, &budget)
                .await
                .map_err(transport_code)
        }
        .await;
        let _guard = self.gate.lock().await;
        let mut tx = self.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        let mut current = get(&mut tx, "operations", &profile, &opid).await?;
        if current["status"] != "running" {
            return Ok(());
        }
        let mut live = get(&mut tx, "connections", &profile, c["id"].as_str().unwrap()).await?;
        let old = revision(&current)?;
        current["physicalCallCount"] = json!(budget.physical_calls());
        if live["revision"] != c["revision"]
            || live["credentialRevision"] != c["credentialRevision"]
            || live["credentialRef"] != c["credentialRef"]
        {
            terminal(&mut current, "failed", "STALE_CONNECTION_TEST")?;
        } else {
            match attempt {
                Ok(models) => {
                    let tested = now();
                    live["status"] = json!("verified");
                    live["verifiedRevision"] = c["revision"].clone();
                    live["verifiedCredentialRevision"] = c["credentialRevision"].clone();
                    live["testedAt"] = json!(tested);
                    let items: Vec<Value> = models
                        .iter()
                        .map(|m| json!({"id":m.id,"label":m.label}))
                        .collect();
                    sqlx::query("INSERT INTO connection_models(profile_id,connection_id,connection_revision,credential_revision,body) VALUES(?,?,?,?,?) ON CONFLICT(profile_id,connection_id) DO UPDATE SET connection_revision=excluded.connection_revision,credential_revision=excluded.credential_revision,body=excluded.body").bind(&profile).bind(c["id"].as_str().unwrap()).bind(revision(&c)?).bind(c["credentialRevision"].as_i64().unwrap()).bind(json!(items).to_string()).execute(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                    bump(&mut current)?;
                    current["status"] = json!("succeeded");
                    current["finishedAt"] = json!(now());
                    current["result"] = json!({"kind":"connection_test","connectionId":c["id"],"connectionRevision":c["revision"],"credentialRevision":c["credentialRevision"],"testedAt":tested,"outcome":"verified"});
                }
                Err(code) => {
                    invalidate(&mut live);
                    live["status"] = json!("error");
                    terminal(
                        &mut current,
                        if code == "OPERATION_CANCELLED" {
                            "cancelled"
                        } else {
                            "failed"
                        },
                        code,
                    )?;
                }
            }
            put(&mut tx, "connections", &live, false, revision(&c)?).await?;
            event(&mut tx, &profile, "connection.changed", &live).await?;
            let mut p = get(&mut tx, "profiles", "", &profile).await?;
            refresh_ready(&mut tx, &mut p).await?;
        }
        put(&mut tx, "operations", &current, false, old).await?;
        event(&mut tx, &profile, "operation.changed", &current).await?;
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
        Ok(())
    }
}
fn terminal(op: &mut Value, status: &str, code: Failure) -> Result<(), Failure> {
    bump(op)?;
    op["status"] = json!(status);
    op["finishedAt"] = json!(now());
    op["error"] = json!({"code":code,"messageKey":format!("error.{}",code.to_lowercase()),"retryable":code!="OPERATION_CANCELLED","operationId":op["id"]});
    Ok(())
}
fn transport_code(e: TransportError) -> Failure {
    match e {
        TransportError::Storage => "STORAGE_ERROR",
        TransportError::Protocol(providers::ProtocolError::OutputIncomplete) => {
            "PROVIDER_OUTPUT_INCOMPLETE"
        }
        TransportError::Protocol(providers::ProtocolError::PolicyBlocked) => "POLICY_DENIED",
        TransportError::Cancelled => "OPERATION_CANCELLED",
        TransportError::Timeout => "PROVIDER_TIMEOUT",
        TransportError::HttpStatus {
            status: 401 | 403, ..
        } => "PROVIDER_AUTHENTICATION_FAILED",
        TransportError::CallLimit | TransportError::DiscoveryIncomplete => {
            "OPERATION_BUDGET_EXCEEDED"
        }
        _ => "PROVIDER_ERROR",
    }
}

async fn routes_ready(tx: &mut Tx<'_>, p: &Value) -> Result<bool, Failure> {
    for (key, role) in [("primaryRouteId", "primary"), ("scribeRouteId", "scribe")] {
        if role == "scribe" && p["scribeMode"] == "disabled" {
            continue;
        }
        let Some(routeid) = p[key].as_str() else {
            return Ok(false);
        };
        let route = get(tx, "routes", p["id"].as_str().unwrap(), routeid).await?;
        if route["role"] != role {
            return Err("STORAGE_ERROR");
        }
        let c = get(
            tx,
            "connections",
            p["id"].as_str().unwrap(),
            route["connectionId"].as_str().unwrap(),
        )
        .await?;
        if !verified(&c) {
            return Ok(false);
        }
    }
    Ok(true)
}
async fn refresh_ready(tx: &mut Tx<'_>, p: &mut Value) -> Result<(), Failure> {
    let status = if routes_ready(tx, p).await? {
        "ready"
    } else {
        "draft"
    };
    if p["status"] != status {
        let old = revision(p)?;
        p["status"] = json!(status);
        bump(p)?;
        put(tx, "profiles", p, false, old).await?;
        event(tx, p["id"].as_str().unwrap(), "profile.changed", p).await?;
    }
    Ok(())
}

struct RecordAttempt {
    pool: SqlitePool,
    gate: Arc<Mutex<()>>,
    profile: String,
    operation: String,
}
impl providers::transport::CallRecorder for RecordAttempt {
    fn reserved(
        &self,
        count: u8,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            let _guard = self.gate.lock().await;
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|_| TransportError::Storage)?;
            let mut op = get(&mut tx, "operations", &self.profile, &self.operation)
                .await
                .map_err(|_| TransportError::Storage)?;
            if op["status"] != "running" {
                return Err(TransportError::Cancelled);
            }
            if count == 0 || u64::from(count) > op["limits"]["maxPhysicalCalls"].as_u64().unwrap() {
                return Err(TransportError::CallLimit);
            }
            if u64::from(count) > op["physicalCallCount"].as_u64().unwrap() {
                let old = revision(&op).map_err(|_| TransportError::Storage)?;
                bump(&mut op).map_err(|_| TransportError::Storage)?;
                op["physicalCallCount"] = json!(count);
                put(&mut tx, "operations", &op, false, old)
                    .await
                    .map_err(|_| TransportError::Storage)?;
                event(&mut tx, &self.profile, "operation.changed", &op)
                    .await
                    .map_err(|_| TransportError::Storage)?;
            }
            tx.commit().await.map_err(|_| TransportError::Storage)?;
            Ok(())
        })
    }
}

fn vault_write_code(error: crate::vault::VaultError) -> Failure {
    match error {
        crate::vault::VaultError::Invalid => "VALIDATION_ERROR",
        _ => "CREDENTIAL_UNAVAILABLE",
    }
}
