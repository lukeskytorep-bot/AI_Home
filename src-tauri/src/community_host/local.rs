//! Local registration command adapter. Native caller authorization is a separate
//! argument, never a JSON field. Production Tauri and Experimental are disabled.
use super::CommunityLifecycle;
use crate::{
    application::Application,
    community_identity::{
        registry::{lifecycle, Preparation, PreparationState, RegistryError},
        CommunityScope, RegistrationProfile,
    },
    domain::schema,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, Transaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalCapability {
    Status,
    Prepare,
    Provision,
    Recover,
}
/// Supplied afresh by the trusted native caller, not deserializable. This is an
/// explicit host grant; it is not inferred from remote capability advertisements.
#[derive(Clone)]
pub struct LocalAccess {
    scope: CommunityScope,
    capabilities: Vec<LocalCapability>,
}
impl LocalAccess {
    pub fn new(scope: CommunityScope, capabilities: Vec<LocalCapability>) -> Self {
        Self {
            scope,
            capabilities,
        }
    }
}
fn failure(r: &Value, code: &str) -> Value {
    json!({"contractVersion":1,"requestId":r["requestId"].as_str().filter(|s|!s.is_empty()&&s.chars().count()<=128).unwrap_or("invalid-request"),"ok":false,"error":{"code":code,"messageKey":format!("error.{}",code.to_lowercase()),"retryable":false}})
}
fn success(r: &Value, result: Value) -> Value {
    json!({"contractVersion":1,"requestId":r["requestId"],"ok":true,"result":result})
}
fn project(record: Option<Preparation>) -> Value {
    let (prepared, rev, state) = match record {
        None => (false, 0, "absent"),
        Some(p) => (
            true,
            p.revision(),
            match p.state() {
                PreparationState::Reserved => "reserved",
                PreparationState::Provisioning => "provisioning",
                PreparationState::Ready => "ready",
            },
        ),
    };
    json!({"prepared":prepared,"registrationRevision":rev,"preparationState":state,"onlineStatusKnown":false})
}
fn valid_result(v: &Value, contract: &Value) -> bool {
    schema::valid(v, &contract["result"])
        && if v["prepared"] == false {
            v["registrationRevision"] == 0 && v["preparationState"] == "absent"
        } else {
            v["registrationRevision"].as_i64().is_some_and(|n| n > 0)
                && v["preparationState"] != "absent"
        }
}
fn terminal(r: &Value, v: &Value, contract: &Value) -> Result<Value, &'static str> {
    let object = v.as_object().ok_or("STORAGE_ERROR")?;
    if object.len() != 4 || v["contractVersion"] != 1 || v["requestId"] != r["requestId"] {
        return Err("STORAGE_ERROR");
    }
    match v["ok"].as_bool() {
        Some(true) if valid_result(&v["result"], contract) => Ok(success(r, v["result"].clone())),
        Some(false) => {
            let error = v["error"]
                .as_object()
                .filter(|e| e.len() == 3)
                .ok_or("STORAGE_ERROR")?;
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .ok_or("STORAGE_ERROR")?;
            if ![
                "NOT_FOUND",
                "REVISION_CONFLICT",
                "POLICY_DENIED",
                "REGISTRATION_RECONCILIATION_REQUIRED",
                "CREDENTIAL_UNAVAILABLE",
                "RESOURCE_UNAVAILABLE",
                "STORAGE_ERROR",
            ]
            .contains(&code)
                || error.get("retryable") != Some(&json!(false))
                || error.get("messageKey") != Some(&json!(format!("error.{}", code.to_lowercase())))
            {
                return Err("STORAGE_ERROR");
            }
            Ok(failure(r, code))
        }
        _ => Err("STORAGE_ERROR"),
    }
}
fn safe_registry(e: RegistryError) -> &'static str {
    match e {
        RegistryError::NotFound => "NOT_FOUND",
        RegistryError::Conflict => "REVISION_CONFLICT",
        RegistryError::PolicyDenied => "POLICY_DENIED",
        RegistryError::ProvisioningUncertain => "REGISTRATION_RECONCILIATION_REQUIRED",
        RegistryError::Identity(_) => "CREDENTIAL_UNAVAILABLE",
        RegistryError::Busy => "RESOURCE_UNAVAILABLE",
        _ => "STORAGE_ERROR",
    }
}
async fn authorize(
    tx: &mut Transaction<'_, Sqlite>,
    r: &Value,
    a: &LocalAccess,
) -> Result<(), &'static str> {
    let id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
    if id != a.scope.profile_id() {
        return Err("POLICY_DENIED");
    }
    let row = sqlx::query("SELECT revision,body FROM profiles WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
        .ok_or("NOT_FOUND")?;
    let body: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
    if !schema::dto("Profile", &body)
        || body["id"] != id
        || body["revision"].as_i64() != Some(row.get("revision"))
    {
        return Err("STORAGE_ERROR");
    }
    if body["revision"] != r["expectedRevision"] {
        return Err("REVISION_CONFLICT");
    }
    let bound: i64 =
        sqlx::query_scalar("SELECT count(*) FROM experimental_instances WHERE profile_id=?")
            .bind(id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    if bound != 0 || body["mode"] != "standard" || body["status"] == "archived" {
        return Err("POLICY_DENIED");
    }
    if crate::connections::profile_deletion::pending(tx, id).await? {
        return Err("POLICY_DENIED");
    }
    if sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profile_deletion_jobs WHERE request_id=?")
        .bind(r["requestId"].as_str().unwrap())
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
        != 0
    {
        return Err("IDEMPOTENCY_CONFLICT");
    }
    Ok(())
}

pub(crate) async fn dispatch(
    app: &Application,
    service: &CommunityLifecycle,
    request: Value,
    access: LocalAccess,
) -> Value {
    let r = &request;
    match execute(app, service, r, &access).await {
        Ok(value) => value,
        Err(code) => failure(r, code),
    }
}
async fn execute(
    app: &Application,
    service: &CommunityLifecycle,
    r: &Value,
    a: &LocalAccess,
) -> Result<Value, &'static str> {
    if r["contractVersion"] != 1 {
        return Err("UNSUPPORTED_CONTRACT_VERSION");
    }
    if !schema::dto("CommandEnvelope", r) || r["expectedRevision"].as_i64().is_none() {
        return Err("VALIDATION_ERROR");
    }
    let command = r["command"].as_str().unwrap();
    let cap = match command {
        "community.registration.status" => LocalCapability::Status,
        "community.registration.prepare" => LocalCapability::Prepare,
        "community.registration.provision" => LocalCapability::Provision,
        "community.registration.recover" => LocalCapability::Recover,
        _ => return Err("UNKNOWN_COMMAND"),
    };
    let contracts: Value =
        serde_json::from_str(include_str!("../../../contracts/community.local.ipc.json"))
            .map_err(|_| "STORAGE_ERROR")?;
    let contract = &contracts["commands"][command];
    if !schema::valid(&r["payload"], &contract["payload"]) {
        return Err("VALIDATION_ERROR");
    }
    let public_profile = if cap == LocalCapability::Prepare {
        Some(
            RegistrationProfile::new(
                r["payload"]["displayName"].as_str().unwrap(),
                r["payload"]["about"].as_str().unwrap(),
            )
            .map_err(|_| "VALIDATION_ERROR")?,
        )
    } else {
        None
    };
    if !a.capabilities.contains(&cap) {
        return Err("POLICY_DENIED");
    }
    let mut tx = app
        .community_pool()
        .begin()
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    authorize(&mut tx, r, a).await?;
    let mutates = cap != LocalCapability::Status;
    crate::connections::profile_deletion::bind_host(&mut tx, Some(service), command, false).await?;
    if service.registry.restore_fence().exists() {
        return Err("POLICY_DENIED");
    }
    if let Some(state) = lifecycle::state(&service.registry, a.scope.profile_id())
        .await
        .map_err(|_| "STORAGE_ERROR")?
    {
        if state != "active" {
            return Err("POLICY_DENIED");
        }
    }
    if !mutates {
        let record = match service.registry.get(&a.scope).await {
            Ok(p) => Some(p),
            Err(RegistryError::NotFound) => None,
            Err(e) => return Err(safe_registry(e)),
        };
        let result = project(record);
        if !valid_result(&result, contract) {
            return Err("STORAGE_ERROR");
        }
        return Ok(success(r, result));
    }
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            json!([
                "community-local/1",
                command,
                r["profileId"],
                r["expectedRevision"],
                r["payload"],
                a.scope.binding()
            ])
            .to_string()
            .as_bytes()
        )
    );
    if let Some(row) = sqlx::query("SELECT fingerprint,response FROM receipts WHERE request_id=?")
        .bind(r["requestId"].as_str().unwrap())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
    {
        if row.get::<&str, _>("fingerprint") != fingerprint {
            return Err("IDEMPOTENCY_CONFLICT");
        }
        let response: Value =
            serde_json::from_str(row.get("response")).map_err(|_| "STORAGE_ERROR")?;
        // An unfinished claim never replays key generation automatically.
        if response.as_object().is_some_and(|o| o.len() == 1)
            && response["communityLocalPending"] == true
        {
            if cap != LocalCapability::Prepare {
                return Err("REGISTRATION_RECONCILIATION_REQUIRED");
            }
        } else {
            return terminal(r, &response, contract);
        }
    } else {
        crate::connections::profile_deletion::bind_host(&mut tx, Some(service), command, true)
            .await?;
        sqlx::query("INSERT INTO receipts(request_id,fingerprint,response) VALUES(?,?,?)")
            .bind(r["requestId"].as_str().unwrap())
            .bind(&fingerprint)
            .bind(json!({"communityLocalPending":true}).to_string())
            .execute(&mut *tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    }
    // Persist Main ownership/pin and request reservation BEFORE registry/vault
    // writes. Backup captures this claim and normal IPC cannot steal its ID.
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    let mut tx = app
        .community_pool()
        .begin()
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    authorize(&mut tx, r, a).await?;
    crate::connections::profile_deletion::bind_host(&mut tx, Some(service), command, false).await?;
    // Retain a Main write reservation through the separate registry/key write;
    // an independently opened cooperating Store cannot archive/change direction.
    sqlx::query("UPDATE profiles SET revision=revision WHERE id=?")
        .bind(a.scope.profile_id())
        .execute(&mut *tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    let outcome = match cap {
        LocalCapability::Prepare => {
            let public = public_profile.as_ref().ok_or("VALIDATION_ERROR")?;
            service
                .registry
                .prepare(
                    &a.scope,
                    public,
                    r["payload"]["clientStartId"].as_str().unwrap(),
                )
                .await
        }
        LocalCapability::Provision => {
            service
                .registry
                .provision(
                    &service.keys,
                    &a.scope,
                    r["payload"]["registrationRevision"].as_i64().unwrap(),
                )
                .await
        }
        LocalCapability::Recover => {
            service
                .registry
                .recover(
                    &service.keys,
                    &a.scope,
                    r["payload"]["registrationRevision"].as_i64().unwrap(),
                )
                .await
        }
        LocalCapability::Status => unreachable!(),
    };
    let response = match outcome {
        Ok(record) => {
            let result = project(Some(record));
            if !valid_result(&result, contract) {
                return Err("STORAGE_ERROR");
            }
            success(r, result)
        }
        Err(e) => failure(r, safe_registry(e)),
    };
    let finalized =
        sqlx::query("UPDATE receipts SET response=? WHERE request_id=? AND fingerprint=?")
            .bind(response.to_string())
            .bind(r["requestId"].as_str().unwrap())
            .bind(fingerprint)
            .execute(&mut *tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?
            .rows_affected();
    if finalized != 1 {
        return Err("STORAGE_ERROR");
    }
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(response)
}
