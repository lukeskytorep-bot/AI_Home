use super::*;
use crate::{
    application::Application,
    community_host::CommunityLifecycle,
    community_identity::{
        registry::RegistryError, tokens::CommunityTokenStore, CommunityKeyStore, CommunityScope,
        CommunitySecretStore, IdentityError, RegistrationProfile,
    },
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use zeroize::Zeroizing;
#[derive(Default)]
struct Secrets {
    data: Mutex<HashMap<String, Zeroizing<String>>>,
    reads: AtomicUsize,
    creates: AtomicUsize,
    removes: AtomicUsize,
}
impl CommunitySecretStore for Secrets {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, v: &str) -> Result<(), IdentityError> {
        let mut d = self.data.lock().unwrap();
        if d.contains_key(r) {
            return Err(IdentityError::AlreadyExists);
        }
        d.insert(r.into(), Zeroizing::new(v.into()));
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.data.lock().unwrap().remove(r);
        self.removes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct NoModels;
impl SecretStore for NoModels {
    fn read(&self, _: &str) -> Result<Option<String>, VaultError> {
        Err(VaultError::Unavailable)
    }
    fn write_new(&self, _: &str, _: &str) -> Result<(), VaultError> {
        Err(VaultError::Unavailable)
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("aihome-bundle-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&p).unwrap();
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
    root: PathBuf,
    registry: RegistrationRegistry,
    keys: CommunityKeyStore,
    secret: Arc<Secrets>,
}
impl Fixture {
    async fn new() -> Self {
        let temp = Temp::new();
        let root = temp.0.join("app");
        private_dir(&root).unwrap();
        let store = Store::open(&root.join("foundation.sqlite3")).await.unwrap();
        store.pool.close().await;
        drop(store);
        let registry = RegistrationRegistry::open(&root.join("community.sqlite3"))
            .await
            .unwrap();
        let secret = Arc::new(Secrets::default());
        let keys = CommunityKeyStore::with_backend(secret.clone());
        Self {
            temp,
            root,
            registry,
            keys,
            secret,
        }
    }
    fn core(&self) -> OfflineCommunityBackup {
        OfflineCommunityBackup::new(&self.root, self.registry.clone()).unwrap()
    }
    fn bundle(&self) -> PathBuf {
        self.temp.0.join("backup 'Żółw'")
    }
    async fn app(&self) -> (Application, sqlx::SqlitePool) {
        let store = Store::open(&self.root.join("foundation.sqlite3"))
            .await
            .unwrap();
        let p = store.pool.clone();
        (
            Application::with_vault(store, Vault::with_backend(Arc::new(NoModels)))
                .with_community_lifecycle(CommunityLifecycle::new(
                    self.registry.clone(),
                    self.keys.clone(),
                    CommunityTokenStore::with_backend(Arc::new(Secrets::default())),
                )),
            p,
        )
    }
    async fn seed(&self) -> (Value, CommunityScope) {
        let (app, pool) = self.app().await;
        let p = ok(
            &app,
            request_cmd(
                "profile.create",
                None,
                None,
                json!({"displayName":"Private Local Profile"}),
            ),
        )
        .await;
        let id = p["id"].as_str().unwrap();
        let s = CommunityScope::new(
            id,
            "https://example.invalid/installation/api/v1/",
            "https://example.invalid",
            "stable-community",
        )
        .unwrap();
        self.registry
            .prepare(
                &s,
                &RegistrationProfile::new("Public Agent", "Public description").unwrap(),
                &uuid::Uuid::new_v4().to_string(),
            )
            .await
            .unwrap();
        self.registry.provision(&self.keys, &s, 1).await.unwrap();
        ok(
            &app,
            request_cmd(
                "conversation.create",
                Some(id),
                None,
                json!({"title":"Original conversation","language":"en"}),
            ),
        )
        .await;
        drop(app);
        pool.close().await;
        drop(pool);
        (p, s)
    }
    async fn main_count(&self) -> i64 {
        let mut c = connection(&self.root.join("foundation.sqlite3"), true, false)
            .await
            .unwrap();
        let n = sqlx::query_scalar("SELECT count(*) FROM profiles")
            .fetch_one(&mut c)
            .await
            .unwrap();
        c.close().await.unwrap();
        n
    }
    async fn reg_conn(&self) -> sqlx::SqliteConnection {
        connection(&self.root.join("community.sqlite3"), false, false)
            .await
            .unwrap()
    }
    async fn delete(&self, p: &Value) {
        let (app, pool) = self.app().await;
        let a = ok(
            &app,
            request_cmd(
                "profile.archive",
                p["id"].as_str(),
                p["revision"].as_i64(),
                json!({"archived":true}),
            ),
        )
        .await;
        ok(
            &app,
            request_cmd(
                "profile.delete.permanent",
                p["id"].as_str(),
                a["revision"].as_i64(),
                json!({"confirm":true}),
            ),
        )
        .await;
        drop(app);
        pool.close().await;
        drop(pool);
    }
}
fn request_cmd(command: &str, profile: Option<&str>, rev: Option<i64>, payload: Value) -> Value {
    let mut r = json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});
    if let Some(p) = profile {
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
async fn create(f: &Fixture) -> Value {
    f.core().create(&f.bundle(), "backup-1").await.unwrap()
}
async fn raw_preferences(f: &Fixture) -> Value {
    let mut c = connection(&f.root.join("foundation.sqlite3"), true, false)
        .await
        .unwrap();
    let text: String = sqlx::query_scalar("SELECT body FROM preferences WHERE id=1")
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    serde_json::from_str(&text).unwrap()
}

#[tokio::test]
async fn bundle_captures_both_stores_without_reading_vault_and_replays_original_capture() {
    let f = Fixture::new().await;
    f.seed().await;
    let before = f.secret.reads.load(Ordering::SeqCst);
    let m = create(&f).await;
    assert_eq!(m["profileCount"], 1);
    assert_eq!(m["communityProfileCount"], 1);
    assert_eq!(m["vaultSecretsIncluded"], false);
    assert_eq!(f.secret.reads.load(Ordering::SeqCst), before);
    assert_eq!(f.core().create(&f.bundle(), "backup-1").await.unwrap(), m);
    assert_eq!(
        f.core()
            .create(&f.bundle(), "different-request")
            .await
            .unwrap_err(),
        BackupError::IdempotencyConflict
    );
    assert_eq!(std::fs::read_dir(f.bundle()).unwrap().count(), 3);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            f.bundle().metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        for n in ["manifest.json", "application.sqlite3", "community.sqlite3"] {
            assert_eq!(
                f.bundle().join(n).metadata().unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
#[tokio::test]
async fn open_store_and_surviving_pool_clone_block_offline_work() {
    let f = Fixture::new().await;
    f.seed().await;
    let store = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    let pool = store.pool.clone();
    drop(store);
    assert_eq!(
        f.core().create(&f.bundle(), "backup-1").await.unwrap_err(),
        BackupError::Busy
    );
    assert!(!f.bundle().exists());
    pool.close().await;
    drop(pool);
    create(&f).await;
}
#[tokio::test]
async fn restores_main_preferences_and_preserves_live_registry_generation() {
    let f = Fixture::new().await;
    let (_, scope) = f.seed().await;
    let mut c = f.reg_conn().await;
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,?)")
        .bind(scope.binding())
        .bind(json!({"state":"ready","tokenRef":"original-reference","generation":7}).to_string())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    create(&f).await;
    let (app, pool) = f.app().await;
    ok(
        &app,
        request_cmd(
            "app.preferences.set",
            None,
            Some(1),
            json!({"locale":"pl","themeId":"aurora","themeVariant":"dark"}),
        ),
    )
    .await;
    drop(app);
    pool.close().await;
    drop(pool);
    let mut c = f.reg_conn().await;
    sqlx::query("UPDATE community_registration_finishes SET revision=2,body=?")
        .bind(json!({"state":"ready","tokenRef":"newer-reference","generation":9}).to_string())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let mut copied = connection(&f.bundle().join("community.sqlite3"), true, true)
        .await
        .unwrap();
    let copied_body: String =
        sqlx::query_scalar("SELECT body FROM community_registration_finishes")
            .fetch_one(&mut copied)
            .await
            .unwrap();
    copied.close().await.unwrap();
    let copied_body: Value = serde_json::from_str(&copied_body).unwrap();
    assert_eq!(copied_body["state"], "uncertain");
    assert!(copied_body["tokenRef"].is_null());
    let reads = f.secret.reads.load(Ordering::SeqCst);
    let core = f.core();
    let result = core
        .restore(&f.bundle(), "restore-1", RestorePolicy::SameInstallation)
        .await
        .unwrap();
    assert_eq!(result["applied"], true);
    assert_eq!(raw_preferences(&f).await["locale"], "en");
    let mut c = f.reg_conn().await;
    let body: String = sqlx::query_scalar("SELECT body FROM community_registration_finishes")
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["generation"], 9);
    assert_eq!(body["tokenRef"], "newer-reference");
    assert_eq!(f.secret.reads.load(Ordering::SeqCst), reads);
    assert_eq!(
        core.restore(&f.bundle(), "restore-1", RestorePolicy::SameInstallation)
            .await
            .unwrap(),
        result
    );
    assert_eq!(core.receipt("restore-1").unwrap(), Some(result));
}
#[tokio::test]
async fn locally_deleted_profile_is_pruned_from_main_backup_without_resurrection() {
    let f = Fixture::new().await;
    let (p, _) = f.seed().await;
    create(&f).await;
    f.delete(&p).await;
    assert_eq!(f.main_count().await, 0);
    let r = f
        .core()
        .restore(
            &f.bundle(),
            "restore-deleted",
            RestorePolicy::SameInstallation,
        )
        .await
        .unwrap();
    assert_eq!(r["prunedProfileCount"], 1);
    assert_eq!(f.main_count().await, 0);
    assert_eq!(
        lifecycle::state(&f.registry, p["id"].as_str().unwrap())
            .await
            .unwrap()
            .as_deref(),
        Some("deleted")
    );
    let (app, pool) = f.app().await;
    let list = ok(
        &app,
        request_cmd("profile.list", None, None, json!({"limit":100})),
    )
    .await;
    assert_eq!(list["items"], json!([]));
    drop(app);
    pool.close().await;
}
#[tokio::test]
async fn fresh_relocation_requires_explicit_policy_and_does_not_create_missing_keys() {
    let f = Fixture::new().await;
    let (p, scope) = f.seed().await;
    create(&f).await;
    let target = Fixture::new().await;
    let core = target.core();
    assert_eq!(
        core.restore(&f.bundle(), "relocate", RestorePolicy::SameInstallation)
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
    let r = core
        .restore(
            &f.bundle(),
            "relocate",
            RestorePolicy::FreshInstallationRelocation,
        )
        .await
        .unwrap();
    assert_eq!(r["applied"], true);
    assert_eq!(target.main_count().await, 1);
    assert_eq!(target.secret.creates.load(Ordering::SeqCst), 0);
    assert_eq!(target.secret.reads.load(Ordering::SeqCst), 0);
    assert_eq!(
        lifecycle::state(&target.registry, p["id"].as_str().unwrap())
            .await
            .unwrap()
            .as_deref(),
        Some("reconcile")
    );
    let reconciled =
        lifecycle::reconcile_profile(&target.registry, &target.keys, p["id"].as_str().unwrap())
            .await
            .unwrap();
    assert!(!reconciled.active);
    assert_eq!(reconciled.missing_keys, 1);
    assert_eq!(target.secret.creates.load(Ordering::SeqCst), 0);
    let mut c = connection(&target.root.join("foundation.sqlite3"), true, false)
        .await
        .unwrap();
    let pin: String = sqlx::query_scalar("SELECT registry_binding FROM community_host_binding")
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(pin, target.registry.host_binding());
    assert_ne!(pin, f.registry.host_binding());
    assert_eq!(
        target.registry.get(&scope).await.unwrap().public_identity(),
        f.registry.get(&scope).await.unwrap().public_identity()
    );
}
#[tokio::test]
async fn restored_identity_reconciles_only_against_original_existing_keyset() {
    let f = Fixture::new().await;
    let (p, _) = f.seed().await;
    create(&f).await;
    let target = Fixture::new().await;
    target
        .core()
        .restore(
            &f.bundle(),
            "same-vault",
            RestorePolicy::FreshInstallationRelocation,
        )
        .await
        .unwrap();
    let before = f.secret.creates.load(Ordering::SeqCst);
    let rec = lifecycle::reconcile_profile(&target.registry, &f.keys, p["id"].as_str().unwrap())
        .await
        .unwrap();
    assert!(rec.active);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), before);
}
#[tokio::test]
async fn cannot_drop_a_newer_live_community_profile_absent_from_backup() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    f.seed().await;
    let before = hash(&f.root.join("foundation.sqlite3")).unwrap();
    assert_eq!(
        f.core()
            .restore(
                &f.bundle(),
                "unsafe-rollback",
                RestorePolicy::SameInstallation
            )
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
    assert!(!crate::app_ownership::pending(&f.root));
    assert_eq!(hash(&f.root.join("foundation.sqlite3")).unwrap(), before);
    assert_eq!(f.main_count().await, 2);
}
#[tokio::test]
async fn relocation_refuses_nonempty_target_or_wrong_live_registry() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let target = Fixture::new().await;
    target.seed().await;
    assert_eq!(
        target
            .core()
            .restore(
                &f.bundle(),
                "not-fresh",
                RestorePolicy::FreshInstallationRelocation
            )
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
    let wrong = RegistrationRegistry::open(&f.root.join("wrong.sqlite3"))
        .await
        .unwrap();
    let core = OfflineCommunityBackup::new(&f.root, wrong).unwrap();
    assert_eq!(
        core.restore(&f.bundle(), "wrong", RestorePolicy::SameInstallation)
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
}
#[tokio::test]
async fn corrupted_component_and_rehashed_extra_trigger_are_rejected_before_pending_marker() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let mut manifest = read_json(&f.bundle().join("manifest.json")).unwrap();
    let path = f.bundle().join("application.sqlite3");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(f
        .core()
        .restore(&f.bundle(), "bad-hash", RestorePolicy::SameInstallation)
        .await
        .is_err());
    assert!(!crate::app_ownership::pending(&f.root));
    std::fs::write(&path, original).unwrap();
    let mut c = connection(&path, false, false).await.unwrap();
    sqlx::query("CREATE TRIGGER attacker BEFORE UPDATE ON community_host_binding BEGIN SELECT RAISE(ABORT,'attacker'); END").execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    manifest["appSha256"] = json!(hash(&path).unwrap());
    std::fs::write(f.bundle().join("manifest.json"), manifest.to_string()).unwrap();
    assert_eq!(
        f.core()
            .restore(&f.bundle(), "bad-schema", RestorePolicy::SameInstallation)
            .await
            .unwrap_err(),
        BackupError::InvalidBundle
    );
    assert!(!crate::app_ownership::pending(&f.root));
    assert_eq!(f.main_count().await, 1);
}
#[tokio::test]
async fn pending_deletion_or_noninactive_experimental_prevents_destructive_restore() {
    let f = Fixture::new().await;
    let (p, _) = f.seed().await;
    create(&f).await;
    let store = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    sqlx::query("INSERT INTO profile_deletion_jobs VALUES(?,?,?,1)")
        .bind(p["id"].as_str().unwrap())
        .bind("pending-delete")
        .bind("f".repeat(64))
        .execute(&store.pool)
        .await
        .unwrap();
    store.pool.close().await;
    drop(store);
    assert_eq!(
        f.core()
            .restore(&f.bundle(), "busy-delete", RestorePolicy::SameInstallation)
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
    let store = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    sqlx::query("DELETE FROM profile_deletion_jobs")
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO experimental_instances VALUES(?,'exp',1,?)")
        .bind(p["id"].as_str().unwrap())
        .bind(json!({"status":"running"}).to_string())
        .execute(&store.pool)
        .await
        .unwrap();
    store.pool.close().await;
    drop(store);
    assert_eq!(
        f.core()
            .restore(
                &f.bundle(),
                "busy-experimental",
                RestorePolicy::SameInstallation
            )
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
    assert!(!crate::app_ownership::pending(&f.root));
}
#[tokio::test]
async fn resume_all_four_interruption_windows_and_keep_receipts_after_input_removed() {
    for point in 1..=4 {
        let f = Fixture::new().await;
        f.seed().await;
        create(&f).await;
        let target = Fixture::new().await;
        let core = target.core();
        core.fault.store(point, Ordering::SeqCst);
        assert_eq!(
            core.restore(
                &f.bundle(),
                "interrupted",
                RestorePolicy::FreshInstallationRelocation
            )
            .await
            .unwrap_err(),
            BackupError::Io
        );
        assert!(crate::app_ownership::pending(&target.root));
        assert!(Store::open(&target.root.join("foundation.sqlite3"))
            .await
            .is_err());
        assert!(crate::backup::apply_pending_restore(&target.root)
            .await
            .is_err());
        let states = lifecycle::state(
            &target.registry,
            f.registry
                .maintenance_states(&f.registry.maintenance().await)
                .await
                .unwrap()[0]
                .0
                .as_str(),
        )
        .await
        .unwrap();
        assert_eq!(states.as_deref(), Some("reconcile"));
        let id = f
            .registry
            .maintenance_states(&f.registry.maintenance().await)
            .await
            .unwrap()[0]
            .0
            .clone();
        let scope = CommunityScope::new(
            &id,
            "https://example.invalid/installation/api/v1/",
            "https://example.invalid",
            "stable-community",
        )
        .unwrap();
        assert_eq!(
            target
                .registry
                .prepare(
                    &scope,
                    &RegistrationProfile::new("No override", "").unwrap(),
                    "fresh-start-0123456789"
                )
                .await
                .unwrap_err(),
            RegistryError::PolicyDenied
        );
        assert!(lifecycle::delete_profile(
            &target.registry,
            &target.keys,
            &CommunityTokenStore::with_backend(Arc::new(Secrets::default())),
            &id
        )
        .await
        .is_err());
        assert!(
            lifecycle::reconcile_profile(&target.registry, &target.keys, &id)
                .await
                .is_err()
        );
        let resumed = OfflineCommunityBackup::new(&target.root, target.registry.clone()).unwrap();
        if point == 3 {
            assert!(resumed.finish_cleanup("interrupted").await.is_err());
        }
        let result = if point == 4 {
            let main = target.root.join("foundation.sqlite3");
            let displaced = target.root.join("main-missing-fixture.sqlite3");
            std::fs::rename(&main, &displaced).unwrap();
            assert!(resumed
                .restore(
                    &f.bundle(),
                    "interrupted",
                    RestorePolicy::FreshInstallationRelocation
                )
                .await
                .is_err());
            assert!(crate::app_ownership::pending(&target.root));
            assert!(target.registry.restore_fence().exists());
            assert!(Store::open(&main).await.is_err());
            assert!(!main.exists());
            std::fs::rename(&displaced, &main).unwrap();
            std::fs::remove_dir_all(f.bundle()).unwrap();
            resumed.finish_cleanup("interrupted").await.unwrap()
        } else {
            resumed
                .restore(
                    &f.bundle(),
                    "interrupted",
                    RestorePolicy::FreshInstallationRelocation,
                )
                .await
                .unwrap()
        };
        assert_eq!(result["applied"], true);
        assert!(!crate::app_ownership::pending(&target.root));
        assert!(!target.registry.restore_fence().exists());
        assert_eq!(target.main_count().await, 1);
        if point != 4 {
            std::fs::remove_dir_all(f.bundle()).unwrap();
        }
        assert_eq!(resumed.receipt("interrupted").unwrap(), Some(result));
    }
}
#[tokio::test]
async fn different_restore_request_cannot_steal_pending_job_or_produce_success() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let target = Fixture::new().await;
    let core = target.core();
    core.fault.store(1, Ordering::SeqCst);
    assert!(core
        .restore(
            &f.bundle(),
            "first",
            RestorePolicy::FreshInstallationRelocation
        )
        .await
        .is_err());
    assert_eq!(
        target
            .core()
            .restore(
                &f.bundle(),
                "second",
                RestorePolicy::FreshInstallationRelocation
            )
            .await
            .unwrap_err(),
        BackupError::RestorePending
    );
    assert!(target.core().receipt("second").unwrap().is_none());
    target
        .core()
        .restore(
            &f.bundle(),
            "first",
            RestorePolicy::FreshInstallationRelocation,
        )
        .await
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abandoning_backup_waiter_does_not_drop_maintenance_ownership() {
    let f = Fixture::new().await;
    f.seed().await;
    let held = f.registry.maintenance().await;
    let core = f.core();
    let entered = core.owner_entered.clone();
    let dest = f.bundle();
    let worker = tokio::spawn(async move { core.create(&dest, "abandoned").await });
    tokio::time::timeout(Duration::from_secs(10), entered.notified())
        .await
        .unwrap();
    assert!(crate::app_ownership::acquire(&f.root.join("foundation.sqlite3"), false).is_err());
    assert!(!f.bundle().exists());
    worker.abort();
    let _ = worker.await;
    assert!(crate::app_ownership::acquire(&f.root.join("foundation.sqlite3"), false).is_err());
    drop(held);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if f.bundle().join("manifest.json").exists() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        read_json(&f.bundle().join("manifest.json")).unwrap()["requestId"],
        "abandoned"
    );
}
#[tokio::test]
async fn bound_but_empty_registry_is_not_assumed_to_have_no_lost_tombstones() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let mut c = f.reg_conn().await;
    sqlx::query("DELETE FROM community_preparations")
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query("DELETE FROM community_profile_lifecycle")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        f.core()
            .restore(
                &f.bundle(),
                "lost-registry",
                RestorePolicy::SameInstallation
            )
            .await
            .unwrap_err(),
        BackupError::PolicyDenied
    );
}
#[tokio::test]
async fn completed_request_rejects_different_policy_without_touching_current_state() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    f.core()
        .restore(&f.bundle(), "once", RestorePolicy::SameInstallation)
        .await
        .unwrap();
    assert_eq!(
        f.core()
            .restore(
                &f.bundle(),
                "once",
                RestorePolicy::FreshInstallationRelocation
            )
            .await
            .unwrap_err(),
        BackupError::IdempotencyConflict
    );
}
#[tokio::test]
async fn owner_process_probe() {
    if let Ok(path) = std::env::var("AIHOME_TEST_MAIN_OWNER_PATH") {
        assert!(crate::app_ownership::acquire(Path::new(&path), true).is_err());
    }
}
#[tokio::test]
async fn another_process_cannot_take_exclusive_main_owner() {
    let f = Fixture::new().await;
    let store = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("community_backup::tests::owner_process_probe")
        .arg("--nocapture")
        .env(
            "AIHOME_TEST_MAIN_OWNER_PATH",
            f.root.join("foundation.sqlite3"),
        )
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    store.pool.close().await;
    drop(store);
    assert!(crate::app_ownership::acquire(&f.root.join("foundation.sqlite3"), true).is_ok());
}

#[tokio::test]
async fn restart_restore_probe() {
    let Ok(root) = std::env::var("AIHOME_TEST_RESTORE_ROOT") else {
        return;
    };
    let bundle = std::env::var("AIHOME_TEST_RESTORE_BUNDLE").unwrap();
    let root = PathBuf::from(root);
    assert!(Store::open(&root.join("foundation.sqlite3")).await.is_err());
    let registry = RegistrationRegistry::open(&root.join("community.sqlite3"))
        .await
        .unwrap();
    let core = OfflineCommunityBackup::new(&root, registry).unwrap();
    let result = core
        .restore(
            Path::new(&bundle),
            "process-restart",
            RestorePolicy::FreshInstallationRelocation,
        )
        .await
        .unwrap();
    assert_eq!(result["applied"], true);
    assert!(!crate::app_ownership::pending(&root));
}
#[tokio::test]
async fn actual_process_restart_can_resume_after_registry_import_without_vault() {
    let source = Fixture::new().await;
    source.seed().await;
    create(&source).await;
    let temp = Temp::new();
    let root = temp.0.join("fresh-app");
    private_dir(&root).unwrap();
    let store = Store::open(&root.join("foundation.sqlite3")).await.unwrap();
    store.pool.close().await;
    drop(store);
    let registry = RegistrationRegistry::open(&root.join("community.sqlite3"))
        .await
        .unwrap();
    let core = OfflineCommunityBackup::new(&root, registry).unwrap();
    core.fault.store(1, Ordering::SeqCst);
    assert_eq!(
        core.restore(
            &source.bundle(),
            "process-restart",
            RestorePolicy::FreshInstallationRelocation
        )
        .await
        .unwrap_err(),
        BackupError::Io
    );
    drop(core);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "community_backup::tests::restart_restore_probe",
            "--nocapture",
        ])
        .env("AIHOME_TEST_RESTORE_ROOT", &root)
        .env("AIHOME_TEST_RESTORE_BUNDLE", source.bundle())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let store = Store::open(&root.join("foundation.sqlite3")).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM profiles")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        1
    );
    store.pool.close().await;
}
#[tokio::test]
async fn deleting_community_state_never_publishes_a_bundle() {
    let f = Fixture::new().await;
    f.seed().await;
    let mut c = f.reg_conn().await;
    sqlx::query("UPDATE community_profile_lifecycle SET state='deleting'")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        f.core().create(&f.bundle(), "blocked").await.unwrap_err(),
        BackupError::PolicyDenied
    );
    assert!(!f.bundle().exists());
}
#[cfg(unix)]
#[tokio::test]
async fn bundle_rejects_symlink_destination_and_component() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let link = f.temp.0.join("linked-bundle");
    symlink(f.bundle(), &link).unwrap();
    assert!(f
        .core()
        .restore(&link, "symlink", RestorePolicy::SameInstallation)
        .await
        .is_err());
    let file = f.bundle().join("application.sqlite3");
    let real = f.temp.0.join("real.sqlite3");
    std::fs::rename(&file, &real).unwrap();
    symlink(&real, &file).unwrap();
    assert!(f
        .core()
        .restore(&f.bundle(), "symlink-file", RestorePolicy::SameInstallation)
        .await
        .is_err());
    assert!(!crate::app_ownership::pending(&f.root));
}

#[tokio::test]
async fn malformed_terminal_receipt_does_not_clear_pending_or_fence() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let core = f.core();
    core.fault.store(4, Ordering::SeqCst);
    assert!(core
        .restore(&f.bundle(), "bad-receipt", RestorePolicy::SameInstallation)
        .await
        .is_err());
    let path = f
        .root
        .join("community-restore-results")
        .join(format!("{}.json", key("bad-receipt")));
    let original = std::fs::read(&path).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value.as_object_mut().unwrap().remove("applied");
    std::fs::write(&path, value.to_string()).unwrap();
    assert_eq!(
        f.core()
            .restore(&f.bundle(), "bad-receipt", RestorePolicy::SameInstallation)
            .await
            .unwrap_err(),
        BackupError::InvalidBundle
    );
    assert!(crate::app_ownership::pending(&f.root));
    assert!(f.registry.restore_fence().exists());
    assert!(f.core().receipt("bad-receipt").is_err());
    std::fs::write(&path, original).unwrap();
    f.core()
        .restore(&f.bundle(), "bad-receipt", RestorePolicy::SameInstallation)
        .await
        .unwrap();
}

#[path = "host_tests.rs"]
mod host_tests;
