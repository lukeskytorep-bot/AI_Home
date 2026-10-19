//! Durable text chat. Pure protocol execution stays in providers; all commits are scoped.
use super::{
    bump, event, get, id, kind, now, put, revision, stamp, terminal, transport_code, verified,
    Connections, Failure, Tx,
};
use crate::{
    domain::schema,
    providers::{
        self,
        transport::{BoundCredential, CallBudget, HttpTransport},
        ProviderChatRequest, ProviderMessage, ScopeStamp,
    },
};
use serde_json::{json, Value};
use sqlx::Row;
pub(super) struct Job {
    request: ProviderChatRequest,
    connection: Value,
    user_id: String,
    conversation: String,
    profile_revision: i64,
}
fn settings_dto(route: &Value, profile: &Value, context: i64) -> Value {
    json!({"routeId":route["id"],"routeRevision":route["revision"],"modelId":route["modelId"],"contextTokens":context,"source":"user_declared","measurement":"utf8_bytes_estimate","outputTokens":route["outputLimitTokens"].as_u64().unwrap().min(profile["contextBudget"]["chatOutputTokens"].as_u64().unwrap()),"inputReserveTokens":profile["contextBudget"]["inputReserveTokens"]})
}
async fn primary(tx: &mut Tx<'_>, profile: &str) -> Result<(Value, Value), Failure> {
    let p = get(tx, "profiles", "", profile).await?;
    let route = get(
        tx,
        "routes",
        profile,
        p["primaryRouteId"]
            .as_str()
            .ok_or("MODEL_CAPABILITY_UNKNOWN")?,
    )
    .await?;
    if route["role"] != "primary" {
        return Err("STORAGE_ERROR");
    }
    Ok((p, route))
}
pub(super) async fn settings(
    tx: &mut Tx<'_>,
    command: &str,
    profile: &str,
    payload: &Value,
    expected: i64,
) -> Result<Value, Failure> {
    let (p, mut route) = primary(tx, profile).await?;
    if command == "chat.settings.set" {
        if route["id"] != payload["routeId"] || revision(&route)? != expected {
            return Err("REVISION_CONFLICT");
        }
        bump(&mut route)?;
        put(tx, "routes", &route, false, expected).await?;
        sqlx::query("INSERT INTO chat_policies(profile_id,route_id,route_revision,context_tokens) VALUES(?,?,?,?) ON CONFLICT(profile_id,route_id) DO UPDATE SET route_revision=excluded.route_revision,context_tokens=excluded.context_tokens").bind(profile).bind(route["id"].as_str().unwrap()).bind(revision(&route)?).bind(payload["contextTokens"].as_i64().unwrap()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        snapshot_event(tx, profile).await?;
    }
    let size:Option<i64>=sqlx::query_scalar("SELECT context_tokens FROM chat_policies WHERE profile_id=? AND route_id=? AND route_revision=?").bind(profile).bind(route["id"].as_str().unwrap()).bind(revision(&route)?).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    Ok(settings_dto(&route, &p, size.unwrap_or(0)))
}
async fn snapshot_event(tx: &mut Tx<'_>, profile: &str) -> Result<(), Failure> {
    let seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?",
    )
    .bind(profile)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    event(
        tx,
        profile,
        "snapshot.required",
        &json!({"lastAvailableSequence":seq}),
    )
    .await
}
pub(super) async fn latest(
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
    let row=sqlx::query("SELECT operation_id FROM chat_runs WHERE profile_id=? AND conversation_id=? ORDER BY rowid DESC LIMIT 1").bind(profile).bind(c["id"].as_str().unwrap()).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    match row {
        Some(row) => Ok(
            json!({"found":true,"operation":get(tx,"operations",profile,row.get("operation_id")).await?}),
        ),
        None => Ok(json!({"found":false})),
    }
}
fn scope(route: &Value, c: &Value) -> Result<ScopeStamp, Failure> {
    let mut s = stamp(c)?;
    s.route_id = route["id"].as_str().unwrap().into();
    s.route_revision = route["revision"].as_u64().unwrap();
    s.model_id = route["modelId"].as_str().unwrap().into();
    Ok(s)
}
async fn insert_message(tx: &mut Tx<'_>, v: &Value) -> Result<(), Failure> {
    if !schema::dto("Message", v) {
        return Err("VALIDATION_ERROR");
    }
    sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,?,?,?,1,?)").bind(v["profileId"].as_str().unwrap()).bind(v["id"].as_str().unwrap()).bind(v["conversationId"].as_str().unwrap()).bind(v["sequence"].as_i64().unwrap()).bind(v.to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    event(tx, v["profileId"].as_str().unwrap(), "message.created", v).await
}
async fn next_sequence(tx: &mut Tx<'_>, profile: &str, conversation: &str) -> Result<i64, Failure> {
    let max: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence),0) FROM messages WHERE profile_id=? AND conversation_id=?",
    )
    .bind(profile)
    .bind(conversation)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    max.checked_add(1)
        .filter(|x| *x <= 9_007_199_254_740_991)
        .ok_or("STORAGE_ERROR")
}
pub(super) async fn accept(
    tx: &mut Tx<'_>,
    p: &Value,
    payload: &Value,
    expected: i64,
) -> Result<(Job, Value), Failure> {
    let profile = p["id"].as_str().unwrap();
    let conversation = payload["conversationId"].as_str().unwrap();
    let mut conv = get(tx, "conversations", profile, conversation).await?;
    if revision(&conv)? != expected {
        return Err("REVISION_CONFLICT");
    }
    if conv["status"] != "open" {
        return Err("POLICY_DENIED");
    }
    if payload["attachmentIds"].as_array().unwrap().len() > 0 {
        return Err("VALIDATION_ERROR");
    }
    if payload["content"].as_str().unwrap().trim().is_empty() {
        return Err("VALIDATION_ERROR");
    }
    let pending:i64=sqlx::query_scalar("SELECT COUNT(*) FROM chat_runs r JOIN operations o ON o.profile_id=r.profile_id AND o.id=r.operation_id WHERE r.profile_id=? AND r.conversation_id=? AND json_extract(o.body,'$.status') IN ('queued','running')").bind(profile).bind(conversation).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if pending > 0 {
        return Err("REVISION_CONFLICT");
    }
    let (_, route) = primary(tx, profile).await?;
    let c = get(
        tx,
        "connections",
        profile,
        route["connectionId"].as_str().unwrap(),
    )
    .await?;
    if !verified(&c) {
        return Err("CONNECTION_UNVERIFIED");
    }
    // Native policy is tied to route revision; no guessed model context capacity.
    let context:Option<i64>=sqlx::query_scalar("SELECT context_tokens FROM chat_policies WHERE profile_id=? AND route_id=? AND route_revision=?").bind(profile).bind(route["id"].as_str().unwrap()).bind(revision(&route)?).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let context = context.ok_or("MODEL_CAPABILITY_UNKNOWN")?;
    if p["contextBudget"]["costLimit"]["kind"] != "unconfigured" {
        return Err("COST_ESTIMATE_UNKNOWN");
    }
    if route.get("reasoningValue").is_some() {
        return Err("MODEL_CAPABILITY_UNKNOWN");
    }
    let policy = &p["contextBudget"];
    let calls = policy["maxPhysicalCalls"].as_u64().unwrap();
    let elapsed = policy["maxElapsedMs"].as_u64().unwrap();
    if !(1..=4).contains(&calls) || !(1..=180000).contains(&elapsed) {
        return Err("VALIDATION_ERROR");
    }
    let output = route["outputLimitTokens"]
        .as_u64()
        .unwrap()
        .min(policy["chatOutputTokens"].as_u64().unwrap());
    if output == 0 || output > u32::MAX as u64 {
        return Err("OUTPUT_BUDGET_INSUFFICIENT");
    }
    let scope = scope(&route, &c)?;
    let total:i64=sqlx::query_scalar("SELECT COALESCE(SUM(length(CAST(body AS BLOB))),0) FROM messages WHERE profile_id=? AND conversation_id=?").bind(profile).bind(conversation).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if total > 8 * 1024 * 1024 {
        return Err("CONTEXT_BUDGET_EXCEEDED");
    }
    let rows = sqlx::query(
        "SELECT id FROM messages WHERE profile_id=? AND conversation_id=? ORDER BY sequence",
    )
    .bind(profile)
    .bind(conversation)
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    let mut messages = vec![];
    // System instruction contains no identity claims or private memory placeholders.
    messages.push(ProviderMessage::text("system","You are the assistant in this conversation. Answer the user in the language they use unless they request another language."));
    let mut replay_bytes = 0usize;
    for row in rows {
        let m = get(tx, "messages", profile, row.get("id")).await?;
        if m["conversationId"] != conversation {
            return Err("STORAGE_ERROR");
        }
        if m["status"] != "complete" {
            return Err("POLICY_DENIED");
        }
        let role = match m["role"].as_str() {
            Some("human") => "user",
            Some("assistant") => "assistant",
            _ => return Err("POLICY_DENIED"),
        };
        let mut message = ProviderMessage::text(role, m["content"].as_str().unwrap());
        if let Some(saved) =
            sqlx::query("SELECT scope,body FROM chat_replay WHERE profile_id=? AND message_id=?")
                .bind(profile)
                .bind(m["id"].as_str().unwrap())
                .fetch_optional(&mut **tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?
        {
            let stamp: Value =
                serde_json::from_str(saved.get("scope")).map_err(|_| "STORAGE_ERROR")?;
            if stamp == providers::replay::scope_value(&scope) {
                let raw = saved.get::<&str, _>("body");
                replay_bytes = replay_bytes
                    .checked_add(raw.len())
                    .ok_or("CONTEXT_BUDGET_EXCEEDED")?;
                message.continuation_state = Some(
                    providers::replay::restore(
                        &scope,
                        serde_json::from_str(raw).map_err(|_| "STORAGE_ERROR")?,
                    )
                    .map_err(|_| "STORAGE_ERROR")?,
                );
            }
        }
        messages.push(message);
    }
    messages.push(ProviderMessage::text(
        "user",
        payload["content"].as_str().unwrap(),
    ));
    // Explicit conservative estimate, never described as a model tokenizer count.
    let estimate: usize = messages
        .iter()
        .map(|m| m.content.len() + 64)
        .sum::<usize>()
        .checked_add(replay_bytes)
        .ok_or("CONTEXT_BUDGET_EXCEEDED")?;
    if estimate as u64 + output + policy["inputReserveTokens"].as_u64().unwrap() > context as u64 {
        return Err("CONTEXT_BUDGET_EXCEEDED");
    }
    let request = ProviderChatRequest {
        provider: kind(&c["providerKind"])?,
        model_id: scope.model_id.clone(),
        messages,
        scope,
        temperature: route["temperature"].as_f64(),
        max_output_tokens: Some(output as u32),
        reasoning_effort: None,
        reasoning_max_tokens: None,
        reasoning_transport_kind: None,
        reasoning_transport_value: None,
        custom_output_token_field: None,
        provider_routing: None,
    };
    providers::prepare_chat(&request, &c["normalizedEndpoint"].as_str().unwrap())
        .map_err(|_| "VALIDATION_ERROR")?;
    let opid = id();
    let user = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"conversationId":conversation,"sequence":next_sequence(tx,profile,conversation).await?,"role":"human","content":payload["content"],"language":payload["language"],"status":"complete","sourceRefs":[],"operationId":opid});
    insert_message(tx, &user).await?;
    conv["draftText"] = json!("");
    bump(&mut conv)?;
    put(tx, "conversations", &conv, false, expected).await?;
    let op = json!({"schemaVersion":1,"id":opid,"profileId":profile,"revision":1,"createdAt":now(),"kind":"chat","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":conv["revision"],"limits":{"maxPhysicalCalls":calls,"maxElapsedMs":elapsed},"recordRefs":[{"profileId":profile,"recordKind":"connection","recordId":c["id"]},{"profileId":profile,"recordKind":"conversation","recordId":conversation},{"profileId":profile,"recordKind":"message","recordId":user["id"]}]});
    put(tx, "operations", &op, true, 0).await?;
    event(tx, profile, "operation.changed", &op).await?;
    let job = Job {
        request,
        connection: c,
        user_id: user["id"].as_str().unwrap().into(),
        conversation: conversation.into(),
        profile_revision: revision(p)?,
    };
    sqlx::query("INSERT INTO chat_runs(profile_id,operation_id,conversation_id,body) VALUES(?,?,?,?)").bind(profile).bind(&opid).bind(conversation).bind(json!({"connection":job.connection,"route":route,"profileRevision":job.profile_revision,"userId":job.user_id,"estimatedInputTokens":estimate,"outputTokens":output}).to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    snapshot_event(tx, profile).await?;
    Ok((job, op))
}
pub(super) async fn ensure_current(tx: &mut Tx<'_>, op: &Value) -> Result<(), Failure> {
    let profile = op["profileId"].as_str().unwrap();
    let row = sqlx::query(
        "SELECT body,conversation_id FROM chat_runs WHERE profile_id=? AND operation_id=?",
    )
    .bind(profile)
    .bind(op["id"].as_str().unwrap())
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    let captured: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
    let (p, r) = primary(tx, profile).await?;
    let live = get(
        tx,
        "connections",
        profile,
        captured["connection"]["id"]
            .as_str()
            .ok_or("STORAGE_ERROR")?,
    )
    .await?;
    let conv = get(tx, "conversations", profile, row.get("conversation_id")).await?;
    if p["status"] == "archived"
        || conv["status"] != "open"
        || p["revision"] != captured["profileRevision"]
        || r != captured["route"]
        || live != captured["connection"]
        || !verified(&live)
    {
        return Err("REVISION_CONFLICT");
    }
    Ok(())
}
pub(super) async fn run(
    service: &Connections,
    job: Job,
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
    let answer = tokio::select! {
        biased;
        _=budget.cancelled()=>Err("OPERATION_CANCELLED"),
        result=tokio::time::timeout_at(budget.deadline(),async {
        let secret = service
            .vault
            .secret(
                job.request.scope.clone(),
                job.connection["credentialRef"].as_str().unwrap().into(),
            )
            .await
            .map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
        let credential = BoundCredential::new(&job.request.scope, secret)
            .map_err(|_| "CREDENTIAL_UNAVAILABLE")?;
        HttpTransport::new()
            .map_err(|_| "PROVIDER_ERROR")?
            .execute_chat(&job.request, &credential, &budget)
            .await
            .map_err(transport_code)
    }
    )=>result.unwrap_or(Err("PROVIDER_TIMEOUT")),
    };
    let _guard = service.gate.lock().await;
    let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
    let mut current = get(&mut tx, "operations", profile, opid).await?;
    if current["status"] != "running" {
        return Ok(());
    }
    let rev = revision(&current)?;
    current["physicalCallCount"] = json!(budget.physical_calls());
    let result = ensure_current(&mut tx, &current)
        .await
        .and_then(|_| budget.ensure_active().map_err(transport_code))
        .and(answer)
        .and_then(|response| {
            // Validate replay before committing the assistant. Unsupported native
            // metadata must not poison the next turn after a successful commit.
            if let Some(raw) = response.private_replay(job.request.provider) {
                let state = providers::replay::restore(&job.request.scope, raw)
                    .map_err(|_| "PROVIDER_ERROR")?;
                let mut message = ProviderMessage::text("assistant", response.content());
                message.continuation_state = Some(state);
                let mut probe = job.request.clone();
                probe.messages = vec![
                    message,
                    ProviderMessage::text("user", "continuation validation"),
                ];
                providers::prepare_chat(&probe, &probe.scope.normalized_endpoint)
                    .map_err(|_| "PROVIDER_ERROR")?;
            }
            Ok(response)
        });
    match result {
        Ok(response) => {
            let mut conv = get(&mut tx, "conversations", profile, &job.conversation).await?;
            let message = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"conversationId":job.conversation,"sequence":next_sequence(&mut tx,profile,&job.conversation).await?,"role":"assistant","content":response.content(),"language":conv["language"],"status":"complete","sourceRefs":[],"operationId":opid});
            insert_message(&mut tx, &message).await?;
            if let Some(private) = response.private_replay(job.request.provider) {
                sqlx::query(
                    "INSERT INTO chat_replay(profile_id,message_id,scope,body) VALUES(?,?,?,?)",
                )
                .bind(profile)
                .bind(message["id"].as_str().unwrap())
                .bind(providers::replay::scope_value(&job.request.scope).to_string())
                .bind(private.to_string())
                .execute(&mut *tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
            }
            let old = revision(&conv)?;
            bump(&mut conv)?;
            put(&mut tx, "conversations", &conv, false, old).await?;
            bump(&mut current)?;
            current["status"] = json!("succeeded");
            current["finishedAt"] = json!(now());
            current["result"] = json!({"kind":"chat","conversationId":job.conversation,"userMessageId":job.user_id,"assistantMessageId":message["id"]});
            current["recordRefs"]
                .as_array_mut()
                .unwrap()
                .push(json!({"profileId":profile,"recordKind":"message","recordId":message["id"]}));
        }
        Err(code) => terminal(
            &mut current,
            if code == "OPERATION_CANCELLED" {
                "cancelled"
            } else {
                "failed"
            },
            code,
        )?,
    }
    put(&mut tx, "operations", &current, false, rev).await?;
    event(&mut tx, profile, "operation.changed", &current).await?;
    snapshot_event(&mut tx, profile).await?;
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
