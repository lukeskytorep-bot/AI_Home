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
    calls: Arc<AtomicUsize>,
    release: Arc<Semaphore>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(status: &str, blocked: bool) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let release = Arc::new(Semaphore::new(if blocked { 0 } else { 1 }));
    let gate = release.clone();
    let status = status.to_string();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut data = vec![];
        loop {
            let mut buffer = [0; 2048];
            let n = stream.read(&mut buffer).await.unwrap();
            if n == 0 {
                return;
            }
            data.extend_from_slice(&buffer[..n]);
            if data.windows(4).any(|v| v == b"\r\n\r\n") {
                break;
            }
        }
        count.fetch_add(1, Ordering::SeqCst);
        let _permit = gate.acquire().await.unwrap();
        let body = if status.starts_with("200") {
            r#"{"data":[{"id":"test-model","name":"Mock"}]}"#
        } else {
            "SENTINEL-secret private provider error"
        };
        let wire = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(wire.as_bytes()).await;
    });
    Server {
        endpoint,
        calls,
        release,
        task,
    }
}
async fn received(s: &Server) {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while s.calls.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
}
async fn test(app: &Application, p: &str, c: &Value) -> Value {
    let result = ok(
        app,
        "connection.test",
        json!({"connectionId":c["id"]}),
        Some(p),
        c["revision"].as_i64(),
    )
    .await;
    assert_eq!(result["operation"]["status"], "queued");
    result["operation"].clone()
}
async fn finished(app: &Application, p: &str, op: &Value) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let v = ok(
                app,
                "operation.get",
                json!({"operationId":op["id"]}),
                Some(p),
                None,
            )
            .await;
            if !matches!(v["status"].as_str(), Some("queued") | Some("running")) {
                assert!(schema::dto("Operation", &v));
                return v;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn profiles_are_distinct_and_memory_defaults_survive() {
    let (app, _, _) = setup().await;
    let p = profile(&app, "One").await;
    let q = profile(&app, "One").await;
    assert_ne!(p["id"], q["id"]);
    assert_eq!(p["memorySettings"]["working"]["tokens"], 300);
    assert_eq!(p["contextBudget"]["maxPhysicalCalls"], 4);
    assert_eq!(p["mode"], "standard");
    assert!(schema::dto("Profile", &p));
}
#[tokio::test]
async fn wrong_scope_and_unknown_fields_fail_closed() {
    let (app, _, _) = setup().await;
    let p = profile(&app, "One").await;
    let r = invoke(
        &app,
        "profile.create",
        json!({"displayName":"x"}),
        p["id"].as_str(),
        None,
    )
    .await;
    assert_eq!(r["error"]["code"], "PROFILE_SCOPE_MISMATCH");
    let r = invoke(&app, "profile.snapshot", json!({}), None, None).await;
    assert_eq!(r["error"]["code"], "PROFILE_SCOPE_MISMATCH");
    let r = invoke(
        &app,
        "profile.create",
        json!({"displayName":"x","secret":"bad"}),
        None,
        None,
    )
    .await;
    assert_eq!(r["error"]["code"], "VALIDATION_ERROR");
}
#[tokio::test]
async fn keys_are_not_in_sqlite_or_public_snapshots() {
    let (app, pool, backend) = setup().await;
    let p = profile(&app, "One").await;
    let c = connection(&app, p["id"].as_str().unwrap(), "http://localhost:1/v1").await;
    assert!(!c.to_string().contains("SENTINEL"));
    let records=sqlx::query("SELECT body AS value FROM connections UNION ALL SELECT response AS value FROM receipts UNION ALL SELECT fingerprint AS value FROM receipts UNION ALL SELECT body AS value FROM profile_events").fetch_all(&pool).await.unwrap();
    for row in records {
        assert!(!row.get::<&str, _>("value").contains("SENTINEL"));
    }
    assert!(backend
        .values
        .lock()
        .unwrap()
        .values()
        .any(|v| v.contains("SENTINEL")));
    assert_ne!(p["id"], c["credentialRef"]);
}
#[tokio::test]
async fn idempotency_secret_comparison_survives_without_db_secret_digest() {
    let (app, _, _) = setup().await;
    let p = profile(&app, "One").await;
    let r = request(
        "connection.create",
        json!({"connection":{"providerKind":"custom_openai","endpoint":"http://localhost:1/v1","label":"One"},"credential":{"secret":"SENTINEL-secret"}}),
        p["id"].as_str(),
        None,
    );
    let first = app.dispatch(r.clone()).await.response;
    assert!(first["ok"].as_bool().unwrap());
    assert_eq!(app.dispatch(r.clone()).await.response, first);
    let mut altered = r;
    altered["payload"]["credential"]["secret"] = json!("other");
    assert_eq!(
        app.dispatch(altered).await.response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
}
#[tokio::test]
async fn failed_vault_readback_rolls_back_connection_and_receipt() {
    let (app, pool, backend) = setup().await;
    let p = profile(&app, "One").await;
    backend.discard.store(true, Ordering::SeqCst);
    let r=invoke(&app,"connection.create",json!({"connection":{"providerKind":"openai","endpoint":"","label":"One"},"credential":{"secret":"secret"}}),p["id"].as_str(),None).await;
    assert_eq!(r["error"]["code"], "CREDENTIAL_UNAVAILABLE");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM connections")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn key_failure_preserves_last_connection() {
    let (app, _, backend) = setup().await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, "http://localhost:1/v1").await;
    backend.fail.store(true, Ordering::SeqCst);
    let r = invoke(
        &app,
        "connection.credential.set",
        json!({"connectionId":c["id"],"credential":{"secret":"new"}}),
        Some(pid),
        Some(1),
    )
    .await;
    assert_eq!(r["error"]["code"], "CREDENTIAL_UNAVAILABLE");
    let snap = ok(&app, "profile.snapshot", json!({}), Some(pid), None).await;
    assert_eq!(snap["connections"][0], c);
}
#[tokio::test]
async fn profile_scope_isolation_includes_models_operations_and_routes() {
    let (app, _, _) = setup().await;
    let p = profile(&app, "One").await;
    let q = profile(&app, "Two").await;
    let c = connection(&app, p["id"].as_str().unwrap(), "http://localhost:1/v1").await;
    for cmd in [
        "connection.update",
        "connection.test",
        "connection.models.list",
        "connection.test.latest",
    ] {
        let payload = if cmd == "connection.update" {
            json!({"connectionId":c["id"],"connection":{"providerKind":"openai","endpoint":"","label":"x"}})
        } else {
            json!({"connectionId":c["id"]})
        };
        let r = invoke(&app, cmd, payload, q["id"].as_str(), Some(1)).await;
        assert_eq!(r["error"]["code"], "NOT_FOUND");
    }
    let op = test(&app, p["id"].as_str().unwrap(), &c).await;
    let r = invoke(
        &app,
        "operation.get",
        json!({"operationId":op["id"]}),
        q["id"].as_str(),
        None,
    )
    .await;
    assert_eq!(r["error"]["code"], "NOT_FOUND");
}
#[tokio::test]
async fn real_http_test_persists_attestation_and_models() {
    let (app, pool, _) = setup().await;
    let s = server("200 OK", false).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["physicalCallCount"], 1);
    assert_eq!(result["result"]["connectionRevision"], 1);
    let snap = ok(&app, "profile.snapshot", json!({}), Some(pid), None).await;
    assert_eq!(snap["connections"][0]["status"], "verified");
    let list = ok(
        &app,
        "connection.models.list",
        json!({"connectionId":c["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(list["items"][0]["id"], "test-model");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM profile_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        7
    );
}
#[tokio::test]
async fn safe_failure_has_durable_error_without_provider_body() {
    let (app, _, _) = setup().await;
    let s = server("401 Unauthorized", false).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["error"]["code"], "PROVIDER_AUTHENTICATION_FAILED");
    assert!(!result.to_string().contains("SENTINEL"));
    assert!(!result.to_string().contains("private"));
}
#[tokio::test]
async fn changed_endpoint_requires_reentry_and_does_not_dispatch() {
    let (app, _, _) = setup().await;
    let s = server("200 OK", false).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, "http://localhost:1/v1").await;
    let c=ok(&app,"connection.update",json!({"connectionId":c["id"],"connection":{"providerKind":"custom_openai","endpoint":s.endpoint,"label":"changed"}}),Some(pid),Some(1)).await;
    let op = test(&app, pid, &c).await;
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["error"]["code"], "CREDENTIAL_UNAVAILABLE");
    assert_eq!(result["physicalCallCount"], 0);
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn stale_inflight_test_never_overwrites_new_key() {
    let (app, _, _) = setup().await;
    let s = server("200 OK", true).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    received(&s).await;
    let running = ok(
        &app,
        "operation.get",
        json!({"operationId":op["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(running["physicalCallCount"], 1); // Committed before dispatch, even while HTTP is blocked.
    let newer = ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":c["id"],"credential":{"secret":"new-secret"}}),
        Some(pid),
        Some(1),
    )
    .await;
    s.release.add_permits(1);
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["error"]["code"], "STALE_CONNECTION_TEST");
    let snap = ok(&app, "profile.snapshot", json!({}), Some(pid), None).await;
    assert_eq!(snap["connections"][0], newer);
    assert_eq!(newer["credentialRevision"], 2);
    assert_eq!(newer["status"], "unverified");
}
#[tokio::test]
async fn cancellation_is_profile_scoped_idempotent_and_terminal() {
    let (app, _, _) = setup().await;
    let s = server("200 OK", true).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    received(&s).await;
    let r = request(
        "operation.cancel",
        json!({"operationId":op["id"]}),
        Some(pid),
        None,
    );
    let cancelled = app.dispatch(r.clone()).await.response;
    assert_eq!(cancelled["result"]["status"], "cancelled");
    assert_eq!(app.dispatch(r).await.response, cancelled);
    s.release.add_permits(1);
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["status"], "cancelled");
    assert_eq!(result["physicalCallCount"], 1);
}
#[tokio::test]
async fn primary_and_scribe_are_separate_routes_with_same_connection() {
    let (app, _, _) = setup().await;
    let s = server("200 OK", false).await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    finished(&app, pid, &op).await;
    let mut profile = p.clone();
    let mut routes = vec![];
    for (role, model) in [("primary", "test-model"), ("scribe", "another-model")] {
        let route=ok(&app,"route.save",json!({"route":{"role":role,"connectionId":c["id"],"modelId":model,"outputLimitTokens":1024}}),Some(pid),profile["revision"].as_i64()).await;
        profile = ok(&app, "profile.snapshot", json!({}), Some(pid), None).await["profile"].clone();
        profile = ok(
            &app,
            "route.assign",
            json!({"role":role,"routeId":route["id"]}),
            Some(pid),
            profile["revision"].as_i64(),
        )
        .await;
        routes.push(route);
    }
    assert_ne!(routes[0]["id"], routes[1]["id"]);
    assert_eq!(profile["status"], "ready");
    let _ = ok(
        &app,
        "connection.credential.set",
        json!({"connectionId":c["id"],"credential":{"secret":"new-secret"}}),
        Some(pid),
        Some(1),
    )
    .await;
    assert_eq!(
        ok(&app, "profile.snapshot", json!({}), Some(pid), None).await["profile"]["status"],
        "draft"
    );
}
#[tokio::test]
async fn stale_revision_and_unverified_route_are_rejected() {
    let (app, _, _) = setup().await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, "http://localhost:1/v1").await;
    let r = invoke(
        &app,
        "connection.credential.set",
        json!({"connectionId":c["id"],"credential":{"secret":"new-secret"}}),
        Some(pid),
        Some(99),
    )
    .await;
    assert_eq!(r["error"]["code"], "REVISION_CONFLICT");
    let r=invoke(&app,"route.save",json!({"route":{"role":"primary","connectionId":c["id"],"modelId":"m","outputLimitTokens":1}}),Some(pid),Some(1)).await;
    assert_eq!(r["error"]["code"], "CONNECTION_UNVERIFIED");
}
#[tokio::test]
async fn opaque_profile_cursor_is_query_bound() {
    let (app, _, _) = setup().await;
    profile(&app, "A").await;
    profile(&app, "B").await;
    profile(&app, "C").await;
    let page = ok(&app, "profile.list", json!({"limit":1}), None, None).await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let cursor = page["nextCursor"].as_str().unwrap();
    assert!(!cursor.is_empty());
    let r = invoke(
        &app,
        "profile.list",
        json!({"limit":2,"cursor":cursor}),
        None,
        None,
    )
    .await;
    assert_eq!(r["error"]["code"], "VALIDATION_ERROR");
    let next = ok(
        &app,
        "profile.list",
        json!({"limit":1,"cursor":cursor}),
        None,
        None,
    )
    .await;
    assert_ne!(page["items"][0]["id"], next["items"][0]["id"]);
}
#[tokio::test]
async fn restart_recovers_pending_operations_and_preserves_receipts() {
    let path =
        std::env::temp_dir().join(format!("ai-home-connections-{}.db", uuid::Uuid::new_v4()));
    let backend = Arc::new(MemoryVault::default());
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(backend.clone()));
    let r = request(
        "profile.create",
        json!({"displayName":"Persistent"}),
        None,
        None,
    );
    let original = app.dispatch(r.clone()).await.response;
    let p = &original["result"];
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, "http://localhost:1/v1").await;
    let op = json!({"schemaVersion":1,"id":"pending","profileId":pid,"revision":1,"createdAt":"2026-10-12T10:00:00Z","kind":"connection_test","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":1,"limits":{"maxPhysicalCalls":4,"maxElapsedMs":180000},"recordRefs":[{"profileId":pid,"recordKind":"connection","recordId":c["id"]}]});
    sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,?,1,?)")
        .bind(pid)
        .bind("pending")
        .bind(op.to_string())
        .execute(&pool)
        .await
        .unwrap();
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let app = Application::with_vault(store, Vault::with_backend(backend));
    assert_eq!(app.dispatch(r).await.response, original);
    let result = ok(
        &app,
        "operation.get",
        json!({"operationId":"pending"}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(result["status"], "interrupted");
    assert_eq!(result["error"]["code"], "OPERATION_INTERRUPTED");
    let latest = ok(
        &app,
        "connection.test.latest",
        json!({"connectionId":c["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(latest["operation"], result);
    drop(app);
    let _ = std::fs::remove_file(path);
}
#[tokio::test]
async fn foreign_keys_reject_cross_profile_route() {
    let (app, pool, _) = setup().await;
    let p = profile(&app, "One").await;
    let q = profile(&app, "Two").await;
    let c = connection(&app, p["id"].as_str().unwrap(), "http://localhost:1/v1").await;
    assert!(sqlx::query(
        "INSERT INTO routes(profile_id,id,connection_id,revision,body) VALUES(?,?,?,1,'{}')"
    )
    .bind(q["id"].as_str().unwrap())
    .bind("bad")
    .bind(c["id"].as_str().unwrap())
    .execute(&pool)
    .await
    .is_err());
}
#[tokio::test]
async fn vault_rejects_legacy_and_foreign_bound_entries_before_network() {
    for legacy in [true, false] {
        let (app, _, backend) = setup().await;
        let s = server("200 OK", false).await;
        let p = profile(&app, "One").await;
        let pid = p["id"].as_str().unwrap();
        let c = connection(&app, pid, &s.endpoint).await;
        {
            let mut map = backend.values.lock().unwrap();
            let entry = map.get_mut(c["credentialRef"].as_str().unwrap()).unwrap();
            if legacy {
                *entry = "legacy-secret-only".into()
            } else {
                *entry = entry.replace(pid, "foreign-profile");
            }
        }
        let op = test(&app, pid, &c).await;
        let result = finished(&app, pid, &op).await;
        assert_eq!(result["error"]["code"], "CREDENTIAL_UNAVAILABLE");
        assert_eq!(result["physicalCallCount"], 0);
        assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn migration_from_actual_v1_preserves_preferences_and_has_valid_foreign_keys() {
    let path = std::env::temp_dir().join(format!("ai-home-v1-{}.db", uuid::Uuid::new_v4()));
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    for sql in ["CREATE TABLE preferences (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, body TEXT NOT NULL)","CREATE TABLE receipts (request_id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, response TEXT NOT NULL)","CREATE TABLE app_events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL)","PRAGMA user_version=1"]{sqlx::query(sql).execute(&pool).await.unwrap();}
    let prefs = json!({"schemaVersion":1,"revision":7,"locale":"pl","themeId":"aurora","themeVariant":"dark"});
    sqlx::query("INSERT INTO preferences(id,revision,body) VALUES(1,7,?)")
        .bind(prefs.to_string())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    assert_eq!(store.preferences().await.unwrap(), prefs);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        11
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&store.pool)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        "ok"
    );
    store.pool.close().await;
    let _ = std::fs::remove_file(path);
}
#[tokio::test]
async fn failed_sql_commit_keeps_previous_key_binding_and_connection() {
    let (app, pool, backend) = setup().await;
    let p = profile(&app, "One").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, "http://localhost:1/v1").await;
    sqlx::query("CREATE TRIGGER fail_key_receipt BEFORE INSERT ON receipts WHEN NEW.request_id='fault' BEGIN SELECT RAISE(ABORT,'injected storage failure'); END").execute(&pool).await.unwrap();
    let mut r = request(
        "connection.credential.set",
        json!({"connectionId":c["id"],"credential":{"secret":"new-secret"}}),
        Some(pid),
        Some(1),
    );
    r["requestId"] = json!("fault");
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "STORAGE_ERROR"
    );
    assert_eq!(
        ok(&app, "profile.snapshot", json!({}), Some(pid), None).await["connections"][0],
        c
    );
    let map = backend.values.lock().unwrap();
    assert!(map
        .get(c["credentialRef"].as_str().unwrap())
        .unwrap()
        .contains("SENTINEL-secret"));
    assert_eq!(map.len(), 3); // The new immutable entry is orphaned, never used as a fallback.
}
async fn retry_server(first_status: &str, retry_after: &str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let release = Arc::new(Semaphore::new(1));
    let status = first_status.to_string();
    let after = retry_after.to_string();
    let task = tokio::spawn(async move {
        for index in 0..2 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            loop {
                let mut b = [0; 2048];
                let n = stream.read(&mut b).await.unwrap();
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&b[..n]);
                if data.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            counter.fetch_add(1, Ordering::SeqCst);
            let body = if index == 0 {
                "private provider error"
            } else {
                r#"{"data":[{"id":"retried-model"}]}"#
            };
            let status = if index == 0 {
                status.as_str()
            } else {
                "200 OK"
            };
            let metadata = if index == 0 {
                format!("Retry-After: {after}\r\n")
            } else {
                String::new()
            };
            let wire=format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{metadata}\r\n{body}",body.len());
            let _ = stream.write_all(wire.as_bytes()).await;
        }
    });
    Server {
        endpoint,
        calls,
        release,
        task,
    }
}
#[tokio::test]
async fn transient_connection_test_commits_both_attempts_and_one_final_result() {
    let (app, pool, _) = setup().await;
    let s = retry_server("503 Service Unavailable", "0").await;
    let p = profile(&app, "Retry").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    let result = finished(&app, pid, &op).await;
    assert_eq!(result["status"], "succeeded");
    assert_eq!(result["physicalCallCount"], 2);
    assert_eq!(result["result"]["connectionRevision"], 1);
    assert_eq!(s.calls.load(Ordering::SeqCst), 2);
    let events =
        sqlx::query("SELECT body FROM profile_events WHERE profile_id=? ORDER BY sequence")
            .bind(pid)
            .fetch_all(&pool)
            .await
            .unwrap();
    let counts: Vec<u64> = events
        .iter()
        .filter_map(|row| {
            let v: Value = serde_json::from_str(row.get("body")).unwrap();
            if v["eventType"] == "operation.changed" && v["payload"]["status"] == "running" {
                v["payload"]["physicalCallCount"].as_u64()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(counts, vec![0, 1, 2]);
    let cache = ok(
        &app,
        "connection.models.list",
        json!({"connectionId":c["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(cache["items"][0]["id"], "retried-model");
    let latest = ok(
        &app,
        "connection.test.latest",
        json!({"connectionId":c["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(latest["operation"], result);
}
#[tokio::test]
async fn cancelling_connection_backoff_preserves_terminal_count() {
    let (app, _, _) = setup().await;
    let s = retry_server("429 Too Many Requests", "30").await;
    let p = profile(&app, "Retry").await;
    let pid = p["id"].as_str().unwrap();
    let c = connection(&app, pid, &s.endpoint).await;
    let op = test(&app, pid, &c).await;
    received(&s).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let result = ok(
        &app,
        "operation.cancel",
        json!({"operationId":op["id"]}),
        Some(pid),
        None,
    )
    .await;
    assert_eq!(result["status"], "cancelled");
    assert_eq!(result["physicalCallCount"], 1);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    assert_eq!(finished(&app, pid, &op).await["status"], "cancelled");
}

#[tokio::test]
async fn hmac_receipt_survives_restart_and_missing_installation_key_fails_closed() {
    let path = std::env::temp_dir().join(format!("ai-home-hmac-{}.db", uuid::Uuid::new_v4()));
    let backend = Arc::new(MemoryVault::default());
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(backend.clone()));
    let p = profile(&app, "HMAC").await;
    let r = request(
        "connection.create",
        json!({"connection":{"providerKind":"custom_openai","endpoint":"http://localhost:1/v1","label":"HMAC"},"credential":{"secret":"SENTINEL-secret"}}),
        p["id"].as_str(),
        None,
    );
    let original = app.dispatch(r.clone()).await.response;
    assert_eq!(original["ok"], true);
    let fingerprint: String =
        sqlx::query_scalar("SELECT fingerprint FROM receipts WHERE request_id=?")
            .bind(r["requestId"].as_str().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(fingerprint.starts_with("hmac-v1:"));
    assert_eq!(fingerprint.len(), 72);
    let reference: String =
        sqlx::query_scalar("SELECT credential_ref FROM installation_hmac WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!reference.contains("SENTINEL"));
    assert!(backend.values.lock().unwrap()[&reference].starts_with("ai-home-installation-hmac:v1:"));
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let app = Application::with_vault(store, Vault::with_backend(backend.clone()));
    assert_eq!(app.dispatch(r.clone()).await.response, original);
    let mut changed = r.clone();
    changed["payload"]["credential"]["secret"] = json!("other-secret");
    assert_eq!(
        app.dispatch(changed).await.response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    backend.values.lock().unwrap().remove(&reference);
    let before = backend.values.lock().unwrap().len();
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "CREDENTIAL_UNAVAILABLE"
    );
    assert_eq!(backend.values.lock().unwrap().len(), before);
    drop(app);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn version_two_legacy_receipt_is_read_only_and_checks_original_secret() {
    use sha2::{Digest, Sha256};
    let path = std::env::temp_dir().join(format!("ai-home-legacy-{}.db", uuid::Uuid::new_v4()));
    let backend = Arc::new(MemoryVault::default());
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(backend.clone()));
    let p = profile(&app, "Legacy").await;
    let r = request(
        "connection.create",
        json!({"connection":{"providerKind":"custom_openai","endpoint":"http://localhost:1/v1","label":"Legacy"},"credential":{"secret":"SENTINEL-secret"}}),
        p["id"].as_str(),
        None,
    );
    let original = app.dispatch(r.clone()).await.response;
    assert_eq!(original["ok"], true);
    let mut payload = r["payload"].clone();
    payload.as_object_mut().unwrap().remove("credential");
    let legacy = format!(
        "{:x}",
        Sha256::digest(
            json!([
                "connection.create",
                p["id"].as_str().unwrap(),
                r["expectedRevision"].as_i64().unwrap_or(0),
                payload
            ])
            .to_string()
            .as_bytes()
        )
    );
    sqlx::query("UPDATE receipts SET fingerprint=? WHERE request_id=?")
        .bind(&legacy)
        .bind(r["requestId"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE installation_hmac")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=2")
        .execute(&pool)
        .await
        .unwrap();
    backend
        .values
        .lock()
        .unwrap()
        .retain(|_, v| !v.starts_with("ai-home-installation-hmac:v1:"));
    let before = backend.values.lock().unwrap().clone();
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(backend.clone()));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        11
    );
    assert_eq!(app.dispatch(r.clone()).await.response, original);
    let mut changed = r.clone();
    changed["payload"]["credential"]["secret"] = json!("other-secret");
    assert_eq!(
        app.dispatch(changed).await.response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM installation_hmac")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT fingerprint FROM receipts WHERE request_id=?")
            .bind(r["requestId"].as_str().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap(),
        legacy
    );
    assert_eq!(*backend.values.lock().unwrap(), before);
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}
