use super::*;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Default)]
struct Memory {
    values: Mutex<HashMap<String, Zeroizing<String>>>,
    creates: AtomicUsize,
    fail_after_create: AtomicBool,
    fail_read: AtomicBool,
    pause_after_create: AtomicBool,
    created: tokio::sync::Notify,
    release: (Mutex<bool>, std::sync::Condvar),
}
impl CommunitySecretStore for Memory {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        if self.fail_read.swap(false, Ordering::SeqCst) {
            return Err(IdentityError::VaultUnavailable);
        }
        Ok(self.values.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, value: &str) -> Result<(), IdentityError> {
        {
            let mut map = self.values.lock().unwrap();
            if map.contains_key(r) {
                return Err(IdentityError::AlreadyExists);
            }
            map.insert(r.into(), Zeroizing::new(value.into()));
        }
        self.creates.fetch_add(1, Ordering::SeqCst);
        if self.fail_after_create.load(Ordering::SeqCst) {
            self.fail_read.store(true, Ordering::SeqCst);
        }
        self.created.notify_one();
        if self.pause_after_create.load(Ordering::SeqCst) {
            let (lock, cv) = &self.release;
            let guard = lock.lock().unwrap();
            let _wait = cv
                .wait_timeout_while(guard, Duration::from_secs(15), |released| !*released)
                .unwrap();
        }
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.values.lock().unwrap().remove(r);
        Ok(())
    }
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ai-home-community-registry-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self) -> std::path::PathBuf {
        self.0.join("registry.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scope(name: &str) -> CommunityScope {
    CommunityScope::new(
        name,
        "https://example.test/api/v1/",
        "https://example.test",
        "community-test",
    )
    .unwrap()
}
fn profile() -> RegistrationProfile {
    RegistrationProfile::new("Agent Żółw", "Public description").unwrap()
}
const START: &str = "stable-client-start-0123456789";
async fn setup() -> (Temp, RegistrationRegistry, CommunityKeyStore, Arc<Memory>) {
    let temp = Temp::new();
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let backend = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(backend.clone());
    (temp, registry, keys, backend)
}

#[tokio::test]
async fn reservation_is_durable_and_exact_prepare_is_idempotent() {
    let (temp, registry, _, backend) = setup().await;
    let a = registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert_eq!(a.state(), PreparationState::Reserved);
    assert_eq!(a.revision(), 1);
    registry.close().await;
    let reopened = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(
        reopened
            .prepare(&scope("a"), &profile(), START)
            .await
            .unwrap(),
        a
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn changed_profile_or_start_id_conflicts_without_mutation() {
    let (_temp, registry, _, _) = setup().await;
    let a = registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert_eq!(
        registry
            .prepare(
                &scope("a"),
                &RegistrationProfile::new("Changed", "").unwrap(),
                START
            )
            .await,
        Err(RegistryError::Conflict)
    );
    assert_eq!(
        registry
            .prepare(&scope("a"), &profile(), "another-stable-start-0123456789")
            .await,
        Err(RegistryError::Conflict)
    );
    assert_eq!(registry.get(&scope("a")).await.unwrap(), a);
}

#[tokio::test]
async fn start_id_cannot_move_to_another_profile_or_endpoint() {
    let (_temp, registry, _, _) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert_eq!(
        registry.prepare(&scope("b"), &profile(), START).await,
        Err(RegistryError::Conflict)
    );
    let other = CommunityScope::new(
        "a",
        "https://other.test/api/v1/",
        "https://example.test",
        "community-test",
    )
    .unwrap();
    assert_eq!(
        registry.prepare(&other, &profile(), START).await,
        Err(RegistryError::Conflict)
    );
    assert_eq!(registry.get(&other).await, Err(RegistryError::NotFound));
}

#[tokio::test]
async fn invalid_start_ids_create_no_rows_or_keys() {
    let (_temp, registry, _, backend) = setup().await;
    for id in ["short", "012345678901234\n56789", &"x".repeat(129)] {
        assert_eq!(
            registry.prepare(&scope("a"), &profile(), id).await,
            Err(RegistryError::Identity(IdentityError::Invalid))
        );
    }
    assert_eq!(
        registry.get(&scope("a")).await,
        Err(RegistryError::NotFound)
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn keys_and_start_bytes_survive_close_and_reopen_without_regeneration() {
    let (temp, registry, keys, backend) = setup().await;
    let a = registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    let ready = registry
        .provision(&keys, &scope("a"), a.revision())
        .await
        .unwrap();
    assert_eq!(ready.state(), PreparationState::Ready);
    assert_eq!(ready.revision(), 3);
    let before = registry
        .load_intent(&keys, &scope("a"))
        .await
        .unwrap()
        .start_request("secret-invite")
        .unwrap();
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(
        registry
            .recover(&keys, &scope("a"), ready.revision())
            .await
            .unwrap(),
        ready
    );
    let after = registry
        .load_intent(&keys, &scope("a"))
        .await
        .unwrap()
        .start_request("secret-invite")
        .unwrap();
    assert_eq!(before.as_bytes(), after.as_bytes());
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn competing_database_handles_create_one_keyset_only() {
    let (temp, registry, keys, backend) = setup().await;
    let other = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let a = registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    let s = scope("a");
    let (first, second) = tokio::join!(
        registry.provision(&keys, &s, a.revision()),
        other.provision(&keys, &s, a.revision())
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(matches!(
        first.as_ref().err().or(second.as_ref().err()),
        Some(RegistryError::Conflict)
    ));
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stale_revision_is_rejected_before_touching_vault() {
    let (_temp, registry, keys, backend) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert_eq!(
        registry.provision(&keys, &scope("a"), 2).await,
        Err(RegistryError::Conflict)
    );
    assert_eq!(
        registry.recover(&keys, &scope("a"), 2).await,
        Err(RegistryError::Conflict)
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failure_after_secret_write_can_only_load_existing_keys() {
    let (temp, registry, keys, backend) = setup().await;
    backend.fail_after_create.store(true, Ordering::SeqCst);
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert_eq!(
        registry.provision(&keys, &scope("a"), 1).await,
        Err(RegistryError::Identity(IdentityError::VaultUnavailable))
    );
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(
        registry.get(&scope("a")).await.unwrap().state(),
        PreparationState::Provisioning
    );
    assert_eq!(
        registry.provision(&keys, &scope("a"), 2).await,
        Err(RegistryError::ProvisioningUncertain)
    );
    assert_eq!(
        registry
            .recover(&keys, &scope("a"), 2)
            .await
            .unwrap()
            .state(),
        PreparationState::Ready
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn interrupted_claim_with_no_keys_stays_uncertain_never_reclaimed() {
    let (temp, registry, keys, backend) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    // Crash after durable ownership, before any keyring call.
    sqlx::query(
        "UPDATE community_preparations SET state='provisioning',owner='dead-process',revision=2",
    )
    .execute(&registry.pool)
    .await
    .unwrap();
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(
        registry.recover(&keys, &scope("a"), 2).await,
        Err(RegistryError::Identity(IdentityError::NotFound))
    );
    assert_eq!(
        registry.provision(&keys, &scope("a"), 2).await,
        Err(RegistryError::ProvisioningUncertain)
    );
    assert_eq!(registry.get(&scope("a")).await.unwrap().revision(), 2);
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_writer_cannot_lose_ownership_to_second_provisioner() {
    let (temp, registry, keys, backend) = setup().await;
    backend.pause_after_create.store(true, Ordering::SeqCst);
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    let worker = {
        let registry = registry.clone();
        let keys = keys.clone();
        tokio::spawn(async move { registry.provision(&keys, &scope("a"), 1).await })
    };
    tokio::time::timeout(Duration::from_secs(10), backend.created.notified())
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    let other = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(
        other.provision(&keys, &scope("a"), 2).await,
        Err(RegistryError::ProvisioningUncertain)
    );
    let recovered = other.recover(&keys, &scope("a"), 2).await.unwrap();
    {
        *backend.release.0.lock().unwrap() = true;
        backend.release.1.notify_all();
    }
    assert_eq!(recovered.state(), PreparationState::Ready);
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(registry.get(&scope("a")).await.unwrap(), recovered);
}

#[tokio::test]
async fn ready_keys_missing_after_restore_are_not_regenerated() {
    let (_temp, registry, keys, backend) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    let a = registry.provision(&keys, &scope("a"), 1).await.unwrap();
    backend.values.lock().unwrap().clear();
    assert!(matches!(
        registry.load_intent(&keys, &scope("a")).await,
        Err(RegistryError::Identity(IdentityError::NotFound))
    ));
    assert_eq!(
        registry.recover(&keys, &scope("a"), a.revision()).await,
        Err(RegistryError::Identity(IdentityError::NotFound))
    );
    assert_eq!(registry.get(&scope("a")).await.unwrap(), a);
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn changed_public_expectation_cannot_sign_as_restored_identity() {
    let (_temp, registry, keys, _) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    let a = registry.provision(&keys, &scope("a"), 1).await.unwrap();
    let unrelated = CommunityKeyStore::reserve_reference().unwrap();
    let other = keys.provision(&scope("a"), &unrelated).await.unwrap();
    sqlx::query("UPDATE community_preparations SET identity_public_key=?,operational_public_key=?")
        .bind(other.identity_public_key())
        .bind(other.operational_public_key())
        .execute(&registry.pool)
        .await
        .unwrap();
    assert!(matches!(
        registry.load_intent(&keys, &scope("a")).await,
        Err(RegistryError::Identity(IdentityError::ScopeMismatch))
    ));
    assert_eq!(
        registry.recover(&keys, &scope("a"), a.revision()).await,
        Err(RegistryError::Identity(IdentityError::ScopeMismatch))
    );
}

#[tokio::test]
async fn registry_contains_no_invite_or_private_key_bytes() {
    let (temp, registry, keys, _) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    registry.provision(&keys, &scope("a"), 1).await.unwrap();
    let _request = registry
        .load_intent(&keys, &scope("a"))
        .await
        .unwrap()
        .start_request("private-invite-do-not-persist")
        .unwrap();
    for entry in std::fs::read_dir(&temp.0).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for forbidden in [
            "private-invite-do-not-persist",
            "identity_pkcs8=",
            "operational_pkcs8=",
            "PRIVATE KEY",
            "invite_token",
        ] {
            assert!(!text.contains(forbidden));
        }
    }
}

#[tokio::test]
async fn unrelated_or_future_database_is_refused() {
    let temp = Temp::new();
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(temp.path())
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE unrelated(value TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO unrelated VALUES('preserved')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        RegistrationRegistry::open(&temp.path()).await,
        Err(RegistryError::Corrupt)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM unrelated")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "preserved"
    );
    let temp2 = Temp::new();
    let registry = RegistrationRegistry::open(&temp2.path()).await.unwrap();
    sqlx::query("PRAGMA user_version=2")
        .execute(&registry.pool)
        .await
        .unwrap();
    registry.close().await;
    assert!(matches!(
        RegistrationRegistry::open(&temp2.path()).await,
        Err(RegistryError::Corrupt)
    ));
}

#[tokio::test]
async fn corrupt_scope_or_public_key_is_never_loaded() {
    let (_temp, registry, keys, _) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    registry.provision(&keys, &scope("a"), 1).await.unwrap();
    sqlx::query("UPDATE community_preparations SET identity_public_key='invalid'")
        .execute(&registry.pool)
        .await
        .unwrap();
    assert_eq!(registry.get(&scope("a")).await, Err(RegistryError::Corrupt));
    sqlx::query("UPDATE community_preparations SET origin='https://other.test'")
        .execute(&registry.pool)
        .await
        .unwrap();
    assert_eq!(registry.get(&scope("a")).await, Err(RegistryError::Corrupt));
}

#[tokio::test]
async fn concurrent_identical_reservations_share_the_same_reference() {
    let (temp, registry, _, _) = setup().await;
    let other = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let s = scope("a");
    let p = profile();
    let (a, b) = tokio::join!(
        registry.prepare(&s, &p, START),
        other.prepare(&s, &p, START)
    );
    assert_eq!(a.unwrap(), b.unwrap());
}

#[tokio::test]
async fn independent_profile_keys_and_intents_never_cross_scopes() {
    let (_temp, registry, keys, _) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    registry
        .prepare(
            &scope("b"),
            &profile(),
            "second-stable-client-start-0123456789",
        )
        .await
        .unwrap();
    let a = registry.provision(&keys, &scope("a"), 1).await.unwrap();
    let b = registry.provision(&keys, &scope("b"), 1).await.unwrap();
    assert_ne!(a.key_reference(), b.key_reference());
    assert_ne!(a.public_identity(), b.public_identity());
    assert_eq!(
        keys.load(&scope("a"), b.key_reference()).await,
        Err(IdentityError::ScopeMismatch)
    );
    assert!(registry.load_intent(&keys, &scope("a")).await.is_ok());
    assert!(registry.load_intent(&keys, &scope("b")).await.is_ok());
}

#[tokio::test]
async fn reserved_metadata_does_not_imply_existing_keys_or_registration() {
    let (_temp, registry, keys, backend) = setup().await;
    registry
        .prepare(&scope("a"), &profile(), START)
        .await
        .unwrap();
    assert!(matches!(
        registry.load_intent(&keys, &scope("a")).await,
        Err(RegistryError::ProvisioningUncertain)
    ));
    assert_eq!(
        registry.recover(&keys, &scope("a"), 1).await,
        Err(RegistryError::ProvisioningUncertain)
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 0);
}
