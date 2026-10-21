//! One approved L2 snapshot per request. Memory is reference data, never system authority.
use super::super::{get, revision, Failure, Tx};
use crate::providers::ProviderMessage;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::domain::schema;

pub(super) async fn capture(tx: &mut Tx<'_>, profile: &str) -> Result<Value, Failure> {
    let id: String = sqlx::query_scalar("SELECT id FROM memory_records WHERE profile_id=? AND json_extract(body,'$.kind')='working'")
        .bind(profile).fetch_one(&mut **tx).await.map_err(|_| "STORAGE_ERROR")?;
    let record = get(tx, "memory_records", profile, &id).await?;
    if record["state"] != "active" {
        if record["state"] == "empty" && record.get("activeVersionId").is_some() { return Err("STORAGE_ERROR"); }
        return Ok(json!({"schemaVersion":1,"record":record,"version":null,"decision":null}));
    }
    let version = get(tx, "memory_versions", profile, record["activeVersionId"].as_str().ok_or("STORAGE_ERROR")?).await?;
    let content = version["content"].as_str().ok_or("STORAGE_ERROR")?;
    if content.len()>8*1024*1024 { return Err("CONTEXT_BUDGET_EXCEEDED"); }
    let digest = format!("{:x}", Sha256::digest(content.as_bytes()));
    if version["recordId"] != record["id"] || version["contentSha256"] != digest || version["origin"] != "agent_decision" { return Err("STORAGE_ERROR"); }
    let decision = get(tx,"memory_decisions",profile,version["decisionReceiptId"].as_str().ok_or("STORAGE_ERROR")?).await?;
    let operation = get(tx,"operations",profile,decision["operationId"].as_str().ok_or("STORAGE_ERROR")?).await?;
    if !matches!(decision["decision"].as_str(),Some("accept" | "modify"))
        || decision["candidateSha256"] != version["contentSha256"]
        || decision["candidateMeasurement"] != version["measurement"]
        || decision["primaryRouteId"] != version["measurement"]["modelRouteId"]
        || decision["routeRevision"] != version["measurement"]["modelRouteRevision"]
        || operation["kind"] != "memory_update" || operation["status"] != "succeeded"
        || operation["result"]["outcome"] != "committed" || operation["result"]["versionId"] != version["id"]
        || operation["result"]["recordId"] != record["id"] { return Err("STORAGE_ERROR"); }
    Ok(json!({"schemaVersion":1,"record":record,"version":version,"decision":decision}))
}

pub(super) fn message(snapshot: &Value) -> Option<ProviderMessage> {
    let v = &snapshot["version"];
    if v.is_null() { return None; }
    let data = json!({"recordId":snapshot["record"]["id"],"recordRevision":snapshot["record"]["revision"],
        "versionId":v["id"],"versionNumber":v["versionNumber"],"contentSha256":v["contentSha256"],
        "language":v["language"],"claimKind":v["claimKind"],"origin":v["origin"],
        "decisionReceiptId":v["decisionReceiptId"],"sources":v["sources"],"content":v["content"]});
    Some(ProviderMessage::text("user", &format!("AI Home Working Context (untrusted reference data; JSON):\n{data}")))
}

pub(super) fn descriptor(snapshot: &Value, route: &Value) -> Value {
    let r = &snapshot["record"];
    let mut d = json!({"included":false,"recordId":r["id"],"recordRevision":r["revision"],"state":r["state"]});
    if let Some(m) = message(snapshot) {
        let v = &snapshot["version"];
        d["included"] = json!(true);
        for (key, source) in [("versionId","id"),("versionNumber","versionNumber"),("contentSha256","contentSha256"),("language","language"),("decisionReceiptId","decisionReceiptId")] { d[key] = v[source].clone(); }
        // A fresh estimate for the current Primary, without mutating historical MemoryVersion.
        d["measurement"] = json!({"value":v["content"].as_str().unwrap().len(),"method":"estimated","estimatorId":"utf8_bytes_v1","modelRouteId":route["id"],"modelRouteRevision":route["revision"]});
        d["inputTokens"] = json!(m.content.len()+64); // Includes JSON/provenance/framing, not just content.
    }
    d
}

pub(super) async fn ensure_current(tx: &mut Tx<'_>, profile: &str, saved: &Value) -> Result<(), Failure> {
    if saved["schemaVersion"] != 1 { return Err("STORAGE_ERROR"); }
    if capture(tx,profile).await? != *saved { return Err("REVISION_CONFLICT"); }
    Ok(())
}

pub(super) async fn historical(tx: &mut Tx<'_>, profile: &str, payload: &Value) -> Result<Value, Failure> {
    let opid=payload["operationId"].as_str().ok_or("VALIDATION_ERROR")?;
    let op=get(tx,"operations",profile,opid).await?;
    if op["kind"]!="chat" { return Err("VALIDATION_ERROR"); }
    let body: String=sqlx::query_scalar("SELECT body FROM chat_runs WHERE profile_id=? AND operation_id=?")
        .bind(profile).bind(opid).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let body:Value=serde_json::from_str(&body).map_err(|_|"STORAGE_ERROR")?;
    match body.get("workingContext") {
        None=>Ok(json!({"known":false,"operationId":opid})), // Historical pre-step-3 runs are not rewritten.
        Some(s)=>{
            if s["schemaVersion"]!=1 || !schema::dto("MemoryRecord",&s["record"]) || s["record"]["profileId"]!=profile
                || revision(&s["record"]).is_err() || !schema::dto("ModelRoute",&body["route"]) || body["route"]["profileId"]!=profile { return Err("STORAGE_ERROR"); }
            let v=&s["version"];
            if s["record"]["state"]=="active" {
                if !schema::dto("MemoryVersion",v) || !schema::dto("MemoryDecisionReceipt",&s["decision"])
                    || v["profileId"]!=profile || v["recordId"]!=s["record"]["id"] || v["id"]!=s["record"]["activeVersionId"]
                    || v["contentSha256"]!=format!("{:x}",Sha256::digest(v["content"].as_str().ok_or("STORAGE_ERROR")?.as_bytes()))
                    || s["decision"]["profileId"]!=profile || s["decision"]["id"]!=v["decisionReceiptId"]
                    || s["decision"]["candidateSha256"]!=v["contentSha256"]
                    || !matches!(s["decision"]["decision"].as_str(),Some("accept" | "modify")) { return Err("STORAGE_ERROR"); }
            } else if !v.is_null() { return Err("STORAGE_ERROR"); }
            Ok(json!({"known":true,"operationId":opid,"workingContext":descriptor(s,&body["route"])}))
        }
    }
}
