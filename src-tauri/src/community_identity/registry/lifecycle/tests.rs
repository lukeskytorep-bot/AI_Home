use super::*;
use std::path::PathBuf;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;
#[derive(Default)]
struct Memory {
    data: Mutex<HashMap<String, Zeroizing<String>>>,
    creates: AtomicUsize,
    fail_remove: AtomicBool,
    pause_create: AtomicBool,
    pause_remove: AtomicBool,
    entered: Notify,
    release: (Mutex<bool>, std::sync::Condvar),
}
impl Memory {
    fn wait(&self) {
        self.entered.notify_one();
        let (lock, cv) = &self.release;
        let _held = cv
            .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(15), |r| !*r)
            .unwrap();
    }
    fn resume(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
}
impl CommunitySecretStore for Memory {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, s: &str) -> Result<(), IdentityError> {
        {
            let mut d = self.data.lock().unwrap();
            if d.contains_key(r) {
                return Err(IdentityError::AlreadyExists);
            }
            d.insert(r.into(), Zeroizing::new(s.into()));
        }
        self.creates.fetch_add(1, Ordering::SeqCst);
        if self.pause_create.load(Ordering::SeqCst) {
            self.wait();
        }
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        if self.fail_remove.load(Ordering::SeqCst) {
            return Err(IdentityError::VaultUnavailable);
        }
        if self.pause_remove.load(Ordering::SeqCst) {
            self.wait();
        }
        self.data.lock().unwrap().remove(r);
        Ok(())
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ai-home-life-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("registry.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scope(profile: &str) -> CommunityScope {
    CommunityScope::new(
        profile,
        "https://example.test/sub/api/v1/",
        "https://example.test",
        "test-community",
    )
    .unwrap()
}
fn profile() -> RegistrationProfile {
    RegistrationProfile::new("Agent Żółw", "public description").unwrap()
}
fn reg() -> String {
    format!("reg_{}", "1".repeat(32))
}
fn actor() -> String {
    format!("agt_{}", "2".repeat(32))
}
fn token() -> String {
    format!("eqai_{}", super::super::super::crypto::b64(&[7; 32]))
}
async fn ready(r: &RegistrationRegistry, keys: &CommunityKeyStore, s: &CommunityScope) {
    r.prepare(
        s,
        &profile(),
        &format!("original-start-{}-0123456789", s.profile_id),
    )
    .await
    .unwrap();
    r.provision(keys, s, 1).await.unwrap();
}
async fn seed_token(
    r: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    tokens: &CommunityTokenStore,
    s: &CommunityScope,
) -> String {
    let i = r.load_intent(keys, s).await.unwrap();
    let nonce = super::super::super::crypto::b64(&[7; 32]);
    let expiry =
        (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let statement = super::super::super::registration_statement(
        s,
        &i.identity,
        &i.profile,
        &reg(),
        &nonce,
        &expiry,
    );
    let context = json!({"ok":true,"request_id":"fixture","registration":{"registration_id":reg(),"state":"pending","nonce":nonce,"expires_at":expiry,"community_origin":s.origin,"community_instance":s.instance,"profile_format":"COMMUNITY-PROFILE/1","profile_sha256":i.profile.sha256(),"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}});
    sqlx::query("INSERT INTO community_registration_starts VALUES(?,?,1,?,NULL)")
        .bind(s.binding())
        .bind("0".repeat(64))
        .bind(context.to_string())
        .execute(&r.pool)
        .await
        .unwrap();
    let reference = CommunityTokenStore::reserve();
    tokens
        .save(
            s,
            &reference,
            &reg(),
            &actor(),
            1,
            super::super::super::tokens::SecretToken::new(token()).unwrap(),
        )
        .await
        .unwrap();
    let body = json!({"registrationId":reg(),"completeBody":null,"state":"ready","agentId":actor(),"keyId":null,"generation":1,"tokenRef":reference});
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,?)")
        .bind(s.binding())
        .bind(body.to_string())
        .execute(&r.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_dispatches VALUES(?,?,1,'complete','{}',?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(s.binding())
        .bind(&reference)
        .execute(&r.pool)
        .await
        .unwrap();
    reference
}
struct Setup {
    temp: Temp,
    r: RegistrationRegistry,
    keys: CommunityKeyStore,
    k: Arc<Memory>,
    tokens: CommunityTokenStore,
    t: Arc<Memory>,
    s: CommunityScope,
}
async fn setup() -> Setup {
    let temp = Temp::new();
    let r = RegistrationRegistry::open(&temp.db()).await.unwrap();
    let k = Arc::new(Memory::default());
    let t = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(k.clone());
    let tokens = CommunityTokenStore::with_backend(t.clone());
    let s = scope("profile-a");
    ready(&r, &keys, &s).await;
    seed_token(&r, &keys, &tokens, &s).await;
    Setup {
        temp,
        r,
        keys,
        k,
        tokens,
        t,
        s,
    }
}
fn finish(tokens: CommunityTokenStore) -> super::super::finish::RegistrationFinish {
    super::super::finish::RegistrationFinish::new(Duration::from_secs(5), tokens).unwrap()
}

#[tokio::test]
async fn delete_removes_keys_active_and_orphan_candidates_and_metadata() {
    let s = setup().await;
    let orphan = CommunityTokenStore::reserve();
    s.tokens
        .save(
            &s.s,
            &orphan,
            &reg(),
            &actor(),
            2,
            super::super::super::tokens::SecretToken::new(token()).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_dispatches VALUES(?,?,1,'reissue','{}',?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(s.s.binding())
        .bind(orphan)
        .execute(&s.r.pool)
        .await
        .unwrap();
    let d = delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    assert_eq!(d.key_references_processed, 1);
    assert_eq!(d.token_references_processed, 2);
    assert!(s.k.data.lock().unwrap().is_empty());
    assert!(s.t.data.lock().unwrap().is_empty());
    assert_eq!(s.r.get(&s.s).await, Err(RegistryError::NotFound));
    assert_eq!(
        state(&s.r, "profile-a").await.unwrap().as_deref(),
        Some("deleted")
    );
}
#[tokio::test]
async fn deletion_is_idempotent_and_tombstone_blocks_recreation_after_reopen() {
    let s = setup().await;
    delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    let d = delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    assert_eq!(d.key_references_processed, 0);
    s.r.clone().close().await;
    let r = RegistrationRegistry::open(&s.temp.db()).await.unwrap();
    assert_eq!(
        r.prepare(&s.s, &profile(), "new-start-0123456789").await,
        Err(RegistryError::PolicyDenied)
    );
}
#[tokio::test]
async fn deleting_one_profile_preserves_other_profile_keys_tokens_and_access() {
    let s = setup().await;
    let other = scope("profile-b");
    ready(&s.r, &s.keys, &other).await;
    seed_token(&s.r, &s.keys, &s.tokens, &other).await;
    delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    assert!(s.r.load_intent(&s.keys, &other).await.is_ok());
    assert!(finish(s.tokens.clone())
        .current(&s.r, &s.keys, &other)
        .await
        .is_ok());
    assert_eq!(s.k.data.lock().unwrap().len(), 1);
    assert_eq!(s.t.data.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn failed_secret_removal_keeps_gate_closed_until_idempotent_resume() {
    let s = setup().await;
    s.t.fail_remove.store(true, Ordering::SeqCst);
    assert_eq!(
        delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
            .await
            .unwrap_err(),
        LifecycleError::Identity(IdentityError::VaultUnavailable)
    );
    assert_eq!(
        state(&s.r, "profile-a").await.unwrap().as_deref(),
        Some("deleting")
    );
    assert!(s.r.load_intent(&s.keys, &s.s).await.is_err());
    assert_eq!(
        snapshot(&s.r, &s.temp.0.join("bad.sqlite"))
            .await
            .unwrap_err(),
        LifecycleError::Registry(RegistryError::PolicyDenied)
    );
    s.t.fail_remove.store(false, Ordering::SeqCst);
    delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    assert!(s.t.data.lock().unwrap().is_empty());
}
#[tokio::test]
async fn cancelled_provision_writer_is_drained_before_secret_deletion() {
    let temp = Temp::new();
    let r = RegistrationRegistry::open(&temp.db()).await.unwrap();
    let k = Arc::new(Memory::default());
    k.pause_create.store(true, Ordering::SeqCst);
    let keys = CommunityKeyStore::with_backend(k.clone());
    let tokens = CommunityTokenStore::with_backend(Arc::new(Memory::default()));
    let s = scope("profile-a");
    r.prepare(&s, &profile(), "start-01234567890123")
        .await
        .unwrap();
    let worker = {
        let r = r.clone();
        let keys = keys.clone();
        let s = s.clone();
        tokio::spawn(async move { r.provision(&keys, &s, 1).await })
    };
    tokio::time::timeout(Duration::from_secs(5), k.entered.notified())
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    let deletion = {
        let r = r.clone();
        let keys = keys.clone();
        tokio::spawn(async move { delete_profile(&r, &keys, &tokens, "profile-a").await })
    };
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!deletion.is_finished());
    k.resume();
    deletion.await.unwrap().unwrap();
    assert!(k.data.lock().unwrap().is_empty());
    assert_eq!(
        state(&r, "profile-a").await.unwrap().as_deref(),
        Some("deleted")
    );
}
#[tokio::test]
async fn a_returned_credential_holds_a_lease_until_it_is_dropped() {
    let s = setup().await;
    let credential = finish(s.tokens.clone())
        .credential(&s.r, &s.keys, &s.s)
        .await
        .unwrap();
    let deletion = {
        let r = s.r.clone();
        let k = s.keys.clone();
        let t = s.tokens.clone();
        tokio::spawn(async move { delete_profile(&r, &k, &t, "profile-a").await })
    };
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!deletion.is_finished());
    drop(credential);
    deletion.await.unwrap().unwrap();
    assert!(s.t.data.lock().unwrap().is_empty());
}
#[tokio::test]
async fn cancelling_maintenance_caller_does_not_release_a_blocking_cleanup_owner() {
    let s = setup().await;
    s.t.pause_remove.store(true, Ordering::SeqCst);
    let task = {
        let r = s.r.clone();
        let k = s.keys.clone();
        let t = s.tokens.clone();
        tokio::spawn(async move { delete_profile(&r, &k, &t, "profile-a").await })
    };
    tokio::time::timeout(Duration::from_secs(5), s.t.entered.notified())
        .await
        .unwrap();
    task.abort();
    let _ = task.await;
    let other = {
        let r = s.r.clone();
        tokio::spawn(async move {
            r.prepare(&scope("profile-b"), &profile(), "other-start-0123456789")
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!other.is_finished());
    s.t.resume();
    other.await.unwrap().unwrap();
    assert_eq!(
        state(&s.r, "profile-a").await.unwrap().as_deref(),
        Some("deleted")
    );
}
#[tokio::test]
async fn snapshot_preserves_live_readiness_but_disables_copied_credentials() {
    let s = setup().await;
    let f = finish(s.tokens.clone());
    let before = f.current(&s.r, &s.keys, &s.s).await.unwrap();
    let path = s.temp.0.join("snapshot.sqlite");
    let receipt = snapshot(&s.r, &path).await.unwrap();
    assert!(!receipt.secrets_included);
    assert!(receipt.requires_reconciliation);
    assert_eq!(f.current(&s.r, &s.keys, &s.s).await.unwrap(), before);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(&token()));
    let mut c = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .read_only(true)
            .immutable(true),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM community_profile_lifecycle")
            .fetch_one(&mut c)
            .await
            .unwrap(),
        "reconcile"
    );
    let body: Value = serde_json::from_str(
        &sqlx::query_scalar::<_, String>("SELECT body FROM community_registration_finishes")
            .fetch_one(&mut c)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["state"], "uncertain");
    assert!(body["tokenRef"].is_null());
}
#[tokio::test]
async fn restore_requires_key_reconciliation_and_never_trusts_existing_token_candidates() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    let snap = snapshot(&s.r, &path).await.unwrap();
    let target = Temp::new();
    let r = RegistrationRegistry::open(&target.db()).await.unwrap();
    assert_eq!(
        restore_metadata(&r, &path, &snap.sha256)
            .await
            .unwrap()
            .profiles,
        1
    );
    assert_eq!(
        r.load_intent(&s.keys, &s.s).await.unwrap_err(),
        RegistryError::PolicyDenied
    );
    let result = reconcile_profile(&r, &s.keys, "profile-a").await.unwrap();
    assert!(result.active);
    assert_eq!(result.token_recovery_required, 1);
    assert!(matches!(
        finish(s.tokens.clone()).credential(&r, &s.keys, &s.s).await,
        Err(super::super::finish::FinishError::RecoveryRequired)
    ));
    assert_eq!(s.k.creates.load(Ordering::SeqCst), 1);
    assert_eq!(s.t.creates.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn missing_keys_after_restore_do_not_generate_replacement_identity() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    let snap = snapshot(&s.r, &path).await.unwrap();
    let target = Temp::new();
    let r = RegistrationRegistry::open(&target.db()).await.unwrap();
    restore_metadata(&r, &path, &snap.sha256).await.unwrap();
    let missing = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(missing.clone());
    let result = reconcile_profile(&r, &keys, "profile-a").await.unwrap();
    assert!(!result.active);
    assert_eq!(result.missing_keys, 1);
    assert!(r.provision(&keys, &s.s, 3).await.is_err());
    assert_eq!(missing.creates.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn old_snapshot_cannot_resurrect_a_locally_deleted_profile() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    let snap = snapshot(&s.r, &path).await.unwrap();
    delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
        .await
        .unwrap();
    let result = restore_metadata(&s.r, &path, &snap.sha256).await.unwrap();
    assert_eq!(result.profiles, 0);
    assert_eq!(result.skipped_live_or_deleted, 1);
    assert_eq!(s.r.get(&s.s).await, Err(RegistryError::NotFound));
    assert!(s.k.data.lock().unwrap().is_empty());
}
#[tokio::test]
async fn restore_skips_live_profiles_without_rolling_back_readiness() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    let snap = snapshot(&s.r, &path).await.unwrap();
    let f = finish(s.tokens.clone());
    let before = f.current(&s.r, &s.keys, &s.s).await.unwrap();
    let result = restore_metadata(&s.r, &path, &snap.sha256).await.unwrap();
    assert_eq!(result.skipped_live_or_deleted, 1);
    assert_eq!(f.current(&s.r, &s.keys, &s.s).await.unwrap(), before);
}
#[tokio::test]
async fn wrong_snapshot_hash_is_read_only_and_rejected() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    snapshot(&s.r, &path).await.unwrap();
    let target = Temp::new();
    let r = RegistrationRegistry::open(&target.db()).await.unwrap();
    assert_eq!(
        r.load_intent(&s.keys, &s.s).await.unwrap_err(),
        RegistryError::NotFound
    );
    assert!(r.prepare(&s.s, &profile(), "short").await.is_err());
    assert!(state(&r, "profile-a").await.unwrap().is_none());
    assert_eq!(
        restore_metadata(&r, &path, &"0".repeat(64))
            .await
            .unwrap_err(),
        LifecycleError::InvalidSnapshot
    );
    assert!(state(&r, "profile-a").await.unwrap().is_none());
}
#[tokio::test]
async fn an_unsanitized_snapshot_cannot_import_ready_credentials() {
    let s = setup().await;
    let path = s.temp.0.join("snapshot.sqlite");
    snapshot(&s.r, &path).await.unwrap();
    let mut c = sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("UPDATE community_profile_lifecycle SET state='active'")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let sha = super::super::super::crypto::digest(&std::fs::read(&path).unwrap());
    let target = Temp::new();
    let r = RegistrationRegistry::open(&target.db()).await.unwrap();
    assert_eq!(
        restore_metadata(&r, &path, &sha).await.unwrap_err(),
        LifecycleError::InvalidSnapshot
    );
    assert!(state(&r, "profile-a").await.unwrap().is_none());
    let mut c = sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("UPDATE community_profile_lifecycle SET state='reconcile'")
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query("CREATE VIEW unexpected_snapshot_query AS SELECT 1")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let sha = super::super::super::crypto::digest(&std::fs::read(&path).unwrap());
    assert_eq!(
        restore_metadata(&r, &path, &sha).await.unwrap_err(),
        LifecycleError::InvalidSnapshot
    );
    assert!(state(&r, "profile-a").await.unwrap().is_none());
}
#[tokio::test]
async fn snapshot_does_not_overwrite_an_existing_file() {
    let s = setup().await;
    let path = s.temp.0.join("owned.txt");
    std::fs::write(&path, b"preserve").unwrap();
    assert_eq!(snapshot(&s.r, &path).await.unwrap_err(), LifecycleError::Io);
    assert_eq!(std::fs::read(path).unwrap(), b"preserve");
    let escaped = s.temp.0.join(if cfg!(unix) {
        "quote'and\\slash.sqlite"
    } else {
        "quote'and.sqlite"
    });
    snapshot(&s.r, &escaped).await.unwrap();
    assert!(escaped.is_file());
}
#[tokio::test]
async fn restored_reserved_metadata_cannot_silently_provision_new_keys() {
    let temp = Temp::new();
    let r = RegistrationRegistry::open(&temp.db()).await.unwrap();
    let scope = scope("profile-a");
    r.prepare(&scope, &profile(), "start-01234567890123")
        .await
        .unwrap();
    let path = temp.0.join("snapshot.sqlite");
    let snap = snapshot(&r, &path).await.unwrap();
    let target = Temp::new();
    let restored = RegistrationRegistry::open(&target.db()).await.unwrap();
    restore_metadata(&restored, &path, &snap.sha256)
        .await
        .unwrap();
    let backend = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(backend.clone());
    let result = reconcile_profile(&restored, &keys, "profile-a")
        .await
        .unwrap();
    assert!(!result.active);
    assert_eq!(result.pending_provisioning, 1);
    assert_eq!(
        reconcile_keys(&restored, &keys, &scope, 1)
            .await
            .unwrap_err(),
        LifecycleError::Identity(IdentityError::NotFound)
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn controlled_reconciliation_can_adopt_only_existing_reserved_key_material() {
    let temp = Temp::new();
    let r = RegistrationRegistry::open(&temp.db()).await.unwrap();
    let scope = scope("profile-a");
    r.prepare(&scope, &profile(), "start-01234567890123")
        .await
        .unwrap();
    let path = temp.0.join("snapshot.sqlite");
    let snap = snapshot(&r, &path).await.unwrap();
    let backend = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(backend.clone());
    let original = r.provision(&keys, &scope, 1).await.unwrap();
    let target = Temp::new();
    let restored = RegistrationRegistry::open(&target.db()).await.unwrap();
    restore_metadata(&restored, &path, &snap.sha256)
        .await
        .unwrap();
    let adopted = reconcile_keys(&restored, &keys, &scope, 1).await.unwrap();
    assert_eq!(adopted.public_identity(), original.public_identity());
    assert!(
        reconcile_profile(&restored, &keys, "profile-a")
            .await
            .unwrap()
            .active
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn cleanup_refuses_a_foreign_profile_token_reference() {
    let s = setup().await;
    let other = scope("profile-b");
    ready(&s.r, &s.keys, &other).await;
    let other_ref = seed_token(&s.r, &s.keys, &s.tokens, &other).await;
    sqlx::query("UPDATE community_registration_finishes SET body=json_set(body,'$.tokenRef',?) WHERE scope_binding=?").bind(&other_ref).bind(s.s.binding()).execute(&s.r.pool).await.unwrap();
    assert_eq!(
        delete_profile(&s.r, &s.keys, &s.tokens, "profile-a")
            .await
            .unwrap_err(),
        LifecycleError::Identity(IdentityError::ScopeMismatch)
    );
    assert!(s.t.data.lock().unwrap().contains_key(&other_ref));
    assert_eq!(
        state(&s.r, "profile-a").await.unwrap().as_deref(),
        Some("deleting")
    );
}
#[tokio::test]
async fn schema_v3_migration_preserves_identity_and_live_token_metadata() {
    let s = setup().await;
    let before = s.r.get(&s.s).await.unwrap();
    sqlx::query("DROP TABLE community_profile_lifecycle")
        .execute(&s.r.pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=3")
        .execute(&s.r.pool)
        .await
        .unwrap();
    s.r.clone().close().await;
    let r = RegistrationRegistry::open(&s.temp.db()).await.unwrap();
    assert_eq!(r.get(&s.s).await.unwrap(), before);
    assert_eq!(
        state(&r, "profile-a").await.unwrap().as_deref(),
        Some("active")
    );
    assert!(finish(s.tokens.clone())
        .current(&r, &s.keys, &s.s)
        .await
        .is_ok());
}
#[cfg(unix)]
#[tokio::test]
async fn snapshot_file_is_private_and_database_symlink_is_refused() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let s = setup().await;
    assert_eq!(
        std::fs::metadata(s.temp.db()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let path = s.temp.0.join("snapshot.sqlite");
    snapshot(&s.r, &path).await.unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let alias = s.temp.0.join("alias.sqlite");
    symlink(s.temp.db(), &alias).unwrap();
    assert!(matches!(
        RegistrationRegistry::open(&alias).await,
        Err(RegistryError::Corrupt)
    ));
}
#[test]
fn one_owner_process_per_registry_path_and_release_after_close() {
    const ENV: &str = "AI_HOME_LIFECYCLE_OWNER_PROBE";
    if let Ok(path) = std::env::var(ENV) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(RegistrationRegistry::open(Path::new(&path)));
        if std::env::var("AI_HOME_EXPECT_BUSY").unwrap() == "yes" {
            assert!(matches!(result, Err(RegistryError::Busy)));
        } else {
            assert!(result.is_ok());
        }
        return;
    }
    let temp = Temp::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let r = rt.block_on(RegistrationRegistry::open(&temp.db())).unwrap();
    let run = |busy: &str| {
        let output=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","community_identity::registry::lifecycle::tests::one_owner_process_per_registry_path_and_release_after_close","--nocapture"]).env(ENV,temp.db()).env("AI_HOME_EXPECT_BUSY",busy).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    };
    run("yes");
    rt.block_on(r.close());
    run("no");
}
