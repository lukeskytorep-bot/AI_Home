use super::*;
async fn progress(h: &CommunityHost, r: Value, caps: Vec<Cap>) -> Value {
    h.dispatch_local(
        r,
        crate::community_host::local::LocalAccess::new(scope(), caps),
    )
    .await
    .unwrap()
}
// Recreate both database owners and the native secret-store wrappers. Only the
// test vault backend and simulated remote server survive, as they do a restart.
async fn restart(s: &mut Setup, h: CommunityHost) -> CommunityHost {
    h.close_application().await.unwrap();
    drop(h);
    s.registry.clone().close().await;
    s.registry = RegistrationRegistry::open(&s.temp.0.join("community.sqlite3"))
        .await
        .unwrap();
    s.keys = CommunityKeyStore::with_backend(s.key_backend.clone());
    s.tokens = CommunityTokenStore::with_backend(s.token_backend.clone());
    let h = CommunityHost::new(
        &s.temp.0,
        CommunityLifecycle::new(s.registry.clone(), s.keys.clone(), s.tokens.clone()),
        Vault::with_backend(Arc::new(NoModels)),
        None,
    )
    .unwrap();
    h.open_application().await.unwrap();
    h
}

#[tokio::test]
async fn host_progress_is_fresh_without_receipts_http_or_token_trust() {
    let (s, h) = fixture().await;
    let r = request("progress", json!({}));
    let absent = progress(&h, r.clone(), vec![Cap::Progress]).await;
    assert_eq!(
        absent["result"],
        json!({"finishRevision":0,"finishState":"absent","onlineStatusKnown":false})
    );
    assert_eq!(receipts(&s).await, 0);
    assert!(s.mock.requests.lock().unwrap().is_empty());
    let completed = send(
        &s,
        &h,
        request("complete", json!({"registrationRevision":3})),
        Cap::Complete,
    )
    .await;
    assert_eq!(completed["ok"], true, "{completed}");
    let before = receipts(&s).await;
    // Reusing the read request ID must observe current state, not the first read.
    let current = progress(&h, r.clone(), vec![Cap::Progress]).await;
    assert_eq!(
        current["result"],
        json!({"finishRevision":completed["result"]["finishRevision"],"finishState":"ready_metadata","onlineStatusKnown":false})
    );
    s.key_backend.fail_read.store(true, Ordering::SeqCst);
    s.token_backend.fail_read.store(true, Ordering::SeqCst);
    assert_eq!(progress(&h, r.clone(), vec![Cap::Progress]).await, current);
    assert!(s.key_backend.fail_read.swap(false, Ordering::SeqCst));
    assert!(s.token_backend.fail_read.swap(false, Ordering::SeqCst));
    s.key_backend.data.lock().unwrap().clear();
    s.token_backend.data.lock().unwrap().clear();
    assert_eq!(progress(&h, r, vec![Cap::Progress]).await, current);
    let status = progress(
        &h,
        request("access.status", json!({})),
        vec![Cap::AccessStatus],
    )
    .await;
    assert_eq!(status["result"]["localCredentialUsable"], false);
    assert_eq!(receipts(&s).await, before);
    assert_eq!(s.mock.count("complete/"), 1);
    assert_eq!(s.mock.requests.lock().unwrap().len(), 1);
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 1);
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn host_progress_rejects_self_grants_wrong_scope_revision_and_remote_entry() {
    let (s, h) = fixture().await;
    let r = request("progress", json!({}));
    for caps in [
        vec![],
        vec![Cap::Status],
        vec![Cap::AccessStatus],
        vec![Cap::RecoverAccess],
    ] {
        assert_eq!(
            progress(&h, r.clone(), caps).await["error"]["code"],
            "POLICY_DENIED"
        );
    }
    let mut stale = r.clone();
    stale["expectedRevision"] = json!(2);
    assert_eq!(
        progress(&h, stale, vec![Cap::Progress]).await["error"]["code"],
        "REVISION_CONFLICT"
    );
    let mut forged = r.clone();
    forged["payload"]["capabilities"] = json!(["Progress"]);
    assert_eq!(
        progress(&h, forged, vec![Cap::Progress]).await["error"]["code"],
        "VALIDATION_ERROR"
    );
    let foreign = CommunityScope::new(
        "foreign",
        "https://example.test/sub/api/v1/",
        "https://example.test",
        "test-community",
    )
    .unwrap();
    assert_eq!(
        h.dispatch_local(
            r.clone(),
            crate::community_host::local::LocalAccess::new(foreign, vec![Cap::Progress])
        )
        .await
        .unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(
        h.dispatch_registration(r, grant(&s, Cap::Progress, None))
            .await
            .unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(receipts(&s).await, 0);
    assert!(s.mock.requests.lock().unwrap().is_empty());
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn host_progress_rechecks_current_policy_and_registry_fence() {
    let (s, h) = fixture().await;
    let r = request("progress", json!({}));
    assert_eq!(
        progress(&h, r.clone(), vec![Cap::Progress]).await["ok"],
        true
    );
    for sql in [
        "UPDATE profiles SET body=json_set(body,'$.status','archived')",
        "UPDATE profiles SET body=json_set(body,'$.mode','conscious')",
        "INSERT INTO experimental_instances VALUES('profile-a','exp',1,'{}')",
    ] {
        main_sql(&s, sql).await;
        assert_eq!(
            progress(&h, r.clone(), vec![Cap::Progress]).await["error"]["code"],
            "POLICY_DENIED"
        );
        main_sql(&s, "DELETE FROM experimental_instances").await;
        main_sql(
            &s,
            "UPDATE profiles SET body=json_set(body,'$.status','draft','$.mode','standard')",
        )
        .await;
    }
    std::fs::write(s.registry.restore_fence(), b"pending").unwrap();
    assert_eq!(
        progress(&h, r.clone(), vec![Cap::Progress]).await["error"]["code"],
        "POLICY_DENIED"
    );
    std::fs::remove_file(s.registry.restore_fence()).unwrap();
    sqlx::query("UPDATE community_profile_lifecycle SET state='reconcile'")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    assert_eq!(
        progress(&h, r.clone(), vec![Cap::Progress]).await["error"]["code"],
        "POLICY_DENIED"
    );
    sqlx::query("UPDATE community_profile_lifecycle SET state='active'")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    main_sql(&s,"INSERT INTO profile_deletion_jobs(profile_id,request_id,fingerprint,expected_revision) VALUES('profile-a','pending-delete','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',1)").await;
    assert_eq!(
        progress(&h, r, vec![Cap::Progress]).await["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(receipts(&s).await, 0);
    assert!(s.mock.requests.lock().unwrap().is_empty());
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn host_progress_corrupt_finish_fails_without_repair_or_http() {
    let (s, h) = fixture().await;
    assert_eq!(
        send(
            &s,
            &h,
            request("complete", json!({"registrationRevision":3})),
            Cap::Complete
        )
        .await["ok"],
        true
    );
    let before = receipts(&s).await;
    sqlx::query("UPDATE community_registration_finishes SET body='{}'")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    let r = progress(&h, request("progress", json!({})), vec![Cap::Progress]).await;
    assert_eq!(r["error"]["code"], "STORAGE_ERROR");
    let body: String = sqlx::query_scalar("SELECT body FROM community_registration_finishes")
        .fetch_one(&s.registry.pool)
        .await
        .unwrap();
    assert_eq!(body, "{}");
    assert_eq!(receipts(&s).await, before);
    assert_eq!(s.mock.requests.lock().unwrap().len(), 1);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_failed_terminal_commit_keeps_claim_and_never_redispatches() {
    let (s, h) = fixture().await;
    main_sql(&s,"CREATE TRIGGER fail_finish BEFORE UPDATE ON receipts BEGIN SELECT RAISE(ABORT,'test'); END").await;
    let r = request("complete", json!({"registrationRevision":3}));
    assert_eq!(
        send(&s, &h, r.clone(), Cap::Complete).await["error"]["code"],
        "STORAGE_ERROR"
    );
    assert_eq!(s.mock.count("complete/"), 1);
    main_sql(&s, "DROP TRIGGER fail_finish").await;
    h.close_application().await.unwrap();
    h.open_application().await.unwrap();
    assert_eq!(
        send(&s, &h, r, Cap::Complete).await["error"]["code"],
        "REGISTRATION_RECONCILIATION_REQUIRED"
    );
    assert_eq!(s.mock.count("complete/"), 1);
    // A fresh explicit complete safely observes the stored credential; no second POST.
    let p = progress(&h, request("progress", json!({})), vec![Cap::Progress]).await;
    assert_eq!(p["result"]["finishState"], "ready_metadata");
    assert_eq!(
        send(
            &s,
            &h,
            request("complete", json!({"registrationRevision":3})),
            Cap::Complete
        )
        .await["ok"],
        true
    );
    assert_eq!(s.mock.count("complete/"), 1);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_receipt_replay_rechecks_policy_and_rejects_corruption() {
    let (s, h) = fixture().await;
    let r = request("complete", json!({"registrationRevision":3}));
    let success = send(&s, &h, r.clone(), Cap::Complete).await;
    assert_eq!(success["ok"], true);
    assert_eq!(
        send(&s, &h, r.clone(), Cap::RetryComplete).await["error"]["code"],
        "POLICY_DENIED"
    );
    main_sql(
        &s,
        "UPDATE profiles SET body=json_set(body,'$.status','archived')",
    )
    .await;
    assert_eq!(
        send(&s, &h, r.clone(), Cap::Complete).await["error"]["code"],
        "POLICY_DENIED"
    );
    main_sql(
        &s,
        "UPDATE profiles SET body=json_set(body,'$.status','draft')",
    )
    .await;
    for raw in [
        "null",
        "{}",
        "{\"communityLocalPending\":true,\"extra\":true}",
    ] {
        let mut c = sqlx::SqliteConnection::connect(&format!(
            "sqlite://{}",
            s.temp.0.join("foundation.sqlite3").display()
        ))
        .await
        .unwrap();
        sqlx::query("UPDATE receipts SET response=?")
            .bind(raw)
            .execute(&mut c)
            .await
            .unwrap();
        c.close().await.unwrap();
        assert_eq!(
            send(&s, &h, r.clone(), Cap::Complete).await["error"]["code"],
            "STORAGE_ERROR"
        );
    }
    assert_eq!(s.mock.count("complete/"), 1);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_invalid_invite_stale_preparation_and_local_entry_cannot_send_http() {
    let (s, h) = fixture().await;
    let r = request("start", json!({"registrationRevision":3}));
    for invite in [None, Some(""), Some("bad\ninvite")] {
        assert_eq!(
            h.dispatch_registration(r.clone(), grant(&s, Cap::Start, invite))
                .await
                .unwrap()["error"]["code"],
            "VALIDATION_ERROR"
        );
    }
    assert_eq!(receipts(&s).await, 0);
    let local = crate::community_host::local::LocalAccess::new(scope(), vec![Cap::Complete]);
    assert_eq!(
        h.dispatch_local(
            request("complete", json!({"registrationRevision":3})),
            local
        )
        .await
        .unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    let stale = send(
        &s,
        &h,
        request("complete", json!({"registrationRevision":1})),
        Cap::Complete,
    )
    .await;
    assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
    assert!(s.mock.requests.lock().unwrap().is_empty());
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_registry_reconcile_and_deletion_intent_block_http_and_success_replay() {
    let (s, h) = fixture().await;
    let r = request("complete", json!({"registrationRevision":3}));
    assert_eq!(send(&s, &h, r.clone(), Cap::Complete).await["ok"], true);
    sqlx::query("UPDATE community_profile_lifecycle SET state='reconcile'")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    assert_eq!(
        send(&s, &h, r.clone(), Cap::Complete).await["error"]["code"],
        "POLICY_DENIED"
    );
    sqlx::query("UPDATE community_profile_lifecycle SET state='active'")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    main_sql(&s,"INSERT INTO profile_deletion_jobs(profile_id,request_id,fingerprint,expected_revision) VALUES('profile-a','pending-delete','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',1)").await;
    assert_eq!(
        send(&s, &h, r, Cap::Complete).await["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(s.mock.count("complete/"), 1);
    h.close_application().await.unwrap();
}
use crate::{
    application::Application,
    community_host::{
        local::LocalCapability as Cap,
        maintenance::{CommunityHost, HostError, HostStatus},
        registration::RegistrationAccess,
        CommunityLifecycle,
    },
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use sqlx::Connection;
struct NoModels;
async fn main_sql(s: &Setup, sql: &str) {
    let mut c = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        s.temp.0.join("foundation.sqlite3").display()
    ))
    .await
    .unwrap();
    sqlx::query(sql).execute(&mut c).await.unwrap();
    c.close().await.unwrap();
}
impl SecretStore for NoModels {
    fn read(&self, _: &str) -> Result<Option<String>, VaultError> {
        Err(VaultError::Unavailable)
    }
    fn write_new(&self, _: &str, _: &str) -> Result<(), VaultError> {
        panic!("no provider writes")
    }
}
async fn fixture() -> (Setup, CommunityHost) {
    let s = setup_at(false, "community.sqlite3").await;
    let store = Store::open(&s.temp.0.join("foundation.sqlite3"))
        .await
        .unwrap();
    let defaults: Value =
        serde_json::from_str(include_str!("../../../../../../contracts/defaults.json")).unwrap();
    let p = json!({"schemaVersion":1,"id":"profile-a","revision":1,"createdAt":Utc::now().to_rfc3339_opts(SecondsFormat::Secs,true),"displayName":"Native","status":"draft","mode":"standard","scribeMode":"automatic","memorySettings":defaults["memorySettings"],"contextBudget":defaults["contextBudget"],"languagePolicy":"preserve_source"});
    assert!(crate::domain::schema::dto("Profile", &p));
    sqlx::query("INSERT INTO profiles VALUES('profile-a',1,?)")
        .bind(p.to_string())
        .execute(&store.pool)
        .await
        .unwrap();
    Application::new(store).close_for_maintenance().await;
    let h = CommunityHost::new(
        &s.temp.0,
        CommunityLifecycle::new(s.registry.clone(), s.keys.clone(), s.tokens.clone()),
        Vault::with_backend(Arc::new(NoModels)),
        None,
    )
    .unwrap();
    h.open_application().await.unwrap();
    (s, h)
}
fn request(command: &str, payload: Value) -> Value {
    json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":format!("community.registration.{command}"),"profileId":"profile-a","expectedRevision":1,"payload":payload})
}
fn grant(s: &Setup, cap: Cap, invite: Option<&str>) -> RegistrationAccess {
    let mut a =
        RegistrationAccess::new(scope(), vec![cap], invite.map(|v| Zeroizing::new(v.into())));
    a.set_loopback(s._server.url.clone());
    a
}
async fn send(s: &Setup, h: &CommunityHost, r: Value, cap: Cap) -> Value {
    h.dispatch_registration(r, grant(s, cap, None))
        .await
        .unwrap()
}
async fn receipts(s: &Setup) -> i64 {
    let mut c = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        s.temp.0.join("foundation.sqlite3").display()
    ))
    .await
    .unwrap();
    let n = sqlx::query_scalar("SELECT count(*) FROM receipts")
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    n
}
#[tokio::test]
async fn host_complete_replay_and_explicit_recovery_use_original_keys_without_token_projection() {
    let (s, h) = fixture().await;
    let r = request("complete", json!({"registrationRevision":3}));
    let first = send(&s, &h, r.clone(), Cap::Complete).await;
    assert_eq!(first["result"]["phase"], "credential_available", "{first}");
    assert_eq!(send(&s, &h, r, Cap::Complete).await, first);
    assert_eq!(s.mock.count("complete/"), 1);
    let rev = first["result"]["finishRevision"].as_i64().unwrap();
    let recovered = send(
        &s,
        &h,
        request("access.recover", json!({"finishRevision":rev})),
        Cap::RecoverAccess,
    )
    .await;
    assert_eq!(
        recovered["result"]["phase"], "credential_available",
        "{recovered}"
    );
    assert_eq!(s.mock.generation.load(Ordering::SeqCst), 2);
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
    for result in [first, recovered] {
        let wire = result.to_string();
        assert!(!wire.contains("eqai_"));
        assert!(!wire.contains("tokenRef"));
        assert_eq!(result["result"]["onlineStatusKnown"], false);
    }
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_start_invite_is_out_of_band_and_cached_replay_never_posts_twice() {
    let (s, h) = fixture().await;
    sqlx::query("DELETE FROM community_registration_starts")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    let r = request("start", json!({"registrationRevision":3}));
    let first = h
        .dispatch_registration(r.clone(), grant(&s, Cap::Start, Some("private-invite")))
        .await
        .unwrap();
    assert_eq!(first["result"]["phase"], "awaiting_complete", "{first}");
    assert_eq!(
        h.dispatch_registration(r.clone(), grant(&s, Cap::Start, Some("private-invite")))
            .await
            .unwrap(),
        first
    );
    assert_eq!(s.mock.count("start/"), 1);
    let conflict = h
        .dispatch_registration(r, grant(&s, Cap::Start, Some("changed-invite")))
        .await
        .unwrap();
    assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
    let bytes = std::fs::read(s.temp.0.join("foundation.sqlite3")).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("private-invite"));
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_wrong_grant_direction_revision_and_json_self_grant_deny_before_http() {
    let (s, h) = fixture().await;
    let r = request("complete", json!({"registrationRevision":3}));
    assert_eq!(
        send(&s, &h, r.clone(), Cap::Start).await["error"]["code"],
        "POLICY_DENIED"
    );
    let mut wrong = r.clone();
    wrong["expectedRevision"] = json!(2);
    assert_eq!(
        send(&s, &h, wrong, Cap::Complete).await["error"]["code"],
        "REVISION_CONFLICT"
    );
    let mut forged = r.clone();
    forged["payload"]["apiRoot"] = json!("https://elsewhere.invalid");
    assert_eq!(
        send(&s, &h, forged, Cap::Complete).await["error"]["code"],
        "VALIDATION_ERROR"
    );
    let mut access = RegistrationAccess::new(
        CommunityScope::new(
            "foreign",
            "https://example.test/sub/api/v1/",
            "https://example.test",
            "test-community",
        )
        .unwrap(),
        vec![Cap::Complete],
        None,
    );
    access.set_loopback(s._server.url.clone());
    assert_eq!(
        h.dispatch_registration(r.clone(), access).await.unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    let mut c = sqlx::SqliteConnection::connect(&format!(
        "sqlite://{}",
        s.temp.0.join("foundation.sqlite3").display()
    ))
    .await
    .unwrap();
    sqlx::query("INSERT INTO experimental_instances VALUES('profile-a','exp',1,'{}')")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        send(&s, &h, r, Cap::Complete).await["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(receipts(&s).await, 0);
    assert!(s.mock.requests.lock().unwrap().is_empty());
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn host_lost_complete_requires_fresh_explicit_retry_and_then_recovery() {
    let (mut s, mut h) = fixture().await;
    s.mock.fault("complete", Fault::Drop);
    let r = request("complete", json!({"registrationRevision":3}));
    assert_eq!(send(&s, &h, r.clone(), Cap::Complete).await["ok"], false);
    let n = s.mock.count("complete/");
    assert_eq!(send(&s, &h, r, Cap::Complete).await["ok"], false);
    assert_eq!(s.mock.count("complete/"), n);
    h = restart(&mut s, h).await;
    let p = progress(&h, request("progress", json!({})), vec![Cap::Progress]).await;
    assert_eq!(p["result"]["finishState"], "uncertain", "{p}");
    let rev = p["result"]["finishRevision"].as_i64().unwrap();
    let retry = send(
        &s,
        &h,
        request("complete.retry", json!({"finishRevision":rev})),
        Cap::RetryComplete,
    )
    .await;
    assert_eq!(
        retry["error"]["code"], "REGISTRATION_RECONCILIATION_REQUIRED",
        "{retry}"
    );
    h = restart(&mut s, h).await;
    let p = progress(&h, request("progress", json!({})), vec![Cap::Progress]).await;
    let rev = p["result"]["finishRevision"].as_i64().unwrap();
    assert_eq!(
        send(
            &s,
            &h,
            request("access.recover", json!({"finishRevision":rev})),
            Cap::RecoverAccess
        )
        .await["ok"],
        true
    );
    h.close_application().await.unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn host_cancelled_http_retains_main_lock_and_maintenance_gate() {
    let (s, h) = fixture().await;
    *s.mock.hold.lock().unwrap() = Some("complete/".into());
    let a = grant(&s, Cap::Complete, None);
    let clone = h.clone();
    let worker = tokio::spawn(async move {
        clone
            .dispatch_registration(request("complete", json!({"registrationRevision":3})), a)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), s.mock.processed.notified())
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(s.temp.0.join("foundation.sqlite3"))
            .busy_timeout(Duration::from_millis(50)),
    )
    .await
    .unwrap();
    assert!(sqlx::query("UPDATE profiles SET revision=revision+1")
        .execute(&mut c)
        .await
        .is_err());
    c.close().await.unwrap();
    let clone = h.clone();
    let close = tokio::spawn(async move { clone.close_application().await });
    while h.status() != HostStatus::Maintenance {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        h.dispatch_registration(
            request("complete", json!({"registrationRevision":3})),
            grant(&s, Cap::Complete, None)
        )
        .await,
        Err(HostError::Busy)
    );
    assert!(!close.is_finished());
    assert_eq!(
        h.dispatch_local(
            request("progress", json!({})),
            crate::community_host::local::LocalAccess::new(scope(), vec![Cap::Progress])
        )
        .await,
        Err(HostError::Busy)
    );
    s.mock.release.notify_one();
    close.await.unwrap().unwrap();
    assert_eq!(s.mock.count("complete/"), 1);
    h.open_application().await.unwrap();
    assert_eq!(
        s.finish
            .current(&s.registry, &s.keys, &scope())
            .await
            .unwrap()
            .ready_metadata,
        true
    );
    h.close_application().await.unwrap();
}
