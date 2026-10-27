//! Real dispatcher + both SQLite stores; deterministic fake OS-secret backend.
use ai_home::{
    application::Application,
    community_host::CommunityLifecycle,
    community_identity::{
        registry::{lifecycle, RegistrationRegistry},
        tokens::CommunityTokenStore,
        CommunityKeyStore, CommunityScope, CommunitySecretStore, IdentityError,
        RegistrationProfile,
    },
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use serde_json::{json, Value};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::Notify;
use zeroize::Zeroizing;

#[derive(Default)]
struct Secrets {
    data: Mutex<HashMap<String, Zeroizing<String>>>,
    creates: AtomicUsize,
    removes: AtomicUsize,
    fail: AtomicBool,
    pause: AtomicBool,
    entered: Notify,
    release: (Mutex<bool>, std::sync::Condvar),
}
impl Secrets {
    fn resume(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
    fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}
impl CommunitySecretStore for Secrets {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, v: &str) -> Result<(), IdentityError> {
        let mut data = self.data.lock().unwrap();
        if data.contains_key(r) {
            return Err(IdentityError::AlreadyExists);
        }
        data.insert(r.into(), Zeroizing::new(v.into()));
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        if self.fail.load(Ordering::SeqCst) {
            return Err(IdentityError::VaultUnavailable);
        }
        if self.pause.load(Ordering::SeqCst) {
            self.entered.notify_one();
            let (g, t) = self
                .release
                .1
                .wait_timeout_while(
                    self.release.0.lock().unwrap(),
                    Duration::from_secs(15),
                    |v| !*v,
                )
                .unwrap();
            if t.timed_out() && !*g {
                return Err(IdentityError::VaultUnavailable);
            }
        }
        self.data.lock().unwrap().remove(r);
        self.removes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[derive(Default)]
struct ModelSecrets;
impl SecretStore for ModelSecrets {
    fn read(&self, _: &str) -> Result<Option<String>, VaultError> {
        Ok(None)
    }
    fn write_new(&self, _: &str, _: &str) -> Result<(), VaultError> {
        Err(VaultError::Unavailable)
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ai-home-host-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Fixture {
    temp: Temp,
    pool: SqlitePool,
    registry: RegistrationRegistry,
    keys: CommunityKeyStore,
    tokens: CommunityTokenStore,
    k: Arc<Secrets>,
    t: Arc<Secrets>,
}
impl Fixture {
    async fn new() -> Self {
        let temp = Temp::new();
        let store = Store::open(&temp.0.join("foundation.sqlite3"))
            .await
            .unwrap();
        let pool = store.pool.clone();
        drop(store);
        let registry = RegistrationRegistry::open(&temp.0.join("community.sqlite3"))
            .await
            .unwrap();
        let k = Arc::new(Secrets::default());
        let t = Arc::new(Secrets::default());
        let keys = CommunityKeyStore::with_backend(k.clone());
        let tokens = CommunityTokenStore::with_backend(t.clone());
        Self {
            temp,
            pool,
            registry,
            keys,
            tokens,
            k,
            t,
        }
    }
    fn app(&self, community: bool) -> Application {
        let app = Application::with_vault(
            Store::from_existing_pool(self.pool.clone()),
            Vault::with_backend(Arc::new(ModelSecrets)),
        );
        if community {
            app.with_community_lifecycle(CommunityLifecycle::new(
                self.registry.clone(),
                self.keys.clone(),
                self.tokens.clone(),
            ))
        } else {
            app
        }
    }
    async fn seed(&self, p: &str) -> CommunityScope {
        let s = CommunityScope::new(
            p,
            "https://example.invalid/forum/api/v1/",
            "https://example.invalid",
            "test-community",
        )
        .unwrap();
        self.registry
            .prepare(
                &s,
                &RegistrationProfile::new("Public Agent", "Public about").unwrap(),
                &format!("start-{}", uuid::Uuid::new_v4()),
            )
            .await
            .unwrap();
        self.registry.provision(&self.keys, &s, 1).await.unwrap();
        s
    }
    async fn raw_registry(&self) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(self.temp.0.join("community.sqlite3"))
                    .foreign_keys(true),
            )
            .await
            .unwrap()
    }
    async fn count(&self, table: &str, profile: &str) -> i64 {
        sqlx::query_scalar(&format!(
            "SELECT count(*) FROM {table} WHERE {}=?",
            if table == "profiles" {
                "id"
            } else {
                "profile_id"
            }
        ))
        .bind(profile)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
    async fn receipt(&self, id: &str) -> Option<Value> {
        sqlx::query_scalar::<_, String>("SELECT response FROM receipts WHERE request_id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
            .map(|v| serde_json::from_str(&v).unwrap())
    }
}
fn req(command: &str, p: Option<&str>, rev: Option<i64>, payload: Value) -> Value {
    let mut r = json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});
    if let Some(p) = p {
        r["profileId"] = json!(p);
    }
    if let Some(v) = rev {
        r["expectedRevision"] = json!(v);
    }
    r
}
async fn ok(app: &Application, r: Value) -> Value {
    let v = app.dispatch(r).await.response;
    assert_eq!(v["ok"], true, "{v}");
    v["result"].clone()
}
async fn profile(app: &Application) -> Value {
    ok(
        app,
        req(
            "profile.create",
            None,
            None,
            json!({"displayName":"Owner profile"}),
        ),
    )
    .await
}
async fn archived(app: &Application, p: &Value) -> Value {
    ok(
        app,
        req(
            "profile.archive",
            Some(p["id"].as_str().unwrap()),
            p["revision"].as_i64(),
            json!({"archived":true}),
        ),
    )
    .await
}
fn deletion(p: &Value) -> Value {
    req(
        "profile.delete.permanent",
        p["id"].as_str(),
        p["revision"].as_i64(),
        json!({"confirm":true}),
    )
}
async fn err(app: &Application, r: Value, code: &str) {
    let v = app.dispatch(r).await.response;
    assert_eq!(v["ok"], false, "{v}");
    assert_eq!(v["error"]["code"], code, "{v}");
    assert!(ai_home::domain::schema::dto("Error", &v["error"]), "{v}");
}

#[tokio::test]
async fn delete_coordinates_real_stores_orphan_token_receipts_and_other_profile() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let create = req(
        "profile.create",
        None,
        None,
        json!({"displayName":"Original"}),
    );
    let p = ok(&app, create.clone()).await;
    let a = p["id"].as_str().unwrap();
    let scope = f.seed(a).await;
    let other = profile(&app).await;
    let b = other["id"].as_str().unwrap();
    f.seed(b).await;
    // A journaled candidate without a ready pointer must still be removed.
    let token_ref = format!(
        "ai-home-community-token:v1:{}",
        uuid::Uuid::new_v4().simple()
    );
    f.t.create(
        &token_ref,
        &format!(
            "AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={}\nfixture=orphan\n",
            scope.binding()
        ),
    )
    .unwrap();
    let cp = f.raw_registry().await;
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,'{}')")
        .bind(scope.binding())
        .execute(&cp)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_dispatches VALUES(?,?,1,'complete','{}',?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(scope.binding())
        .bind(&token_ref)
        .execute(&cp)
        .await
        .unwrap();
    cp.close().await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    let response = app.dispatch(r.clone()).await.response;
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(f.count("profiles", a).await, 0);
    assert_eq!(f.count("profile_deletion_jobs", a).await, 0);
    assert_eq!(
        lifecycle::state(&f.registry, a).await.unwrap().as_deref(),
        Some("deleted")
    );
    assert_eq!(f.k.len(), 1);
    assert_eq!(f.t.len(), 0);
    assert_eq!(f.count("profiles", b).await, 1);
    assert_eq!(
        lifecycle::state(&f.registry, b).await.unwrap().as_deref(),
        Some("active")
    );
    assert_eq!(app.dispatch(r.clone()).await.response, response);
    assert_eq!(
        app.dispatch(create).await.response["error"]["messageKey"],
        "error.permanently_deleted"
    );
    drop(app);
    let restarted = f.app(true);
    assert_eq!(restarted.dispatch(r).await.response, response);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn vault_failure_preserves_intent_blocks_mutations_and_resumes_with_fresh_id_after_restart() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let pid = p["id"].as_str().unwrap();
    f.seed(pid).await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    f.k.fail.store(true, Ordering::SeqCst);
    err(&app, r.clone(), "CREDENTIAL_UNAVAILABLE").await;
    assert_eq!(f.count("profiles", pid).await, 1);
    assert_eq!(f.count("profile_deletion_jobs", pid).await, 1);
    assert!(f.receipt(r["requestId"].as_str().unwrap()).await.is_none());
    assert_eq!(
        lifecycle::state(&f.registry, pid).await.unwrap().as_deref(),
        Some("deleting")
    );
    err(
        &app,
        req(
            "profile.archive",
            Some(pid),
            p["revision"].as_i64(),
            json!({"archived":false}),
        ),
        "POLICY_DENIED",
    )
    .await;
    let mut reused = req("app.preferences.get", None, None, json!({}));
    reused["requestId"] = r["requestId"].clone();
    err(&app, reused, "IDEMPOTENCY_CONFLICT").await;
    err(
        &app,
        req("profile.export.create", Some(pid), None, json!({})),
        "POLICY_DENIED",
    )
    .await;
    drop(app);
    let detached = f.app(false);
    err(&detached, r.clone(), "POLICY_DENIED").await;
    drop(detached);
    f.k.fail.store(false, Ordering::SeqCst);
    let app = f.app(true);
    let retry = deletion(&p);
    let done = app.dispatch(retry.clone()).await.response;
    assert_eq!(done["ok"], true, "{done}");
    let original = app.dispatch(r.clone()).await.response;
    assert_eq!(original["result"], done["result"]);
    assert_eq!(original["requestId"], r["requestId"]);
    assert_eq!(
        f.receipt(retry["requestId"].as_str().unwrap()).await,
        Some(done)
    );
    assert_eq!(f.k.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn final_app_transaction_failure_never_reports_success_and_retries_cleaned_community() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    sqlx::query("CREATE TRIGGER fixture_fail_delete BEFORE DELETE ON profiles BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&f.pool).await.unwrap();
    err(&app, r.clone(), "STORAGE_ERROR").await;
    assert_eq!(f.count("profiles", id).await, 1);
    assert_eq!(f.count("profile_deletion_jobs", id).await, 1);
    assert_eq!(f.k.len(), 0);
    assert!(f.receipt(r["requestId"].as_str().unwrap()).await.is_none());
    assert_eq!(
        lifecycle::state(&f.registry, id).await.unwrap().as_deref(),
        Some("deleted")
    );
    sqlx::query("DROP TRIGGER fixture_fail_delete")
        .execute(&f.pool)
        .await
        .unwrap();
    drop(app);
    let reopened = Store::open(&f.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profile_deletion_jobs")
            .fetch_one(&reopened.pool)
            .await
            .unwrap(),
        1
    );
    drop(reopened);
    let app = f.app(true);
    ok(&app, r).await;
    assert_eq!(f.count("profiles", id).await, 0);
    assert_eq!(f.k.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abandoned_dispatch_keeps_cleanup_gate_and_eventually_commits_receipt() {
    let f = Fixture::new().await;
    let app = Arc::new(f.app(true));
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    f.k.pause.store(true, Ordering::SeqCst);
    let a = app.clone();
    let rr = r.clone();
    let waiter = tokio::spawn(async move { a.dispatch(rr).await });
    tokio::time::timeout(Duration::from_secs(10), f.k.entered.notified())
        .await
        .unwrap();
    waiter.abort();
    let _ = waiter.await;
    assert_eq!(f.count("profile_deletion_jobs", id).await, 1);
    assert!(f.receipt(r["requestId"].as_str().unwrap()).await.is_none());
    f.k.resume();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if f.receipt(r["requestId"].as_str().unwrap()).await.is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(app.dispatch(r).await.response["ok"], true);
    assert_eq!(f.count("profiles", id).await, 0);
}

#[tokio::test]
async fn preflight_rejects_stale_revision_unarchived_and_active_experimental_before_vault() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    err(&app, deletion(&p), "POLICY_DENIED").await;
    let p = archived(&app, &p).await;
    let mut stale = deletion(&p);
    stale["expectedRevision"] = json!(1);
    err(&app, stale, "REVISION_CONFLICT").await;
    let mut invalid = deletion(&p);
    invalid["payload"]["confirm"] = json!(false);
    err(&app, invalid, "VALIDATION_ERROR").await;
    sqlx::query(
        "INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES(?,'exp',1,?)",
    )
    .bind(id)
    .bind(json!({"status":"running"}).to_string())
    .execute(&f.pool)
    .await
    .unwrap();
    err(&app, deletion(&p), "POLICY_DENIED").await;
    assert_eq!(f.count("profile_deletion_jobs", id).await, 0);
    assert_eq!(f.k.removes.load(Ordering::SeqCst), 0);
    assert_eq!(f.k.len(), 1);
    assert_eq!(
        lifecycle::state(&f.registry, id).await.unwrap().as_deref(),
        Some("active")
    );
}

#[tokio::test]
async fn queued_operation_prevents_cleanup_and_inactive_experimental_can_be_deleted() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    let op = json!({"status":"queued"});
    sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,'queued',1,?)")
        .bind(id)
        .bind(op.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    err(&app, deletion(&p), "POLICY_DENIED").await;
    assert_eq!(f.count("profile_deletion_jobs", id).await, 0);
    assert_eq!(f.k.removes.load(Ordering::SeqCst), 0);
    sqlx::query("DELETE FROM operations WHERE id='queued'")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES(?,'exp',1,?)",
    )
    .bind(id)
    .bind(json!({"status":"inactive"}).to_string())
    .execute(&f.pool)
    .await
    .unwrap();
    ok(&app, deletion(&p)).await;
    assert_eq!(f.count("experimental_instances", id).await, 0);
    assert_eq!(f.k.len(), 0);
}

#[tokio::test]
async fn attached_host_refuses_incomplete_backups_but_standard_v16_backup_still_works() {
    let f = Fixture::new().await;
    let store = Store::open(&f.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    let app = Application::new(store).with_community_lifecycle(CommunityLifecycle::new(
        f.registry.clone(),
        f.keys.clone(),
        f.tokens.clone(),
    ));
    for (command, payload) in [
        ("backup.create", json!({})),
        ("backup.restore.preflight", json!({"backupId":"missing"})),
        ("backup.restore.request", json!({"backupId":"missing"})),
    ] {
        err(&app, req(command, None, None, payload), "POLICY_DENIED").await;
    }
    drop(app);
    let app = Application::new(
        Store::open(&f.temp.0.join("foundation.sqlite3"))
            .await
            .unwrap(),
    );
    let backup = ok(&app, req("backup.create", None, None, json!({}))).await;
    assert_eq!(backup["schemaVersion"], 16);
    assert!(ai_home::domain::schema::dto(
        "Error",
        &json!({"code":"POLICY_DENIED","messageKey":"error.policy_denied","retryable":false})
    ));
}

#[tokio::test]
async fn pending_intent_blocks_backup_even_after_detaching_and_preserves_main_data() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    f.k.fail.store(true, Ordering::SeqCst);
    err(&app, deletion(&p), "CREDENTIAL_UNAVAILABLE").await;
    let store = Store::open(&f.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    let request = req("backup.create", None, None, json!({}));
    assert!(ai_home::backup::dispatch(&store, &request).await.is_err());
    let detached = Application::new(store);
    err(&detached, request, "POLICY_DENIED").await;
    assert_eq!(f.count("profiles", id).await, 1);
}

#[tokio::test]
async fn v15_migrates_to_v16_without_altering_existing_records_or_preferences() {
    let f = Fixture::new().await;
    let app = f.app(false);
    let p = profile(&app).await;
    let before: String = sqlx::query_scalar("SELECT body FROM profiles WHERE id=?")
        .bind(p["id"].as_str().unwrap())
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let preferences = ok(&app, req("app.preferences.get", None, None, json!({}))).await;
    sqlx::query("DROP TABLE profile_deletion_jobs")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE community_host_binding")
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=15")
        .execute(&f.pool)
        .await
        .unwrap();
    drop(app);
    let upgraded = Store::open(&f.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&upgraded.pool)
            .await
            .unwrap(),
        16
    );
    assert_eq!(upgraded.preferences().await.unwrap(), preferences);
    let after: String = sqlx::query_scalar("SELECT body FROM profiles WHERE id=?")
        .bind(p["id"].as_str().unwrap())
        .fetch_one(&upgraded.pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profile_deletion_jobs")
            .fetch_one(&upgraded.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn pin_survives_detach_and_wrong_registry_cannot_complete_a_pending_deletion() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    let detached = f.app(false);
    err(&detached, deletion(&p), "POLICY_DENIED").await;
    assert_eq!(f.k.len(), 1);
    f.k.fail.store(true, Ordering::SeqCst);
    let r = deletion(&p);
    err(&app, r.clone(), "CREDENTIAL_UNAVAILABLE").await;
    let other = RegistrationRegistry::open(&f.temp.0.join("wrong-community.sqlite3"))
        .await
        .unwrap();
    let wrong =
        Application::new(Store::from_existing_pool(f.pool.clone())).with_community_lifecycle(
            CommunityLifecycle::new(other.clone(), f.keys.clone(), f.tokens.clone()),
        );
    f.k.fail.store(false, Ordering::SeqCst);
    err(&wrong, r.clone(), "POLICY_DENIED").await;
    assert_eq!(lifecycle::state(&other, id).await.unwrap(), None);
    assert_eq!(f.count("profiles", id).await, 1);
    assert!(f.receipt(r["requestId"].as_str().unwrap()).await.is_none());
    ok(&app, r).await;
    assert_eq!(f.k.len(), 0);
}

#[tokio::test]
async fn prior_queued_restore_cannot_erase_new_native_host_pin_or_deletion_intent() {
    let f = Fixture::new().await;
    let store = Store::open(&f.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    let app = Application::new(store);
    let backup = ok(&app, req("backup.create", None, None, json!({}))).await;
    ok(
        &app,
        req(
            "backup.restore.request",
            None,
            None,
            json!({"backupId":backup["id"]}),
        ),
    )
    .await;
    drop(app);
    // Simulate another native process having committed its intent before this
    // stale restore ledger was noticed. Startup must abort without replacement.
    sqlx::query("INSERT INTO community_host_binding VALUES(1,?)")
        .bind("a".repeat(64))
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(ai_home::backup::apply_pending_restore(&f.temp.0)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM community_host_binding")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn token_vault_failure_after_key_removal_keeps_profile_and_retries_without_new_keys() {
    let f = Fixture::new().await;
    let app = f.app(true);
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    let scope = f.seed(id).await;
    let reference = format!(
        "ai-home-community-token:v1:{}",
        uuid::Uuid::new_v4().simple()
    );
    f.t.create(
        &reference,
        &format!(
            "AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={}\nfixture=orphan\n",
            scope.binding()
        ),
    )
    .unwrap();
    let cp = f.raw_registry().await;
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,'{}')")
        .bind(scope.binding())
        .execute(&cp)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_dispatches VALUES(?,?,1,'complete','{}',?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(scope.binding())
        .bind(&reference)
        .execute(&cp)
        .await
        .unwrap();
    cp.close().await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    f.t.fail.store(true, Ordering::SeqCst);
    err(&app, r.clone(), "CREDENTIAL_UNAVAILABLE").await;
    assert_eq!(f.k.len(), 0);
    assert_eq!(f.t.len(), 1);
    assert_eq!(f.count("profiles", id).await, 1);
    assert!(f.receipt(r["requestId"].as_str().unwrap()).await.is_none());
    f.t.fail.store(false, Ordering::SeqCst);
    ok(&app, r).await;
    assert_eq!(f.k.creates.load(Ordering::SeqCst), 1);
    assert_eq!(f.t.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_hosts_can_replay_one_deletion_without_false_failure() {
    let f = Fixture::new().await;
    let app = Arc::new(f.app(true));
    let other = Arc::new(f.app(true));
    let p = profile(&app).await;
    let id = p["id"].as_str().unwrap();
    f.seed(id).await;
    let p = archived(&app, &p).await;
    let r = deletion(&p);
    f.k.pause.store(true, Ordering::SeqCst);
    let a = app.clone();
    let rr = r.clone();
    let first = tokio::spawn(async move { a.dispatch(rr).await.response });
    tokio::time::timeout(Duration::from_secs(10), f.k.entered.notified())
        .await
        .unwrap();
    let b = other.clone();
    let rr = r.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let second = tokio::spawn(async move {
        let _ = started.send(());
        b.dispatch(rr).await.response
    });
    ready.await.unwrap();
    f.k.resume();
    let (a, b) = tokio::time::timeout(Duration::from_secs(10), async {
        (first.await.unwrap(), second.await.unwrap())
    })
    .await
    .unwrap();
    assert_eq!(a["ok"], true, "{a}");
    assert_eq!(b, a);
    assert_eq!(f.count("profiles", id).await, 0);
    assert_eq!(f.k.creates.load(Ordering::SeqCst), 1);
}
