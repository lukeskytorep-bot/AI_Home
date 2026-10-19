use ai_home::{
    application::Application,
    domain::schema,
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use serde_json::{json, Value};
use sqlx::Row;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Semaphore,
};
#[derive(Default)]
struct MemoryVault {
    values: Mutex<HashMap<String, String>>,
    fail: AtomicBool,
    discard: AtomicBool,
}
impl SecretStore for MemoryVault {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(VaultError::Unavailable);
        }
        Ok(self.values.lock().unwrap().get(id).cloned())
    }
    fn write_new(&self, id: &str, value: &str) -> Result<(), VaultError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(VaultError::Unavailable);
        }
        if !self.discard.load(Ordering::SeqCst) {
            let mut map = self.values.lock().unwrap();
            if map.contains_key(id) {
                return Err(VaultError::Exists);
            }
            map.insert(id.into(), value.into());
        }
        Ok(())
    }
}
fn request(command: &str, payload: Value, profile: Option<&str>, expected: Option<i64>) -> Value {
    let mut r = json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});
    if let Some(p) = profile {
        r["profileId"] = json!(p)
    }
    if let Some(e) = expected {
        r["expectedRevision"] = json!(e)
    }
    r
}
async fn invoke(
    app: &Application,
    command: &str,
    payload: Value,
    profile: Option<&str>,
    expected: Option<i64>,
) -> Value {
    app.dispatch(request(command, payload, profile, expected))
        .await
        .response
}
async fn ok(
    app: &Application,
    command: &str,
    payload: Value,
    profile: Option<&str>,
    expected: Option<i64>,
) -> Value {
    let r = invoke(app, command, payload, profile, expected).await;
    assert_eq!(r["ok"], true, "{r}");
    r["result"].clone()
}
async fn setup() -> (Application, sqlx::SqlitePool, Arc<MemoryVault>) {
    let store = Store::memory().await.unwrap();
    let pool = store.pool.clone();
    let backend = Arc::new(MemoryVault::default());
    (
        Application::with_vault(store, Vault::with_backend(backend.clone())),
        pool,
        backend,
    )
}
async fn profile(app: &Application, name: &str) -> Value {
    ok(
        app,
        "profile.create",
        json!({"displayName":name}),
        None,
        None,
    )
    .await
}
async fn connection(app: &Application, p: &str, endpoint: &str) -> Value {
    ok(app,"connection.create",json!({"connection":{"providerKind":"custom_openai","endpoint":endpoint,"label":"Local test"},"credential":{"secret":"SENTINEL-secret"}}),Some(p),None).await
}
struct Server {
    endpoint: String,
    wires: Arc<Mutex<Vec<String>>>,
    responses: Arc<Mutex<std::collections::VecDeque<(u16, String)>>>,
    hold: Arc<AtomicBool>,
    release: Arc<Semaphore>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let wires = Arc::new(Mutex::new(vec![]));
    let responses = Arc::new(Mutex::new(
        std::collections::VecDeque::<(u16, String)>::new(),
    ));
    let hold = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Semaphore::new(0));
    let (w, q, h, g) = (
        wires.clone(),
        responses.clone(),
        hold.clone(),
        release.clone(),
    );
    let task = tokio::spawn(async move {
        let mut children = tokio::task::JoinSet::new();
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let (w, q, h, g) = (w.clone(), q.clone(), h.clone(), g.clone());
            children.spawn(async move{let mut data=vec![];let end=loop{let mut b=[0;2048];let n=socket.read(&mut b).await.unwrap();if n==0{return;}data.extend_from_slice(&b[..n]);if let Some(i)=data.windows(4).position(|x|x==b"\r\n\r\n"){break i+4;}};let head=String::from_utf8_lossy(&data[..end]).to_lowercase();let len=head.lines().find_map(|x|x.strip_prefix("content-length:").and_then(|s|s.trim().parse::<usize>().ok())).unwrap_or(0);while data.len()<end+len{let mut b=[0;2048];let n=socket.read(&mut b).await.unwrap();if n==0{return;}data.extend_from_slice(&b[..n]);}let raw=String::from_utf8(data).unwrap();let post=raw.starts_with("POST ");if post{w.lock().unwrap().push(raw);if h.load(Ordering::SeqCst){g.acquire().await.unwrap().forget();}}let(status,body)=if post{q.lock().unwrap().pop_front().unwrap_or((200,r#"{"choices":[{"message":{"content":"Reply 🪐"},"finish_reason":"stop"}]}"#.into()))}else{(200,r#"{"data":[{"id":"mock-model"}]}"#.into())};let response=format!("HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\nRetry-After: 0\r\n\r\n{body}",body.len());let _=socket.write_all(response.as_bytes()).await;});
        }
    });
    Server {
        endpoint,
        wires,
        responses,
        hold,
        release,
        task,
    }
}
async fn finished(app: &Application, p: &str, id: &Value) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let op = ok(
                app,
                "operation.get",
                json!({"operationId":id}),
                Some(p),
                None,
            )
            .await;
            if !matches!(op["status"].as_str(), Some("queued") | Some("running")) {
                assert!(schema::dto("Operation", &op), "{op}");
                return op;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
async fn configured() -> (
    Application,
    sqlx::SqlitePool,
    Arc<MemoryVault>,
    Server,
    String,
    Value,
    Value,
) {
    let (app, pool, vault) = setup().await;
    let server = server().await;
    let p = profile(&app, "Chat").await;
    let pid = p["id"].as_str().unwrap().to_string();
    let c = connection(&app, &pid, &server.endpoint).await;
    let accepted = ok(
        &app,
        "connection.test",
        json!({"connectionId":c["id"]}),
        Some(&pid),
        Some(1),
    )
    .await;
    assert_eq!(
        finished(&app, &pid, &accepted["operation"]["id"]).await["status"],
        "succeeded"
    );
    let route=ok(&app,"route.save",json!({"route":{"role":"primary","connectionId":c["id"],"modelId":"mock-model","outputLimitTokens":1024}}),Some(&pid),Some(1)).await;
    ok(
        &app,
        "route.assign",
        json!({"role":"primary","routeId":route["id"]}),
        Some(&pid),
        Some(2),
    )
    .await;
    ok(
        &app,
        "chat.settings.set",
        json!({"routeId":route["id"],"contextTokens":32768}),
        Some(&pid),
        Some(1),
    )
    .await;
    let conv = ok(
        &app,
        "conversation.create",
        json!({"title":"Chat","language":"en"}),
        Some(&pid),
        None,
    )
    .await;
    (app, pool, vault, server, pid, conv, c)
}
fn send_request(p: &str, c: &Value, text: &str) -> Value {
    request(
        "chat.send",
        json!({"conversationId":c["id"],"content":text,"language":"pl","attachmentIds":[]}),
        Some(p),
        c["revision"].as_i64(),
    )
}
async fn history(app: &Application, p: &str, c: &Value) -> Value {
    ok(
        app,
        "conversation.messages.list",
        json!({"conversationId":c["id"],"page":{"limit":100}}),
        Some(p),
        None,
    )
    .await["items"]
        .clone()
}
async fn received(s: &Server) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while s.wires.lock().unwrap().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn successful_chat_is_atomic_durable_and_replay_is_idempotent() {
    let (app, pool, _, s, p, c, _) = configured().await;
    let r = send_request(&p, &c, "Żółć <think>literal</think>");
    let accepted = app.dispatch(r.clone()).await.response;
    assert_eq!(accepted["ok"], true, "{accepted}");
    let opid = &accepted["result"]["operation"]["id"];
    let op = finished(&app, &p, opid).await;
    assert_eq!(op["status"], "succeeded", "{op}");
    assert_eq!(op["physicalCallCount"], 1);
    let m = history(&app, &p, &c).await;
    assert_eq!(m.as_array().unwrap().len(), 2);
    assert_eq!(m[0]["content"], "Żółć <think>literal</think>");
    assert_eq!(m[1]["content"], "Reply 🪐");
    assert_eq!(op["result"]["userMessageId"], m[0]["id"]);
    assert_eq!(app.dispatch(r).await.response, accepted);
    assert_eq!(s.wires.lock().unwrap().len(), 1);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    let latest = ok(
        &app,
        "conversation.chat.latest",
        json!({"conversationId":c["id"]}),
        Some(&p),
        None,
    )
    .await;
    assert_eq!(latest["operation"], op);
    assert!(!op.to_string().contains("SENTINEL"));
}
#[tokio::test]
async fn second_turn_contains_full_ordered_history_and_smaller_output_limit() {
    let (app, _, _, s, p, c, _) = configured().await;
    let a = app.dispatch(send_request(&p, &c, "First")).await.response;
    finished(&app, &p, &a["result"]["operation"]["id"]).await;
    let c = ok(
        &app,
        "conversation.get",
        json!({"conversationId":c["id"]}),
        Some(&p),
        None,
    )
    .await;
    let a = app.dispatch(send_request(&p, &c, "Second")).await.response;
    assert_eq!(
        finished(&app, &p, &a["result"]["operation"]["id"]).await["status"],
        "succeeded"
    );
    let w = s.wires.lock().unwrap();
    let body: Value = serde_json::from_str(w[1].split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["messages"][1]["content"], "First");
    assert_eq!(body["messages"][2]["content"], "Reply 🪐");
    assert_eq!(body["messages"][3]["content"], "Second");
    assert_eq!(body["max_tokens"], 1024);
}
#[tokio::test]
async fn cancellation_keeps_user_and_never_commits_late_assistant() {
    let (app, _, _, s, p, c, _) = configured().await;
    s.hold.store(true, Ordering::SeqCst);
    let r = app.dispatch(send_request(&p, &c, "Cancel")).await.response;
    received(&s).await;
    let opid = &r["result"]["operation"]["id"];
    let cancelled = ok(
        &app,
        "operation.cancel",
        json!({"operationId":opid}),
        Some(&p),
        None,
    )
    .await;
    assert_eq!(cancelled["status"], "cancelled");
    s.release.add_permits(1);
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(finished(&app, &p, opid).await["status"], "cancelled");
    assert_eq!(history(&app, &p, &c).await.as_array().unwrap().len(), 1);
    assert_eq!(s.wires.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn draft_saved_during_generation_survives_answer_commit() {
    let (app, _, _, s, p, c, _) = configured().await;
    s.hold.store(true, Ordering::SeqCst);
    let r = app.dispatch(send_request(&p, &c, "First")).await.response;
    received(&s).await;
    ok(
        &app,
        "conversation.draft.set",
        json!({"conversationId":c["id"],"text":"Next message"}),
        Some(&p),
        Some(2),
    )
    .await;
    s.release.add_permits(1);
    assert_eq!(
        finished(&app, &p, &r["result"]["operation"]["id"]).await["status"],
        "succeeded"
    );
    assert_eq!(
        ok(
            &app,
            "conversation.get",
            json!({"conversationId":c["id"]}),
            Some(&p),
            None
        )
        .await["draftText"],
        "Next message"
    );
}
#[tokio::test]
async fn scope_conflict_attachments_and_unknown_context_do_not_dispatch() {
    let (app, pool, _, s, p, c, _) = configured().await;
    let q = profile(&app, "Foreign").await;
    let mut r = send_request(&p, &c, "No");
    r["profileId"] = q["id"].clone();
    assert_eq!(app.dispatch(r).await.response["error"]["code"], "NOT_FOUND");
    let mut r = send_request(&p, &c, "No");
    r["expectedRevision"] = json!(9);
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    let mut r = send_request(&p, &c, "No");
    r["payload"]["attachmentIds"] = json!(["missing"]);
    let rejected = app.dispatch(r).await.response;
    assert_eq!(rejected["error"]["code"], "VALIDATION_ERROR");
    assert!(schema::dto("Error", &rejected["error"]), "{rejected}");
    assert!(schema::dto("ResultEnvelope", &rejected), "{rejected}");
    sqlx::query("DELETE FROM chat_policies")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        app.dispatch(send_request(&p, &c, "No")).await.response["error"]["code"],
        "MODEL_CAPABILITY_UNKNOWN"
    );
    assert!(s.wires.lock().unwrap().is_empty());
    assert!(history(&app, &p, &c).await.as_array().unwrap().is_empty());
}
#[tokio::test]
async fn context_overflow_never_trims_or_records_a_user_message() {
    let (app, pool, _, s, p, c, _) = configured().await;
    sqlx::query("UPDATE chat_policies SET context_tokens=2048")
        .execute(&pool)
        .await
        .unwrap();
    let r = app
        .dispatch(send_request(&p, &c, &"🪐".repeat(600)))
        .await
        .response;
    assert_eq!(r["error"]["code"], "CONTEXT_BUDGET_EXCEEDED");
    assert!(s.wires.lock().unwrap().is_empty());
    assert!(history(&app, &p, &c).await.as_array().unwrap().is_empty());
}
#[tokio::test]
async fn retry_uses_one_budget_and_commits_only_one_answer() {
    let (app, _, _, s, p, c, _) = configured().await;
    s.responses
        .lock()
        .unwrap()
        .push_back((503, "private".into()));
    let r = app.dispatch(send_request(&p, &c, "Retry")).await.response;
    let op = finished(&app, &p, &r["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "succeeded");
    assert_eq!(op["physicalCallCount"], 2);
    assert_eq!(history(&app, &p, &c).await.as_array().unwrap().len(), 2);
    let wires = s.wires.lock().unwrap();
    assert_eq!(wires.len(), 2);
    assert_eq!(wires[0], wires[1]);
}
#[tokio::test]
async fn key_change_during_request_never_attaches_stale_answer() {
    let (app, _, _, s, p, c, connection) = configured().await;
    s.hold.store(true, Ordering::SeqCst);
    let r = app.dispatch(send_request(&p, &c, "Stale")).await.response;
    received(&s).await;
    ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":connection["id"],"credential":{"secret":"other-key"}}),
        Some(&p),
        Some(1),
    )
    .await;
    s.release.add_permits(1);
    let op = finished(&app, &p, &r["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "failed", "{op}");
    assert_eq!(op["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(history(&app, &p, &c).await.as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn incomplete_output_and_authentication_failure_never_create_an_answer() {
    for response in [
        (
            200,
            r#"{"choices":[{"message":{"content":"Partial"},"finish_reason":"length"}]}"#,
        ),
        (401, "SECRET private error"),
    ] {
        let (app, _, _, s, p, c, _) = configured().await;
        s.responses
            .lock()
            .unwrap()
            .push_back((response.0, response.1.into()));
        let r = app
            .dispatch(send_request(&p, &c, "No partial"))
            .await
            .response;
        let op = finished(&app, &p, &r["result"]["operation"]["id"]).await;
        assert_eq!(op["status"], "failed");
        assert_eq!(op["physicalCallCount"], 1);
        assert!(!op.to_string().contains("SECRET"));
        if response.0 == 401 {
            assert_eq!(op["error"]["retryable"], false);
        }
        assert_eq!(history(&app, &p, &c).await.as_array().unwrap().len(), 1);
        assert_eq!(s.wires.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn receipt_failure_rolls_back_chat_acceptance_without_network() {
    let (app, pool, _, s, p, c, _) = configured().await;
    sqlx::query("CREATE TRIGGER reject_chat BEFORE INSERT ON receipts WHEN NEW.request_id='fault-chat' BEGIN SELECT RAISE(ABORT,'fault'); END").execute(&pool).await.unwrap();
    let mut r = send_request(&p, &c, "Fault");
    r["requestId"] = json!("fault-chat");
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "STORAGE_ERROR"
    );
    assert!(s.wires.lock().unwrap().is_empty());
    assert!(history(&app, &p, &c).await.as_array().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn route_revision_change_requires_fresh_context_declaration() {
    let (app, _, _, s, p, c, connection) = configured().await;
    let snapshot = ok(&app, "profile.snapshot", json!({}), Some(&p), None).await;
    let route = &snapshot["routes"][0];
    ok(&app,"route.save",json!({"routeId":route["id"],"route":{"role":"primary","connectionId":connection["id"],"modelId":"changed-model","outputLimitTokens":1024}}),Some(&p),route["revision"].as_i64()).await;
    let settings = ok(&app, "chat.settings.get", json!({}), Some(&p), None).await;
    assert_eq!(settings["contextTokens"], 0);
    assert_eq!(
        app.dispatch(send_request(&p, &c, "Blocked")).await.response["error"]["code"],
        "MODEL_CAPABILITY_UNKNOWN"
    );
    assert!(s.wires.lock().unwrap().is_empty());
}
#[tokio::test]
async fn final_storage_failure_does_not_commit_an_orphan_assistant() {
    let (app, pool, _, _server, p, c, _) = configured().await;
    sqlx::query("CREATE TRIGGER reject_answer BEFORE INSERT ON messages WHEN json_extract(NEW.body,'$.role')='assistant' BEGIN SELECT RAISE(ABORT,'fault'); END").execute(&pool).await.unwrap();
    let r = app
        .dispatch(send_request(&p, &c, "Keep user"))
        .await
        .response;
    let op = finished(&app, &p, &r["result"]["operation"]["id"]).await;
    assert_eq!(op["status"], "failed");
    assert_eq!(op["error"]["code"], "STORAGE_ERROR");
    assert_eq!(history(&app, &p, &c).await.as_array().unwrap().len(), 1);
}
#[tokio::test]
async fn restart_marks_pending_chat_interrupted_without_network_replay() {
    let path =
        std::env::temp_dir().join(format!("ai-home-chat-restart-{}.db", uuid::Uuid::new_v4()));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let p = profile(&app, "Restart").await;
    let pid = p["id"].as_str().unwrap();
    let c = ok(
        &app,
        "conversation.create",
        json!({"title":"Old","language":"pl"}),
        Some(pid),
        None,
    )
    .await;
    let user = json!({"schemaVersion":1,"id":"u1","profileId":pid,"revision":1,"createdAt":"2026-10-18T20:00:00Z","conversationId":c["id"],"sequence":1,"role":"human","content":"Original","language":"pl","status":"complete","sourceRefs":[],"operationId":"pending"});
    sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,'u1',?,1,1,?)").bind(pid).bind(c["id"].as_str().unwrap()).bind(user.to_string()).execute(&pool).await.unwrap();
    let op = json!({"schemaVersion":1,"id":"pending","profileId":pid,"revision":1,"createdAt":"2026-10-18T20:00:00Z","kind":"chat","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"limits":{"maxPhysicalCalls":4,"maxElapsedMs":180000},"recordRefs":[{"profileId":pid,"recordKind":"message","recordId":"u1"}]});
    sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,'pending',1,?)")
        .bind(pid)
        .bind(op.to_string())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO chat_runs(profile_id,operation_id,conversation_id,body) VALUES(?,'pending',?,'{}')").bind(pid).bind(c["id"].as_str().unwrap()).execute(&pool).await.unwrap();
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let op = finished(&app, pid, &json!("pending")).await;
    assert_eq!(op["status"], "interrupted");
    assert_eq!(op["physicalCallCount"], 0);
    assert_eq!(history(&app, pid, &c).await[0]["content"], "Original");
    assert_eq!(
        ok(
            &app,
            "conversation.chat.latest",
            json!({"conversationId":c["id"]}),
            Some(pid),
            None
        )
        .await["operation"],
        op
    );
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn actual_v4_database_migrates_without_changing_profile_or_preferences() {
    let path = std::env::temp_dir().join(format!("ai-home-chat-v4-{}.db", uuid::Uuid::new_v4()));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let p = profile(&app, "Legacy").await;
    let pref = ok(&app, "app.preferences.get", json!({}), None, None).await;
    for statement in [
        "DROP TABLE chat_replay",
        "DROP TABLE chat_runs",
        "DROP TABLE chat_policies",
        "PRAGMA user_version=4",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    assert_eq!(
        ok(&app, "app.preferences.get", json!({}), None, None).await,
        pref
    );
    assert_eq!(
        ok(&app, "profile.snapshot", json!({}), p["id"].as_str(), None).await["profile"],
        p
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        5
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "ok"
    );
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn another_send_while_running_is_rejected_without_duplicate_dispatch() {
    let (app, _, _, s, p, c, _) = configured().await;
    s.hold.store(true, Ordering::SeqCst);
    let first = app.dispatch(send_request(&p, &c, "First")).await.response;
    received(&s).await;
    let c = ok(
        &app,
        "conversation.get",
        json!({"conversationId":c["id"]}),
        Some(&p),
        None,
    )
    .await;
    let second = app.dispatch(send_request(&p, &c, "Another")).await.response;
    assert_eq!(second["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(s.wires.lock().unwrap().len(), 1);
    ok(
        &app,
        "operation.cancel",
        json!({"operationId":first["result"]["operation"]["id"]}),
        Some(&p),
        None,
    )
    .await;
    s.release.add_permits(1);
}
