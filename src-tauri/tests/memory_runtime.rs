include!("support/memory_setup.rs");
async fn memory_view(app: &Application, p: &str) -> Value {
    ok(app, "memory.working.get", json!({}), Some(p), None).await
}
async fn memory_settings(app: &Application, p: &str, capacity: Value, mode: &str) -> Value {
    let v = memory_view(app, p).await;
    ok(app,"memory.settings.set",json!({"scopeId":v["scopeId"],"capacityPolicy":capacity,"scribeMode":mode,"primaryContextTokens":32768,"scribeContextTokens":32768}),Some(p),v["profileRevision"].as_i64()).await
}
async fn memory_request(app: &Application, p: &str, c: &Value) -> Value {
    let src = ok(
        app,
        "memory.working.sources",
        json!({"conversationId":c["id"]}),
        Some(p),
        None,
    )
    .await;
    request(
        "memory.update.request",
        json!({"recordId":src["recordId"],"sourceRefs":src["sourceRefs"]}),
        Some(p),
        src["recordRevision"].as_i64(),
    )
}
fn answer(s: &Server, content: Value) {
    s.responses.lock().unwrap().push_back((
        200,
        json!({"choices":[{"message":{"content":content.to_string()},"finish_reason":"stop"}]})
            .to_string(),
    ));
}
fn propose_answer(s: &Server, text: &str) {
    answer(
        s,
        json!({"content":text,"language":"pl","claimKind":"mixed"}),
    );
}
fn decide_answer(s: &Server, kind: &str, text: &str) {
    if kind == "modify" {
        answer(
            s,
            json!({"decision":kind,"modifiedContent":text,"reason":"Jawna korekta."}),
        );
    } else {
        answer(s, json!({"decision":kind,"reason":"Jawna decyzja."}));
    }
}
async fn memory_fixture() -> (
    Application,
    sqlx::SqlitePool,
    Arc<MemoryVault>,
    Server,
    String,
    Value,
    Value,
) {
    let (app, pool, vault, s, p, c, conn) = configured().await;
    let r = app
        .dispatch(send_request(&p, &c, "Budujemy AI Home. Zadanie: pamięć."))
        .await
        .response;
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(
        finished(&app, &p, &r["result"]["operation"]["id"]).await["status"],
        "succeeded"
    );
    memory_settings(&app, &p, json!({"kind":"bounded","tokens":300}), "disabled").await;
    s.wires.lock().unwrap().clear();
    (app, pool, vault, s, p, c, conn)
}
async fn update(app: &Application, p: &str, c: &Value) -> Value {
    let req = memory_request(app, p, c).await;
    let a = app.dispatch(req).await.response;
    assert_eq!(a["ok"], true, "{a}");
    finished(app, p, &a["result"]["operation"]["id"]).await
}
async fn wait_posts(s: &Server, n: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while s.wires.lock().unwrap().len() < n {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn working_record_seeded_defaults_and_profile_isolation() {
    let (app, pool, _) = setup().await;
    let p = profile(&app, "First").await;
    let q = profile(&app, "Second").await;
    let v = memory_view(&app, p["id"].as_str().unwrap()).await;
    assert_eq!(v["record"]["state"], "empty");
    assert_eq!(v["capacityPolicy"]["tokens"], 300);
    assert!(schema::dto("MemoryRecord", &v["record"]));
    let w = memory_view(&app, q["id"].as_str().unwrap()).await;
    assert_ne!(w["record"]["id"], v["record"]["id"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        12
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn accepted_memory_commits_receipt_version_result_and_sources_once() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Cel: AI Home.");
    decide_answer(&s, "accept", "");
    let req = memory_request(&app, &p, &c).await;
    let a = app.dispatch(req.clone()).await.response;
    assert_eq!(a["ok"], true, "{a}");
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(op["physicalCallCount"], 2);
    assert_eq!(op["result"]["outcome"], "committed");
    let v = memory_view(&app, &p).await;
    assert_eq!(v["version"]["content"], "Cel: AI Home.");
    assert_eq!(v["version"]["language"], "pl");
    assert_eq!(v["record"]["revision"], 2);
    for (name, values) in [
        ("MemoryProposal", &v["proposals"]),
        ("MemoryWriteAttempt", &v["attempts"]),
        ("MemoryDecisionReceipt", &v["decisions"]),
    ] {
        for d in values.as_array().unwrap() {
            assert!(schema::dto(name, d), "{name}: {d}");
        }
    }
    assert!(schema::dto("MemoryVersion", &v["version"]));
    assert_eq!(app.dispatch(req).await.response, a);
    assert_eq!(s.wires.lock().unwrap().len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let wires = s.wires.lock().unwrap();
    assert!(wires[0].contains("Budujemy AI Home"));
    assert!(wires[1].contains("Cel: AI Home."));
    assert!(!v.to_string().contains("SENTINEL-secret"));
}
#[tokio::test]
async fn oversized_modify_then_second_proposal_shares_four_calls() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Cel.");
    decide_answer(&s, "modify", &"x".repeat(301));
    propose_answer(&s, "Cel poprawiony.");
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(op["physicalCallCount"], 4);
    let v = memory_view(&app, &p).await;
    assert_eq!(v["attempts"][0]["result"], "capacity_rejected");
    assert_eq!(v["attempts"][1]["result"], "accepted");
    assert_eq!(v["version"]["content"], "Cel poprawiony.");
    assert!(s.wires.lock().unwrap()[2].contains("second and final"));
}
#[tokio::test]
async fn second_failure_and_primary_reject_retain_previous_version() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Dobry zapis.");
    decide_answer(&s, "accept", "");
    assert_eq!(update(&app, &p, &c).await["status"], "succeeded");
    let old = memory_view(&app, &p).await["version"].clone();
    propose_answer(&s, &"x".repeat(301));
    propose_answer(&s, &"x".repeat(302));
    let op = update(&app, &p, &c).await;
    assert_eq!(op["error"]["code"], "MEMORY_CAPACITY_EXCEEDED", "{op}");
    assert_eq!(op["physicalCallCount"], 2);
    assert_eq!(memory_view(&app, &p).await["version"], old);
    propose_answer(&s, "Inna pamięć.");
    decide_answer(&s, "reject", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["result"]["outcome"], "no_change");
    assert_eq!(memory_view(&app, &p).await["version"], old);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn unlimited_respects_output_and_context_but_does_not_reject_content_capacity() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    memory_settings(&app, &p, json!({"kind":"unlimited"}), "disabled").await;
    propose_answer(&s, &"x".repeat(1500));
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(
        memory_view(&app, &p).await["version"]["content"]
            .as_str()
            .unwrap()
            .len(),
        1500
    );
}
#[tokio::test]
async fn format_repair_is_visible_and_not_a_capacity_retry() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    answer(&s, json!({"wrong":true}));
    propose_answer(&s, "Cel.");
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(op["physicalCallCount"], 3);
    let v = memory_view(&app, &p).await;
    assert_eq!(v["formatRepairCount"], 1);
    assert_eq!(v["attempts"].as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn two_invalid_formats_fail_without_empty_active_memory() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    answer(&s, json!({"wrong":1}));
    answer(&s, json!({"wrong":2}));
    let op = update(&app, &p, &c).await;
    assert_eq!(op["error"]["code"], "MEMORY_FORMAT_INVALID", "{op}");
    let v = memory_view(&app, &p).await;
    assert_eq!(v["record"]["state"], "empty");
    assert!(v.get("version").is_none());
}
#[tokio::test]
async fn ambiguous_receipt_retry_and_parallel_update_do_not_duplicate_paid_calls() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    s.hold.store(true, Ordering::SeqCst);
    propose_answer(&s, "Cel.");
    decide_answer(&s, "accept", "");
    let req = memory_request(&app, &p, &c).await;
    let accepted = app.dispatch(req.clone()).await.response;
    wait_posts(&s, 1).await;
    assert_eq!(app.dispatch(req).await.response, accepted);
    let second = memory_request(&app, &p, &c).await;
    assert_eq!(
        app.dispatch(second).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    assert_eq!(
        finished(&app, &p, &accepted["result"]["operation"]["id"]).await["status"],
        "succeeded"
    );
    assert_eq!(s.wires.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn cancelled_primary_cannot_commit_late_memory() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Cel.");
    decide_answer(&s, "accept", "");
    s.hold.store(true, Ordering::SeqCst);
    let req = memory_request(&app, &p, &c).await;
    let a = app.dispatch(req).await.response;
    wait_posts(&s, 1).await;
    s.release.add_permits(1);
    wait_posts(&s, 2).await;
    ok(
        &app,
        "operation.cancel",
        json!({"operationId":a["result"]["operation"]["id"]}),
        Some(&p),
        None,
    )
    .await;
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "cancelled");
    assert!(memory_view(&app, &p).await.get("version").is_none());
}
#[tokio::test]
async fn stale_settings_scope_after_key_change_is_rejected() {
    let (app, _, _, _s, p, _, conn) = memory_fixture().await;
    let old = memory_view(&app, &p).await;
    ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":conn["id"],"credential":{"secret":"NEW-secret"}}),
        Some(&p),
        Some(1),
    )
    .await;
    let r=invoke(&app,"memory.settings.set",json!({"scopeId":old["scopeId"],"capacityPolicy":{"kind":"bounded","tokens":500},"scribeMode":"disabled","primaryContextTokens":32768}),Some(&p),old["profileRevision"].as_i64()).await;
    assert_eq!(r["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(memory_view(&app, &p).await["capacityPolicy"]["tokens"], 300);
}
#[tokio::test]
async fn key_change_while_post_is_pending_prevents_commit() {
    let (app, _, _, s, p, c, conn) = memory_fixture().await;
    propose_answer(&s, "Cel.");
    decide_answer(&s, "accept", "");
    s.hold.store(true, Ordering::SeqCst);
    let a = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    wait_posts(&s, 1).await;
    ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":conn["id"],"credential":{"secret":"NEW-secret"}}),
        Some(&p),
        Some(1),
    )
    .await;
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "failed", "{op}");
    assert_eq!(op["error"]["code"], "REVISION_CONFLICT");
    assert!(memory_view(&app, &p).await.get("version").is_none());
    assert_eq!(s.wires.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn settings_change_during_post_does_not_activate_old_candidate() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Cel.");
    s.hold.store(true, Ordering::SeqCst);
    let a = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    wait_posts(&s, 1).await;
    memory_settings(&app, &p, json!({"kind":"bounded","tokens":500}), "disabled").await;
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["error"]["code"], "REVISION_CONFLICT");
    assert!(memory_view(&app, &p).await.get("version").is_none());
}
#[tokio::test]
async fn forged_sources_wrong_profile_and_ui_decision_are_rejected_before_http() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    let original = memory_request(&app, &p, &c).await;
    let mut r = original.clone();
    r["payload"]["sourceRefs"][0]["contentSha256"] = json!("0".repeat(64));
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    let mut r = original.clone();
    r["payload"]["sourceRefs"][0]["profileId"] = json!("foreign");
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "PROFILE_SCOPE_MISMATCH"
    );
    let mut r = original;
    r["payload"]["decision"] = json!("accept");
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "VALIDATION_ERROR"
    );
    assert!(s.wires.lock().unwrap().is_empty());
}
#[tokio::test]
async fn actual_scribe_route_receives_proposal_and_primary_receives_decision() {
    let (app, _, _, s, p, c, conn) = memory_fixture().await;
    let snap = ok(&app, "profile.snapshot", json!({}), Some(&p), None).await;
    let route=ok(&app,"route.save",json!({"route":{"role":"scribe","connectionId":conn["id"],"modelId":"scribe-model","outputLimitTokens":1024}}),Some(&p),snap["profile"]["revision"].as_i64()).await;
    let snap = ok(&app, "profile.snapshot", json!({}), Some(&p), None).await;
    ok(
        &app,
        "route.assign",
        json!({"role":"scribe","routeId":route["id"]}),
        Some(&p),
        snap["profile"]["revision"].as_i64(),
    )
    .await;
    memory_settings(
        &app,
        &p,
        json!({"kind":"bounded","tokens":300}),
        "automatic",
    )
    .await;
    propose_answer(&s, "Cel.");
    decide_answer(&s, "modify", "Cel skorygowany.");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    let wires = s.wires.lock().unwrap();
    let bodies: Vec<Value> = wires
        .iter()
        .map(|w| serde_json::from_str(w.split("\r\n\r\n").nth(1).unwrap()).unwrap())
        .collect();
    assert_eq!(bodies[0]["model"], "scribe-model");
    assert_eq!(bodies[1]["model"], "mock-model");
    let v = memory_view(&app, &p).await;
    assert_eq!(v["proposals"][0]["proposerRole"], "scribe");
    assert_eq!(
        v["version"]["measurement"]["modelRouteId"],
        v["primary"]["id"]
    );
}
#[tokio::test]
async fn final_transaction_failure_does_not_activate_or_leave_orphan_version() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    sqlx::query("CREATE TRIGGER fail_memory BEFORE INSERT ON memory_versions BEGIN SELECT RAISE(ABORT,'test'); END").execute(&pool).await.unwrap();
    propose_answer(&s, "Cel.");
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["error"]["code"], "STORAGE_ERROR", "{op}");
    assert!(memory_view(&app, &p).await.get("version").is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn budget_preflight_rejects_without_paid_calls_or_pending_operation() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    let old = memory_view(&app, &p).await;
    ok(&app,"memory.settings.set",json!({"scopeId":old["scopeId"],"capacityPolicy":{"kind":"bounded","tokens":300},"scribeMode":"disabled","primaryContextTokens":1024}),Some(&p),old["profileRevision"].as_i64()).await;
    let r = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    assert_eq!(r["error"]["code"], "CONTEXT_BUDGET_EXCEEDED", "{r}");
    assert!(s.wires.lock().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn retry_transport_and_two_capacity_attempts_share_durable_four_post_budget() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    s.responses.lock().unwrap().push_back((429, "busy".into()));
    propose_answer(&s, "Cel.");
    decide_answer(&s, "modify", &"x".repeat(301));
    propose_answer(&s, "Cel 2.");
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["error"]["code"], "OPERATION_BUDGET_EXCEEDED", "{op}");
    assert_eq!(op["physicalCallCount"], 4);
    assert_eq!(s.wires.lock().unwrap().len(), 4);
    let v = memory_view(&app, &p).await;
    assert!(v.get("version").is_none());
    assert_eq!(v["attempts"][0]["result"], "capacity_rejected");
    assert_eq!(v["proposals"][1]["state"], "failed");
}
#[tokio::test]
async fn source_change_without_revision_bump_still_invalidates_snapshot() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Cel.");
    s.hold.store(true, Ordering::SeqCst);
    let a = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    wait_posts(&s, 1).await;
    sqlx::query("UPDATE messages SET body=json_set(body,'$.content','Changed source') WHERE profile_id=? AND sequence=1").bind(&p).execute(&pool).await.unwrap();
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["error"]["code"], "REVISION_CONFLICT");
    assert!(memory_view(&app, &p).await.get("version").is_none());
    assert_eq!(s.wires.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn changed_model_invalidates_memory_context_declaration() {
    let (app, _, _, _s, p, c, conn) = memory_fixture().await;
    let v = memory_view(&app, &p).await;
    ok(&app,"route.save",json!({"routeId":v["primary"]["id"],"route":{"role":"primary","connectionId":conn["id"],"modelId":"new-model","outputLimitTokens":1024}}),Some(&p),v["primary"]["revision"].as_i64()).await;
    assert_eq!(memory_view(&app, &p).await["primary"]["contextTokens"], 0);
    let r = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    assert_eq!(r["error"]["code"], "MODEL_CAPABILITY_UNKNOWN");
}
#[tokio::test]
async fn record_cas_conflict_and_foreign_record_do_not_make_network_calls() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    let mut r = memory_request(&app, &p, &c).await;
    r["expectedRevision"] = json!(99);
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    let q = profile(&app, "foreign").await;
    let mut r = memory_request(&app, &p, &c).await;
    r["payload"]["recordId"] =
        memory_view(&app, q["id"].as_str().unwrap()).await["record"]["id"].clone();
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    assert!(s.wires.lock().unwrap().is_empty());
}
#[tokio::test]
async fn committed_memory_and_interrupted_pending_work_survive_real_file_restart() {
    let path = std::env::temp_dir().join(format!("ai-home-memory-{}.sqlite", uuid::Uuid::new_v4()));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let vault = Arc::new(MemoryVault::default());
    let app = Application::with_vault(store, Vault::with_backend(vault.clone()));
    let (app, pool, vault, s, p, c, _) = configure(app, pool, vault).await;
    let a = app
        .dispatch(send_request(&p, &c, "Pamięć po restarcie."))
        .await
        .response;
    finished(&app, &p, &a["result"]["operation"]["id"]).await;
    memory_settings(&app, &p, json!({"kind":"bounded","tokens":300}), "disabled").await;
    propose_answer(&s, "Trwały cel.");
    decide_answer(&s, "accept", "");
    let done = update(&app, &p, &c).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    let old = memory_view(&app, &p).await["version"].clone();
    // Persist the exact queued/run snapshot as a process lost between accepted and commit.
    let req = memory_request(&app, &p, &c).await;
    let pending_id = uuid::Uuid::new_v4().to_string();
    let mut op = done.clone();
    op["id"] = json!(pending_id);
    op["status"] = json!("running");
    op["revision"] = json!(1);
    op["physicalCallCount"] = json!(1);
    op.as_object_mut().unwrap().remove("result");
    op.as_object_mut().unwrap().remove("finishedAt");
    assert!(schema::dto("Operation", &op));
    sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,?,1,?)")
        .bind(&p)
        .bind(&pending_id)
        .bind(op.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let mut snap: Value = serde_json::from_str(
        &sqlx::query_scalar::<_, String>(
            "SELECT body FROM memory_runs WHERE profile_id=? AND operation_id=?",
        )
        .bind(&p)
        .bind(done["id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap(),
    )
    .unwrap();
    snap["record"] = memory_view(&app, &p).await["record"].clone();
    snap["version"] = old.clone();
    sqlx::query("INSERT INTO memory_runs(profile_id,operation_id,record_id,body) VALUES(?,?,?,?)")
        .bind(&p)
        .bind(&pending_id)
        .bind(req["payload"]["recordId"].as_str().unwrap())
        .bind(snap.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let mut proposal = memory_view(&app, &p).await["proposals"]
        .as_array()
        .unwrap()
        .first()
        .cloned(); // Latest pending has no proposal yet.
    if proposal.is_none() {
        proposal = Some(
            json!({"schemaVersion":1,"id":"pending-proposal","profileId":p,"revision":1,"createdAt":"2026-10-21T00:00:00Z","operationId":pending_id,"memoryRecordId":req["payload"]["recordId"],"memoryKind":"working","proposerRole":"primary","proposerRouteId":snap["routes"]["primary"]["route"]["id"],"content":"Pending","language":"pl","sources":snap["sources"],"state":"pending","capacityPolicySnapshot":{"kind":"bounded","tokens":300},"attemptNumber":1,"claimKind":"mixed"}),
        );
    }
    let pr = proposal.unwrap();
    assert!(schema::dto("MemoryProposal", &pr));
    sqlx::query("INSERT INTO memory_proposals(profile_id,id,revision,body) VALUES(?,?,1,?)")
        .bind(&p)
        .bind(pr["id"].as_str().unwrap())
        .bind(pr.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let posts = s.wires.lock().unwrap().len();
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(vault));
    let v = memory_view(&app, &p).await;
    assert_eq!(v["version"], old);
    assert_eq!(v["latestOperation"]["status"], "interrupted");
    assert_eq!(
        v["latestOperation"]["error"]["code"],
        "OPERATION_INTERRUPTED"
    );
    assert_eq!(v["proposals"][0]["state"], "failed");
    assert_eq!(s.wires.lock().unwrap().len(), posts);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    drop(app);
    pool.close().await;
    let _ = std::fs::remove_file(path);
}
#[tokio::test]
async fn version_eight_migration_seeds_without_changing_profiles_messages_or_preferences() {
    let path = std::env::temp_dir().join(format!("ai-home-v8-{}.sqlite", uuid::Uuid::new_v4()));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(Arc::new(MemoryVault::default())));
    let p = profile(&app, "Migrated").await;
    let pref = ok(&app, "app.preferences.get", json!({}), None, None).await;
    let conv = ok(
        &app,
        "conversation.create",
        json!({"title":"Before migration","language":"pl"}),
        p["id"].as_str(),
        None,
    )
    .await;
    let message = json!({"schemaVersion":1,"id":"before-v9","profileId":p["id"],"revision":1,"createdAt":"2026-10-21T00:00:00Z","conversationId":conv["id"],"sequence":1,"role":"human","content":"Zachowaj źródło.","language":"pl","status":"complete","sourceRefs":[]});
    assert!(schema::dto("Message", &message));
    sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,?,?,1,1,?)").bind(p["id"].as_str().unwrap()).bind("before-v9").bind(conv["id"].as_str().unwrap()).bind(message.to_string()).execute(&pool).await.unwrap();
    for table in [
        "memory_versions",
        "memory_attempts",
        "memory_decisions",
        "memory_proposals",
        "memory_runs",
        "memory_contexts",
        "memory_records",
    ] {
        sqlx::query(&format!("DROP TABLE {table}"))
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("PRAGMA user_version=8")
        .execute(&pool)
        .await
        .unwrap();
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let v = memory_view(&app, p["id"].as_str().unwrap()).await;
    assert_eq!(v["record"]["state"], "empty");
    assert_eq!(
        ok(&app, "profile.snapshot", json!({}), p["id"].as_str(), None).await["profile"],
        p
    );
    assert_eq!(
        ok(&app, "app.preferences.get", json!({}), None, None).await,
        pref
    );
    assert_eq!(
        ok(
            &app,
            "conversation.messages.list",
            json!({"conversationId":conv["id"],"page":{"limit":100}}),
            p["id"].as_str(),
            None
        )
        .await["items"],
        json!([message])
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        12
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    drop(app);
    pool.close().await;
    let _ = std::fs::remove_file(path);
}
#[tokio::test]
async fn explicit_translation_uses_profile_language_without_ui_locale() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    sqlx::query("UPDATE profiles SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.languagePolicy','explicit_translation','$.memoryLanguage','en') WHERE id=?").bind(&p).execute(&pool).await.unwrap();
    answer(
        &s,
        json!({"content":"Goal: AI Home.","language":"en","claimKind":"mixed"}),
    );
    decide_answer(&s, "accept", "");
    let op = update(&app, &p, &c).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(memory_view(&app, &p).await["version"]["language"], "en");
    assert!(s.wires.lock().unwrap()[0].contains("\\\"language\\\":\\\"en\\\""));
}
#[tokio::test]
async fn lowering_capacity_does_not_remove_or_rewrite_previous_version() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Poprzednia dobra pamięć.");
    decide_answer(&s, "accept", "");
    assert_eq!(update(&app, &p, &c).await["status"], "succeeded");
    let old = memory_view(&app, &p).await["version"].clone();
    memory_settings(&app, &p, json!({"kind":"bounded","tokens":1}), "disabled").await;
    assert_eq!(memory_view(&app, &p).await["version"], old);
    propose_answer(&s, "Za długa.");
    propose_answer(&s, "Nadal za długa.");
    assert_eq!(
        update(&app, &p, &c).await["error"]["code"],
        "MEMORY_CAPACITY_EXCEEDED"
    );
    assert_eq!(memory_view(&app, &p).await["version"], old);
}
#[tokio::test]
async fn receipt_failure_rolls_back_acceptance_before_any_paid_post() {
    let (app, pool, _, s, p, c, _) = memory_fixture().await;
    sqlx::query("CREATE TRIGGER memory_receipt_failure BEFORE INSERT ON receipts WHEN json_extract(NEW.response,'$.result.operation.kind')='memory_update' BEGIN SELECT RAISE(ABORT,'test'); END").execute(&pool).await.unwrap();
    let r = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    assert_eq!(r["error"]["code"], "STORAGE_ERROR");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(s.wires.lock().unwrap().is_empty());
}
#[tokio::test]
async fn provider_auth_failure_keeps_old_memory_and_durable_safe_error() {
    let (app, _, _, s, p, c, _) = memory_fixture().await;
    propose_answer(&s, "Dobry cel.");
    decide_answer(&s, "accept", "");
    assert_eq!(update(&app, &p, &c).await["status"], "succeeded");
    let old = memory_view(&app, &p).await["version"].clone();
    s.responses
        .lock()
        .unwrap()
        .push_back((401, "PRIVATE provider dump secret".into()));
    let op = update(&app, &p, &c).await;
    assert_eq!(op["error"]["code"], "PROVIDER_AUTHENTICATION_FAILED");
    let v = memory_view(&app, &p).await;
    assert_eq!(v["version"], old);
    assert!(!v.to_string().contains("PRIVATE provider dump"));
}
#[tokio::test]
async fn changed_endpoint_rebound_key_and_successful_retest_reject_old_scope_and_late_memory() {
    let (app, _, _, s, p, c, conn) = memory_fixture().await;
    let other = server().await;
    let old = memory_view(&app, &p).await;
    propose_answer(&s, "Cel.");
    s.hold.store(true, Ordering::SeqCst);
    let a = app
        .dispatch(memory_request(&app, &p, &c).await)
        .await
        .response;
    wait_posts(&s, 1).await;
    ok(&app,"connection.update",json!({"connectionId":conn["id"],"connection":{"providerKind":"custom_openai","endpoint":other.endpoint,"label":"New endpoint"}}),Some(&p),Some(1)).await;
    ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":conn["id"],"credential":{"secret":"NEW-secret"}}),
        Some(&p),
        Some(2),
    )
    .await;
    let test = ok(
        &app,
        "connection.test",
        json!({"connectionId":conn["id"]}),
        Some(&p),
        Some(3),
    )
    .await;
    assert_eq!(
        finished(&app, &p, &test["operation"]["id"]).await["status"],
        "succeeded"
    );
    let r=invoke(&app,"memory.settings.set",json!({"scopeId":old["scopeId"],"capacityPolicy":{"kind":"bounded","tokens":500},"scribeMode":"disabled","primaryContextTokens":32768}),Some(&p),old["profileRevision"].as_i64()).await;
    assert_eq!(r["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(memory_view(&app, &p).await["primary"]["contextTokens"], 0);
    s.hold.store(false, Ordering::SeqCst);
    s.release.add_permits(10);
    let op = finished(&app, &p, &a["result"]["operation"]["id"]).await;
    assert_eq!(op["error"]["code"], "REVISION_CONFLICT");
    assert!(memory_view(&app, &p).await.get("version").is_none());
    assert_eq!(s.wires.lock().unwrap().len(), 1);
    assert!(other.wires.lock().unwrap().is_empty());
}

async fn automatic_memory_with_windows(app:&Application,p:&str,conn:&Value,primary_context:i64,scribe_context:i64){
    let snap=ok(app,"profile.snapshot",json!({}),Some(p),None).await;
    let route=ok(app,"route.save",json!({"route":{"role":"scribe","connectionId":conn["id"],"modelId":"scribe-model","outputLimitTokens":1024}}),Some(p),snap["profile"]["revision"].as_i64()).await;
    let snap=ok(app,"profile.snapshot",json!({}),Some(p),None).await;
    ok(app,"route.assign",json!({"role":"scribe","routeId":route["id"]}),Some(p),snap["profile"]["revision"].as_i64()).await;
    let v=memory_view(app,p).await;
    ok(app,"memory.settings.set",json!({"scopeId":v["scopeId"],"capacityPolicy":{"kind":"bounded","tokens":300},"scribeMode":"automatic","primaryContextTokens":primary_context,"scribeContextTokens":scribe_context}),Some(p),v["profileRevision"].as_i64()).await;
}
async fn memory_storage_counts(pool:&sqlx::SqlitePool)->Vec<i64>{
    let mut counts=vec![];
    for table in ["operations","receipts","profile_events","memory_runs","memory_proposals","memory_decisions","memory_attempts","memory_versions"]{
        counts.push(sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(pool).await.unwrap());
    }
    counts
}
#[tokio::test]
async fn automatic_scribe_impossible_primary_budget_rejected_before_any_post_or_write(){
    // 1024 is already smaller than output 1024 + reserve 512. At 2048 those
    // fit alone, but the minimum decision instructions and source data do not.
    for primary_context in [1024,2048]{
        let(app,pool,_,s,p,c,conn)=memory_fixture().await;
        automatic_memory_with_windows(&app,&p,&conn,primary_context,32768).await;
        let before=memory_storage_counts(&pool).await;let old=memory_view(&app,&p).await;
        propose_answer(&s,"Cel.");
        let response=app.dispatch(memory_request(&app,&p,&c).await).await.response;
        // Before the correction this receives Accepted, sends one real loopback
        // POST to Scribe, then fails in the Primary request builder.
        let unexpected=if response["ok"]==true{Some(finished(&app,&p,&response["result"]["operation"]["id"]).await)}else{None};
        assert_eq!(response["ok"],false,"known-impossible Primary was accepted: {response}; terminal={unexpected:?}; POSTs={}",s.wires.lock().unwrap().len());
        assert_eq!(response["error"]["code"],"CONTEXT_BUDGET_EXCEEDED");
        assert!(schema::dto("Error",&response["error"]));
        assert!(s.wires.lock().unwrap().is_empty(),"must not pay for Scribe when Primary cannot run");
        assert_eq!(memory_storage_counts(&pool).await,before,"rejection must not create operation, receipt, event, proposal or version");
        assert_eq!(memory_view(&app,&p).await,old);
    }
}
#[tokio::test]
async fn automatic_scribe_impossible_proposer_budget_also_rejected_without_posts(){
    let(app,pool,_,s,p,c,conn)=memory_fixture().await;
    automatic_memory_with_windows(&app,&p,&conn,32768,1024).await;
    let before=memory_storage_counts(&pool).await;
    let r=app.dispatch(memory_request(&app,&p,&c).await).await.response;
    assert_eq!(r["error"]["code"],"CONTEXT_BUDGET_EXCEEDED","{r}");
    assert!(s.wires.lock().unwrap().is_empty());assert_eq!(memory_storage_counts(&pool).await,before);
}
#[tokio::test]
async fn one_physical_call_cannot_cover_proposer_and_primary_decision(){
    let(app,pool,_,s,p,c,_)=memory_fixture().await;
    sqlx::query("UPDATE profiles SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.contextBudget.maxPhysicalCalls',1) WHERE id=?").bind(&p).execute(&pool).await.unwrap();
    let before=memory_storage_counts(&pool).await;
    let r=app.dispatch(memory_request(&app,&p,&c).await).await.response;
    assert_eq!(r["error"]["code"],"OPERATION_BUDGET_EXCEEDED","{r}");
    assert!(s.wires.lock().unwrap().is_empty());assert_eq!(memory_storage_counts(&pool).await,before);
}
