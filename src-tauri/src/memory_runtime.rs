//! Native Working Context coordinator. UI can request work, never supply a decision.
use super::{
    bump, event, get, id, kind, now, put, revision, stamp, terminal, transport_code, verified,
    Connections, Failure, Tx,
};
use crate::{
    memory::{self, Engine, Outcome, Stage},
    providers::{
        self,
        transport::{BoundCredential, CallBudget, HttpTransport},
        ProviderChatRequest, ProviderMessage,
    },
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
#[path="memory_restore.rs"] mod restore_runtime;
pub(super) async fn restore(tx:&mut Tx<'_>,p:&Value,payload:&Value,expected:i64)->Result<(Job,Value),Failure>{restore_runtime::accept(tx,p,payload,expected).await}
fn digest(v: &str) -> String {
    format!("{:x}", Sha256::digest(v.as_bytes()))
}
fn hash(v: &Value) -> String {
    digest(&v.to_string())
}
pub(super) async fn seed(tx: &mut Tx<'_>, profile: &str) -> Result<(), Failure> {
    let r = json!({"schemaVersion":1,"id":format!("working-{profile}"),"profileId":profile,"revision":1,"createdAt":now(),"kind":"working","state":"empty"});
    put(tx, "memory_records", &r, true, 0).await
}
pub(super) async fn records(tx: &mut Tx<'_>, profile: &str) -> Result<Vec<Value>, Failure> {
    let rows = sqlx::query("SELECT id FROM memory_records WHERE profile_id=? ORDER BY id")
        .bind(profile)
        .fetch_all(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    let mut all = vec![];
    for r in rows {
        all.push(get(tx, "memory_records", profile, r.get("id")).await?);
    }
    Ok(all)
}
async fn record(tx: &mut Tx<'_>, p: &str) -> Result<Value, Failure> {
    let id:String=sqlx::query_scalar("SELECT id FROM memory_records WHERE profile_id=? AND json_extract(body,'$.kind')='working'").bind(p).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    get(tx, "memory_records", p, &id).await
}
async fn version(tx: &mut Tx<'_>, record: &Value) -> Result<Option<Value>, Failure> {
    if let Some(v) = record["activeVersionId"].as_str() {
        let v = get(
            tx,
            "memory_versions",
            record["profileId"].as_str().unwrap(),
            v,
        )
        .await?;
        if v["recordId"] != record["id"]
            || v["contentSha256"] != digest(v["content"].as_str().unwrap())
        {
            return Err("STORAGE_ERROR");
        }
        Ok(Some(v))
    } else if record["state"] == "empty" {
        Ok(None)
    } else {
        Err("STORAGE_ERROR")
    }
}
fn route_scope(r: &Value, c: &Value) -> String {
    hash(&json!(["ai-home-memory-route:v1", r, c]))
}
async fn routes(tx: &mut Tx<'_>, p: &Value) -> Result<Value, Failure> {
    let profile = p["id"].as_str().unwrap();
    let mut all = json!({});
    for (role, key) in [("primary", "primaryRouteId"), ("scribe", "scribeRouteId")] {
        if let Some(id) = p[key].as_str() {
            let r = get(tx, "routes", profile, id).await?;
            if r["role"] != role {
                return Err("STORAGE_ERROR");
            }
            let mut c = get(
                tx,
                "connections",
                profile,
                r["connectionId"].as_str().unwrap(),
            )
            .await?;
            c.as_object_mut().unwrap().remove("testedAt"); // Retesting identical configuration does not change its identity.
            let sid = route_scope(&r, &c);
            let own:Option<i64>=sqlx::query_scalar("SELECT context_tokens FROM memory_contexts WHERE profile_id=? AND route_id=? AND scope_id=?").bind(profile).bind(id).bind(&sid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
            let context = own.unwrap_or(0);
            all[role] = json!({"route":r,"connection":c,"contextTokens":context,"scopeId":sid});
        }
    }
    Ok(all)
}
fn config_scope(p: &Value, routes: &Value) -> String {
    hash(&json!(["ai-home-memory-settings:v1", p, routes]))
}
async fn items(
    tx: &mut Tx<'_>,
    table: &str,
    p: &str,
    operation: &str,
) -> Result<Vec<Value>, Failure> {
    let rows=sqlx::query(&format!("SELECT id FROM {table} WHERE profile_id=? AND json_extract(body,'$.operationId')=? ORDER BY rowid")).bind(p).bind(operation).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut values = vec![];
    for row in rows {
        values.push(get(tx, table, p, row.get("id")).await?);
    }
    Ok(values)
}
pub(super) async fn view(tx: &mut Tx<'_>, profile: &str) -> Result<Value, Failure> {
    let r = record(tx, profile).await?;
    record_view(tx, profile, r).await
}
pub(super) async fn record_view(tx:&mut Tx<'_>,profile:&str,r:Value)->Result<Value,Failure>{
    let p=get(tx,"profiles","",profile).await?;
    let rs=routes(tx,&p).await?;
    let v = version(tx, &r).await?;
    let mut result = json!({"record":r,"profileRevision":p["revision"],"capacityPolicy":p["memorySettings"][r["kind"].as_str().ok_or("STORAGE_ERROR")?],"scribeMode":p["scribeMode"],"scopeId":config_scope(&p,&rs),"attempts":[],"proposals":[],"decisions":[],"formatRepairCount":0});
    if let Some(v) = v {
        result["version"] = v;
    }
    for role in ["primary", "scribe"] {
        if let Some(b) = rs.get(role) {
            result[role] = json!({"id":b["route"]["id"],"revision":b["route"]["revision"],"modelId":b["route"]["modelId"],"scopeId":b["scopeId"],"contextTokens":b["contextTokens"],"verified":verified(&b["connection"])});
        }
    }
    if let Some(row)=sqlx::query("SELECT operation_id,body FROM memory_runs WHERE profile_id=? AND record_id=? ORDER BY rowid DESC LIMIT 1").bind(profile).bind(r["id"].as_str().unwrap()).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?{
  let opid=row.get::<&str,_>("operation_id");let body:Value=serde_json::from_str(row.get("body")).map_err(|_|"STORAGE_ERROR")?;
  result["latestOperation"]=get(tx,"operations",profile,opid).await?;
  result["formatRepairCount"]=body["formatRepairCount"].clone();
  for(key,table)in [("proposals","memory_proposals"),("attempts","memory_attempts"),("decisions","memory_decisions")] {result[key]=json!(items(tx,table,profile,opid).await?);}
 }
    Ok(result)
}
pub(super) async fn settings(
    tx: &mut Tx<'_>,
    p: &mut Value,
    payload: &Value,
    expected: i64,
) -> Result<Value, Failure> {
    let profile = p["id"].as_str().unwrap().to_string();
    let rs = routes(tx, p).await?;
    if revision(p)? != expected || payload["scopeId"] != config_scope(p, &rs) {
        return Err("REVISION_CONFLICT");
    }
    for (role, key) in [
        ("primary", "primaryContextTokens"),
        ("scribe", "scribeContextTokens"),
    ] {
        if role == "scribe" && payload["scribeMode"] == "disabled" {
            continue;
        }
        let b = rs.get(role).ok_or("MODEL_CAPABILITY_UNKNOWN")?;
        if !verified(&b["connection"]) {
            return Err("CONNECTION_UNVERIFIED");
        }
        let n = payload[key].as_i64().ok_or("VALIDATION_ERROR")?;
        sqlx::query("INSERT INTO memory_contexts(profile_id,route_id,scope_id,context_tokens) VALUES(?,?,?,?) ON CONFLICT(profile_id,route_id) DO UPDATE SET scope_id=excluded.scope_id,context_tokens=excluded.context_tokens")
   .bind(&profile).bind(b["route"]["id"].as_str().unwrap()).bind(b["scopeId"].as_str().unwrap()).bind(n).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    }
    p["memorySettings"]["working"] = payload["capacityPolicy"].clone();
    p["scribeMode"] = payload["scribeMode"].clone();
    p["status"] = json!(if super::routes_ready(tx, p).await? {
        "ready"
    } else {
        "draft"
    });
    bump(p)?;
    put(tx, "profiles", p, false, expected).await?;
    event(tx, &profile, "profile.changed", p).await?;
    view(tx, &profile).await
}
pub(super) async fn sources(
    tx: &mut Tx<'_>,
    profile: &str,
    payload: &Value,
) -> Result<Value, Failure> {
    let c = get(
        tx,
        "conversations",
        profile,
        payload["conversationId"].as_str().unwrap(),
    )
    .await?;
    if c["status"] != "open" {
        return Err("POLICY_DENIED");
    }
    let total:i64=sqlx::query_scalar("SELECT COALESCE(SUM(length(CAST(body AS BLOB))),0) FROM messages WHERE profile_id=? AND conversation_id=?").bind(profile).bind(c["id"].as_str().unwrap()).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if total > 8 * 1024 * 1024 {
        return Err("CONTEXT_BUDGET_EXCEEDED");
    }
    let rows = sqlx::query(
        "SELECT id FROM messages WHERE profile_id=? AND conversation_id=? ORDER BY sequence",
    )
    .bind(profile)
    .bind(c["id"].as_str().unwrap())
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    let mut refs = vec![];
    for row in rows {
        let m = get(tx, "messages", profile, row.get("id")).await?;
        if m["status"] != "complete" {
            return Err("POLICY_DENIED");
        }
        refs.push(json!({"profileId":profile,"kind":"message","entityId":m["id"],"contentSha256":digest(m["content"].as_str().unwrap())}));
    }
    let r = record(tx, profile).await?;
    Ok(json!({"recordId":r["id"],"recordRevision":r["revision"],"sourceRefs":refs}))
}
async fn resolve(
    tx: &mut Tx<'_>,
    profile: &str,
    refs: &Value,
) -> Result<(Vec<Value>, Option<Value>), Failure> {
    let list = refs.as_array().ok_or("VALIDATION_ERROR")?;
    if list.is_empty() {return Err("VALIDATION_ERROR");}
    let mode=list.first().and_then(|s|s["kind"].as_str()).ok_or("VALIDATION_ERROR")?;
    if !matches!(mode,"message"|"import")||list.iter().any(|s|s["kind"]!=mode){return Err("POLICY_DENIED")}
    let mut seen = std::collections::HashSet::new();let mut messages = vec![];let mut conversation: Option<Value> = None;let mut bytes = 0usize;
    for s in list {
        if s["profileId"] != profile {return Err("PROFILE_SCOPE_MISMATCH");}
        let id=s["entityId"].as_str().ok_or("VALIDATION_ERROR")?;if !seen.insert(id){return Err("VALIDATION_ERROR")}
        if mode=="message"{
            if s.get("snapshotBlobId").is_some(){return Err("POLICY_DENIED")}
            let m=get(tx,"messages",profile,id).await?;
            if m["status"]!="complete"||!matches!(m["role"].as_str(),Some("human"|"assistant")){return Err("POLICY_DENIED")}
            if s["contentSha256"]!=digest(m["content"].as_str().unwrap()){return Err("REVISION_CONFLICT")}
            let c=get(tx,"conversations",profile,m["conversationId"].as_str().unwrap()).await?;if c["status"]!="open"{return Err("POLICY_DENIED")}
            if conversation.as_ref().is_some_and(|v|v["id"]!=c["id"]){return Err("VALIDATION_ERROR")}
            bytes=bytes.checked_add(m.to_string().len()).ok_or("CONTEXT_BUDGET_EXCEEDED")?;if bytes>8*1024*1024{return Err("CONTEXT_BUDGET_EXCEEDED")}
            conversation=Some(c);messages.push(m);
        }else{
            let m=super::import_runtime::material(tx,profile,id).await?;
            if m["purpose"]!="memory_import"||m["mediaKind"]!="document"||m["parseState"]!="ready"||m.get("conversationId").is_some(){return Err("POLICY_DENIED")}
            let blob_id=m["extractedBlobId"].as_str().ok_or("STORAGE_ERROR")?;let sha=m["extractedSha256"].as_str().ok_or("STORAGE_ERROR")?;
            if s["contentSha256"]!=sha||s["snapshotBlobId"].as_str()!=Some(blob_id){return Err("REVISION_CONFLICT")}
            let raw=super::import_runtime::blob(tx,profile,blob_id,sha).await?;let content=String::from_utf8(raw).map_err(|_|"STORAGE_ERROR")?;
            bytes=bytes.checked_add(content.len()).ok_or("CONTEXT_BUDGET_EXCEEDED")?;if bytes>8*1024*1024{return Err("CONTEXT_BUDGET_EXCEEDED")}
            messages.push(json!({"schemaVersion":1,"id":id,"profileId":profile,"revision":m["revision"],"createdAt":m["importedAt"],"conversationId":"import-source","sequence":0,"role":"human","content":content,"language":"und","status":"complete","sourceRefs":[s]}));
        }
    }
    Ok((messages, conversation))
}
pub(super) struct Job {
    engine: Engine,
    snapshot: Value,
}
fn engine_route(b: &Value) -> Result<memory::Route, Failure> {
    Ok(memory::Route {
        id: b["route"]["id"]
            .as_str()
            .ok_or("MODEL_CAPABILITY_UNKNOWN")?
            .into(),
        revision: b["route"]["revision"].as_u64().unwrap(),
        scope_id: b["scopeId"].as_str().unwrap().into(),
    })
}
fn build(prompt: &memory::Prompt, b: &Value, p: &Value) -> Result<ProviderChatRequest, Failure> {
    let r = &b["route"];
    let c = &b["connection"];
    let mut scope = stamp(c)?;
    scope.route_id = r["id"].as_str().unwrap().into();
    scope.route_revision = r["revision"].as_u64().unwrap();
    scope.model_id = r["modelId"].as_str().unwrap().into();
    let output = r["outputLimitTokens"].as_u64().unwrap().min(
        p["contextBudget"]["memoryOutputCeilingTokens"]
            .as_u64()
            .unwrap(),
    );
    if output == 0 || output > u32::MAX as u64 {
        return Err("OUTPUT_BUDGET_INSUFFICIENT");
    }
    let messages = vec![
        ProviderMessage::text("system", &prompt.system),
        ProviderMessage::text("user", &prompt.data.to_string()),
    ];
    let size: usize = messages.iter().map(|m| m.content.len() + 64).sum();
    let total = size as u64 + output + p["contextBudget"]["inputReserveTokens"].as_u64().unwrap();
    if size > 8 * 1024 * 1024
        || total
            > b["contextTokens"]
                .as_u64()
                .ok_or("MODEL_CAPABILITY_UNKNOWN")?
    {
        return Err("CONTEXT_BUDGET_EXCEEDED");
    }
    let request = ProviderChatRequest {
        provider: kind(&c["providerKind"])?,
        model_id: scope.model_id.clone(),
        scope,
        messages,
        temperature: r["temperature"].as_f64(),
        max_output_tokens: Some(output as u32),
        reasoning_effort: None,
        reasoning_max_tokens: None,
        reasoning_transport_kind: None,
        reasoning_transport_value: None,
        custom_output_token_field: None,
        provider_routing: None,
    };
    let wire = providers::prepare_chat(&request, c["normalizedEndpoint"].as_str().unwrap())
        .map_err(|_| "VALIDATION_ERROR")?;
    if wire.body.to_string().len() > 16 * 1024 * 1024 {
        return Err("CONTEXT_BUDGET_EXCEEDED");
    }
    Ok(request)
}
pub(super) async fn accept(
    tx: &mut Tx<'_>,
    p: &Value,
    payload: &Value,
    expected: i64,
) -> Result<(Job, Value), Failure> {
    let profile = p["id"].as_str().unwrap();
    let r = get(tx,"memory_records",profile,payload["recordId"].as_str().ok_or("VALIDATION_ERROR")?).await.map_err(|e|if e=="NOT_FOUND"{"REVISION_CONFLICT"}else{e})?;
    if r["id"] != payload["recordId"] || revision(&r)? != expected {
        return Err("REVISION_CONFLICT");
    }
    if !matches!(r["state"].as_str(), Some("empty" | "active")) {
        return Err("POLICY_DENIED");
    }
    let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM memory_runs r JOIN operations o ON o.profile_id=r.profile_id AND o.id=r.operation_id WHERE r.profile_id=? AND r.record_id=? AND json_extract(o.body,'$.status') IN ('queued','running')").bind(profile).bind(r["id"].as_str().unwrap()).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if n > 0 {
        return Err("REVISION_CONFLICT");
    }
    let (messages, conv) = resolve(tx, profile, &payload["sourceRefs"]).await?;
    let rs = routes(tx, p).await?;
    let budget = &p["contextBudget"];
    if budget["costLimit"]["kind"] != "unconfigured" {
        return Err("COST_ESTIMATE_UNKNOWN");
    }
    let calls = budget["maxPhysicalCalls"].as_u64().unwrap();
    let elapsed = budget["maxElapsedMs"].as_u64().unwrap();
    if !(1..=4).contains(&calls) || !(1..=180000).contains(&elapsed) {
        return Err("VALIDATION_ERROR");
    }
    // Proposing and deciding are separate required calls, even with Scribe off.
    if calls < 2 {
        return Err("OPERATION_BUDGET_EXCEEDED");
    }
    let cap =
        memory::Capacity::from_dto(&p["memorySettings"][r["kind"].as_str().ok_or("STORAGE_ERROR")?]).map_err(|_| "STORAGE_ERROR")?;
    for role in ["primary", "scribe"] {
        if role == "scribe" && p["scribeMode"] == "disabled" {
            continue;
        }
        let b = rs.get(role).ok_or("MODEL_CAPABILITY_UNKNOWN")?;
        if !verified(&b["connection"]) {
            return Err("CONNECTION_UNVERIFIED");
        }
        if b["contextTokens"].as_u64().unwrap_or(0) < 1024
            || b["route"].get("reasoningValue").is_some()
        {
            return Err("MODEL_CAPABILITY_UNKNOWN");
        }
        if let memory::Capacity::Bounded(tokens) = cap {
            if tokens
                > b["route"]["outputLimitTokens"]
                    .as_u64()
                    .unwrap()
                    .min(budget["memoryOutputCeilingTokens"].as_u64().unwrap())
            {
                return Err("OUTPUT_BUDGET_INSUFFICIENT");
            }
        }
    }
    let source_language = messages
        .iter()
        .filter(|m| m["role"] == "human")
        .max_by_key(|m| m["sequence"].as_u64().unwrap())
        .or_else(|| messages.last())
        .ok_or("VALIDATION_ERROR")?["language"]
        .as_str()
        .ok_or("STORAGE_ERROR")?
        .to_string();
    let language = if p["languagePolicy"] == "explicit_translation" {
        p["memoryLanguage"]
            .as_str()
            .ok_or("VALIDATION_ERROR")?
            .to_string()
    } else {
        source_language
    };
    let previous = version(tx, &r).await?;
    let engine = Engine::new(memory::Snapshot {
        profile_id: profile.into(),
        profile_revision: revision(p)? as u64,
        record_id: r["id"].as_str().unwrap().into(),
        record_revision: expected as u64,
        base_version_id: previous.as_ref().map(|v| v["id"].as_str().unwrap().into()),
        primary: engine_route(&rs["primary"])?,
        scribe: if p["scribeMode"] == "disabled" {
            None
        } else {
            Some(engine_route(&rs["scribe"])?)
        },
        language: language.clone(),
        capacity: cap,
        previous_content: previous
            .as_ref()
            .map(|v| v["content"].as_str().unwrap().into()),
        sources: messages
            .iter()
            .zip(payload["sourceRefs"].as_array().unwrap())
            .map(|(m, r)| memory::Source {
                message_metadata: if r["kind"]=="message" {Some(json!({"role":m["role"],"language":m["language"],"createdAt":m["createdAt"],"sequence":m["sequence"]}))} else {None},
                reference: r.clone(),
                text: m["content"].as_str().unwrap().into(),
            })
            .collect(),
    })
    .map_err(|_| "VALIDATION_ERROR")?;
    let metadata=super::memory_layers::metadata(tx,profile,&r).await?;
    let reason=payload["correctionReason"].as_str().unwrap_or("");
    if r["kind"]!="working" && previous.is_some() && reason.trim().is_empty(){return Err("VALIDATION_ERROR");}
    let context=json!({"record":r,"metadata":metadata,"correctionReason":reason});
    let mut budget_probe = engine.clone();
    let first = budget_probe.next_call().map_err(|_| "STORAGE_ERROR")?;
    let role = if first.role == memory::Role::Scribe {
        "scribe"
    } else {
        "primary"
    };
    build(&super::memory_layers::prompt(first,&context), &rs[role], p)?;
    // Check the minimum possible Primary decision request before accepting work.
    // One unescaped UTF-8 byte and a shortest allowed claimKind form a lower
    // bound, not a prediction of Scribe's output. This cloned pure engine never
    // calls a provider, writes a proposal, or consumes the operation's budget.
    budget_probe
        .proposal(&json!({"content":"x","language":language,"claimKind":"fact"}).to_string())
        .map_err(|_| "STORAGE_ERROR")?;
    let minimum_decision = budget_probe.next_call().map_err(|_| "STORAGE_ERROR")?;
    build(&super::memory_layers::prompt(minimum_decision,&context), &rs["primary"], p)?;
    // run() still builds and validates every actual request with its real data.
    let op = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"kind":"memory_update","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":expected,"limits":{"maxPhysicalCalls":calls,"maxElapsedMs":elapsed},"recordRefs":[{"profileId":profile,"recordKind":"memory_record","recordId":r["id"]}]});
    put(tx, "operations", &op, true, 0).await?;
    event(tx, profile, "operation.changed", &op).await?;
    let snapshot = json!({"profile":p,"record":r,"version":previous,"routes":rs,"messages":messages,"sources":payload["sourceRefs"],"conversationId":conv.as_ref().map(|c|c["id"].clone()).unwrap_or(Value::Null),"language":language,"conversationLanguage":conv.as_ref().map(|c|c["language"].clone()).unwrap_or(json!("und")),"formatRepairCount":0,"metadata":metadata,"correctionReason":reason});
    sqlx::query("INSERT INTO memory_runs(profile_id,operation_id,record_id,body) VALUES(?,?,?,?)")
        .bind(profile)
        .bind(op["id"].as_str().unwrap())
        .bind(r["id"].as_str().unwrap())
        .bind(snapshot.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    Ok((Job { engine, snapshot }, op))
}
async fn snapshot(tx: &mut Tx<'_>, op: &Value) -> Result<Value, Failure> {
    let body: String =
        sqlx::query_scalar("SELECT body FROM memory_runs WHERE profile_id=? AND operation_id=?")
            .bind(op["profileId"].as_str().unwrap())
            .bind(op["id"].as_str().unwrap())
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    serde_json::from_str(&body).map_err(|_| "STORAGE_ERROR")
}
pub(super) async fn ensure_current(tx: &mut Tx<'_>, op: &Value) -> Result<(), Failure> {
    let s = snapshot(tx, op).await?;
    let profile = op["profileId"].as_str().unwrap();
    let p = get(tx, "profiles", "", profile).await?;
    if p != s["profile"] || p["status"] == "archived" {
        return Err("REVISION_CONFLICT");
    }
    let r = get(
        tx,
        "memory_records",
        profile,
        s["record"]["id"].as_str().unwrap(),
    )
    .await?;
    if r != s["record"] || json!(version(tx, &r).await?) != s["version"] {
        return Err("REVISION_CONFLICT");
    }
    if s.get("metadata").is_some() && super::memory_layers::metadata(tx,profile,&r).await?!=s["metadata"] {return Err("REVISION_CONFLICT");}
    let rs = routes(tx, &p).await?;
    for role in ["primary", "scribe"] {
        if role == "scribe" && (p["scribeMode"] == "disabled" || s.get("restoreSource").is_some()) {
            continue;
        }
        if rs[role] != s["routes"][role] || !verified(&rs[role]["connection"]) {
            return Err("REVISION_CONFLICT");
        }
    }
    if s.get("restoreSource").is_some(){return restore_runtime::ensure_source(tx,profile,&s).await;}
    let (messages, c) = resolve(tx, profile, &s["sources"]).await?;
    let cid=c.as_ref().map(|v|v["id"].clone()).unwrap_or(Value::Null);let clang=c.as_ref().map(|v|v["language"].clone()).unwrap_or(json!("und"));
    if json!(messages) != s["messages"] || cid != s["conversationId"] || clang != s["conversationLanguage"] {return Err("REVISION_CONFLICT");}
    Ok(())
}
pub(super) async fn close_pending(
    tx: &mut Tx<'_>,
    profile: &str,
    opid: &str,
) -> Result<(), Failure> {
    let rows = items(tx, "memory_proposals", profile, opid).await?;
    for mut p in rows {
        if p["state"] == "pending" {
            let rev = revision(&p)?;
            bump(&mut p)?;
            p["state"] = json!("failed");
            put(tx, "memory_proposals", &p, false, rev).await?;
        }
    }
    Ok(())
}
async fn fail(
    service: &Connections,
    op: &Value,
    budget: &CallBudget,
    code: Failure,
) -> Result<(), Failure> {
    let _guard = service.gate.lock().await;
    let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
    let profile = op["profileId"].as_str().unwrap();
    let opid = op["id"].as_str().unwrap();
    let mut current = get(&mut tx, "operations", profile, opid).await?;
    if !matches!(current["status"].as_str(), Some("running" | "queued")) {
        return Ok(());
    }
    let code = ensure_current(&mut tx, &current)
        .await
        .err()
        .unwrap_or(code);
    let rev = revision(&current)?;
    if current["kind"]!="restore" {current["physicalCallCount"] = json!(budget.physical_calls());}
    terminal(
        &mut current,
        if code == "OPERATION_CANCELLED" {
            "cancelled"
        } else {
            "failed"
        },
        code,
    )?;
    close_pending(&mut tx, profile, opid).await?;
    put(&mut tx, "operations", &current, false, rev).await?;
    event(&mut tx, profile, "operation.changed", &current).await?;
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
fn failure(f: &memory::Failure) -> Failure {
    match f {
        memory::Failure::Capacity => "MEMORY_CAPACITY_EXCEEDED",
        memory::Failure::Schema => "MEMORY_FORMAT_INVALID",
        memory::Failure::Rejected => "MEMORY_PRIMARY_REJECTED",
        memory::Failure::CallBudget => "OPERATION_BUDGET_EXCEEDED",
        memory::Failure::Cancelled => "OPERATION_CANCELLED",
        memory::Failure::Provider => "PROVIDER_ERROR",
    }
}
async fn persist(
    service: &Connections,
    op: &Value,
    job: &Job,
    budget: &CallBudget,
    prompt: &memory::Prompt,
    raw: &str,
    old_attempts: usize,
    old_decisions: usize,
    proposal_id: &mut Option<String>,
    receipt_id: &mut Option<String>,
) -> Result<(), Failure> {
    let _guard = service.gate.lock().await;
    let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
    let profile = op["profileId"].as_str().unwrap();
    let opid = op["id"].as_str().unwrap();
    let mut current = get(&mut tx, "operations", profile, opid).await?;
    if current["status"] != "running" {
        return Err("OPERATION_CANCELLED");
    }
    ensure_current(&mut tx, &current).await?;
    budget.ensure_active().map_err(transport_code)?;
    let rev = revision(&current)?;
    let engine = &job.engine;
    if prompt.stage == Stage::Proposal
        && (engine.pending_candidate().is_some() || engine.attempts().len() > old_attempts)
    {
        let v: Value = serde_json::from_str(raw).map_err(|_| "STORAGE_ERROR")?;
        let mut p = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"operationId":opid,"memoryRecordId":job.snapshot["record"]["id"],"memoryKind":job.snapshot["record"]["kind"],"proposerRole":if prompt.role==memory::Role::Scribe{"scribe"}else{"primary"},"proposerRouteId":prompt.route.id,"content":v["content"],"language":v["language"],"sources":job.snapshot["sources"],"state":if engine.pending_candidate().is_some(){"pending"}else{"failed"},"capacityPolicySnapshot":engine.snapshot().capacity.dto(),"attemptNumber":prompt.attempt,"claimKind":v["claimKind"]});
        if let Some(base) = &engine.snapshot().base_version_id {
            p["baseVersionId"] = json!(base)
        }
        put(&mut tx, "memory_proposals", &p, true, 0).await?;
        *proposal_id = Some(p["id"].as_str().unwrap().into());
    }
    if engine.decisions().len() > old_decisions {
        let d = engine.decisions().last().unwrap();
        let prop = proposal_id.as_ref().ok_or("STORAGE_ERROR")?;
        let receipt = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"operationId":opid,"proposalId":prop,"primaryRouteId":d.route.id,"routeRevision":d.route.revision,"decision":d.output["decision"],"candidateSha256":d.candidate.sha256,"candidateMeasurement":d.candidate.measurement,"reason":d.output["reason"]});
        put(&mut tx, "memory_decisions", &receipt, true, 0).await?;
        *receipt_id = Some(receipt["id"].as_str().unwrap().into());
        let mut p = get(&mut tx, "memory_proposals", profile, prop).await?;
        let rev = revision(&p)?;
        bump(&mut p)?;
        p["state"] = json!("decided");
        put(&mut tx, "memory_proposals", &p, false, rev).await?;
    }
    for a in &engine.attempts()[old_attempts..] {
        let attempt = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"operationId":opid,"proposalId":proposal_id.as_ref().ok_or("STORAGE_ERROR")?,"attemptNumber":a.number,"capacityPolicySnapshot":engine.snapshot().capacity.dto(),"candidateSha256":a.candidate_sha256,"candidateMeasurement":a.measurement,"result":a.result});
        put(&mut tx, "memory_attempts", &attempt, true, 0).await?;
    }
    current["capacityAttemptCount"] = json!(prompt.attempt);
    if current["kind"]!="restore" {current["physicalCallCount"] = json!(budget.physical_calls());}
    let mut stored = snapshot(&mut tx, &current).await?;
    stored["formatRepairCount"] = json!(engine.format_repairs());
    sqlx::query("UPDATE memory_runs SET body=? WHERE profile_id=? AND operation_id=?")
        .bind(stored.to_string())
        .bind(profile)
        .bind(opid)
        .execute(&mut *tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    match engine.outcome() {
        Some(Outcome::Candidate(c)) => {
            let receipt = get(
                &mut tx,
                "memory_decisions",
                profile,
                receipt_id.as_ref().ok_or("STORAGE_ERROR")?,
            )
            .await?;
            if receipt["operationId"] != opid
                || receipt["candidateSha256"] != c.sha256
                || !matches!(receipt["decision"].as_str(), Some("accept" | "modify"))
            {
                return Err("STORAGE_ERROR");
            }
            let mut r = job.snapshot["record"].clone();
            let old = revision(&r)?;
            let mut v = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"recordId":r["id"],"versionNumber":job.snapshot["version"]["versionNumber"].as_u64().unwrap_or(0)+1,"content":c.content,"language":c.language,"contentSha256":c.sha256,"measurement":c.measurement,"sources":job.snapshot["sources"],"origin":"agent_decision","decisionReceiptId":receipt["id"],"claimKind":c.claim_kind});
            if let Some(source)=job.snapshot.get("restoreSource"){v["origin"]=json!("restore");v["restoredFromVersionId"]=source["id"].clone();}
            if let Some(previous) = r.get("activeVersionId") {
                v["previousVersionId"] = previous.clone()
            }
            put(&mut tx, "memory_versions", &v, true, 0).await?;
            super::memory_layers::commit_metadata(&mut tx,profile,&v,&job.snapshot).await?;
            bump(&mut r)?;
            r["state"] = json!("active");
            r["activeVersionId"] = v["id"].clone();
            put(&mut tx, "memory_records", &r, false, old).await?;
            event(&mut tx, profile, "memory.changed", &r).await?;
            bump(&mut current)?;
            current["status"] = json!("succeeded");
            current["finishedAt"] = json!(now());
            current["result"] = if let Some(source)=job.snapshot.get("restoreSource"){json!({"kind":"restore","recordId":r["id"],"versionId":v["id"],"recordRevision":r["revision"],"restoredFromVersionId":source["id"]})}else{json!({"kind":"memory_update","outcome":"committed","recordId":r["id"],"versionId":v["id"],"recordRevision":r["revision"]})};
        }
        Some(Outcome::Failed(memory::Failure::Rejected)) if job.snapshot.get("restoreSource").is_some() => {terminal(&mut current,"failed","MEMORY_PRIMARY_REJECTED")?;},
        Some(Outcome::Failed(memory::Failure::Rejected)) => {
            bump(&mut current)?;
            current["status"] = json!("succeeded");
            current["finishedAt"] = json!(now());
            let mut result = json!({"kind":"memory_update","outcome":"no_change","recordId":job.snapshot["record"]["id"]});
            if let Some(v) = job.snapshot["record"].get("activeVersionId") {
                result["activeVersionId"] = v.clone()
            }
            current["result"] = result;
        }
        Some(Outcome::Failed(f)) => {
            terminal(&mut current, "failed", failure(f))?;
            close_pending(&mut tx, profile, opid).await?;
        }
        None => {
            bump(&mut current)?;
        }
    }
    put(&mut tx, "operations", &current, false, rev).await?;
    event(&mut tx, profile, "operation.changed", &current).await?;
    // Atomic write: no user-visible state is activated if this commit fails.
    budget.ensure_active().map_err(transport_code)?;
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
pub(super) async fn run(
    service: &Connections,
    mut job: Job,
    op: Value,
    budget: CallBudget,
) -> Result<(), Failure> {
    let profile = op["profileId"].as_str().unwrap();
    let opid = op["id"].as_str().unwrap();
    {
        let _guard = service.gate.lock().await;
        let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        let mut current = get(&mut tx, "operations", profile, opid).await?;
        if current["status"] != "queued" {
            return Ok(());
        }
        let rev = revision(&current)?;
        bump(&mut current)?;
        current["status"] = json!("running");
        put(&mut tx, "operations", &current, false, rev).await?;
        event(&mut tx, profile, "operation.changed", &current).await?;
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    }
    let result=async {
  let mut proposal=job.snapshot["restoreProposalId"].as_str().map(str::to_string);let mut receipt=None;
  while job.engine.outcome().is_none(){
   let prompt=match job.engine.next_call(){Ok(p)=>p,Err(_)=>return Err(job.engine.outcome().and_then(|o|if let Outcome::Failed(f)=o{Some(failure(f))}else{None}).unwrap_or("STORAGE_ERROR"))};
   let prompt=super::memory_layers::prompt(restore_runtime::prompt(prompt,&job.snapshot),&job.snapshot);
   let role=if prompt.role==memory::Role::Scribe{"scribe"}else{"primary"};let b=&job.snapshot["routes"][role];let request=build(&prompt,b,&job.snapshot["profile"])?;
   {let _guard=service.gate.lock().await;let mut tx=service.pool.begin().await.map_err(|_|"STORAGE_ERROR")?;ensure_current(&mut tx,&op).await?;budget.ensure_active().map_err(transport_code)?;}
   let response=tokio::select!{biased;_=budget.cancelled()=>Err("OPERATION_CANCELLED"),r=tokio::time::timeout_at(budget.deadline(),async {
    let secret=service.vault.secret(request.scope.clone(),b["connection"]["credentialRef"].as_str().unwrap().into()).await.map_err(|_|"CREDENTIAL_UNAVAILABLE")?;
    let cred=BoundCredential::new(&request.scope,secret).map_err(|_|"CREDENTIAL_UNAVAILABLE")?;
    HttpTransport::new().map_err(|_|"PROVIDER_ERROR")?.execute_chat(&request,&cred,&budget).await.map_err(transport_code)
   })=>r.unwrap_or(Err("PROVIDER_TIMEOUT"))}?;
   let a=job.engine.attempts().len();let d=job.engine.decisions().len();
   if prompt.stage==Stage::Proposal{job.engine.proposal(response.content())}else{job.engine.decision(response.content())}.map_err(|_|"STORAGE_ERROR")?;
   persist(service,&op,&job,&budget,&prompt,response.content(),a,d,&mut proposal,&mut receipt).await?;
  } Ok(())
 }.await;
    if let Err(code) = result {
        fail(service, &op, &budget, code).await?;
    }
    Ok(())
}
