//! Native host tests reuse the actual two-store fixtures and fake secret store.
use super::*;
use crate::community_host::maintenance::{CommunityHost, HostError, HostStatus};

fn host(f: &Fixture) -> CommunityHost {
    host_picker(f, None)
}
fn host_picker(f: &Fixture, picker: Option<Arc<dyn crate::imports::FilePicker>>) -> CommunityHost {
    CommunityHost::new(
        &f.root,
        CommunityLifecycle::new(
            f.registry.clone(),
            f.keys.clone(),
            CommunityTokenStore::with_backend(f.secret.clone()),
        ),
        Vault::with_backend(Arc::new(NoModels)),
        picker,
    )
    .unwrap()
}
async fn dispatched(h: &CommunityHost, request: Value) -> Value {
    let out = h.dispatch(request).await.unwrap().response;
    assert_eq!(out["ok"], true, "{out}");
    out["result"].clone()
}
async fn wait_status(h: &CommunityHost, status: HostStatus) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while h.status() != status {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn construction_is_offline_and_never_reads_or_creates_secrets() {
    let f = Fixture::new().await;
    let h = host(&f);
    assert_eq!(h.status(), HostStatus::Offline);
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Unavailable)
    ));
    assert_eq!(f.secret.reads.load(Ordering::SeqCst), 0);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    assert_eq!(f.secret.removes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn host_opens_dispatches_closes_and_reopens_without_lending_a_pool() {
    let f = Fixture::new().await;
    let h = host(&f);
    h.open_application().await.unwrap();
    h.clone().open_application().await.unwrap();
    assert_eq!(h.status(), HostStatus::Ready);
    dispatched(
        &h,
        request_cmd("profile.list", None, None, json!({"limit":50})),
    )
    .await;
    h.close_application().await.unwrap();
    assert_eq!(h.status(), HostStatus::Offline);
    let owner = crate::app_ownership::acquire(&f.root.join("foundation.sqlite3"), true).unwrap();
    drop(owner);
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn backup_drains_live_application_and_keeps_it_offline_until_explicit_open() {
    let f = Fixture::new().await;
    f.seed().await;
    let reads = f.secret.reads.load(Ordering::SeqCst);
    let h = host(&f);
    h.open_application().await.unwrap();
    let m = h.create_backup(&f.bundle(), "host-capture").await.unwrap();
    assert_eq!(m["requestId"], "host-capture");
    assert_eq!(m["profileCount"], 1);
    assert_eq!(h.status(), HostStatus::Offline);
    assert_eq!(f.secret.reads.load(Ordering::SeqCst), reads);
    assert_eq!(
        h.create_backup(&f.bundle(), "host-capture").await.unwrap(),
        m
    );
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Unavailable)
    ));
    h.open_application().await.unwrap();
    // The frozen dispatcher cannot silently produce a Store-only backup.
    let out = h
        .dispatch(request_cmd("backup.create", None, None, json!({})))
        .await
        .unwrap()
        .response;
    assert_eq!(out["error"]["code"], "POLICY_DENIED");
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn host_restore_applies_snapshot_and_reopens_only_on_explicit_request() {
    let f = Fixture::new().await;
    f.seed().await;
    let h = host(&f);
    h.open_application().await.unwrap();
    h.create_backup(&f.bundle(), "capture").await.unwrap();
    h.open_application().await.unwrap();
    dispatched(
        &h,
        request_cmd(
            "profile.create",
            None,
            None,
            json!({"displayName":"Later unregistered local profile"}),
        ),
    )
    .await;
    let r = h
        .restore(&f.bundle(), "restore-host", RestorePolicy::SameInstallation)
        .await
        .unwrap();
    assert_eq!(h.status(), HostStatus::Offline);
    assert_eq!(h.receipt("restore-host").unwrap(), Some(r));
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
    assert_eq!(f.main_count().await, 1);
}

#[tokio::test]
async fn bad_bundle_closes_admission_but_preserves_current_database() {
    let f = Fixture::new().await;
    f.seed().await;
    let h = host(&f);
    h.open_application().await.unwrap();
    assert!(h
        .restore(
            &f.bundle(),
            "missing-input",
            RestorePolicy::SameInstallation
        )
        .await
        .is_err());
    assert_eq!(h.status(), HostStatus::Offline);
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
    assert_eq!(f.main_count().await, 1);
}

#[tokio::test]
async fn bootstrap_keeps_malformed_native_marker_and_registry_fence_closed() {
    for fence in [false, true] {
        let f = Fixture::new().await;
        let path = if fence {
            f.registry.restore_fence()
        } else {
            f.root.join(crate::app_ownership::PENDING)
        };
        std::fs::write(&path, b"broken journal").unwrap();
        let before = hash(&f.root.join("foundation.sqlite3")).unwrap();
        let h = host(&f);
        assert_eq!(h.status(), HostStatus::RecoveryRequired);
        assert_eq!(h.open_application().await, Err(HostError::Unavailable));
        assert_eq!(hash(&f.root.join("foundation.sqlite3")).unwrap(), before);
        assert!(h.finish_cleanup("unknown").await.is_err());
        assert!(path.exists());
        assert_eq!(h.status(), HostStatus::RecoveryRequired);
    }
}

#[tokio::test]
async fn absent_main_is_never_replaced_by_empty_store_at_bootstrap() {
    let f = Fixture::new().await;
    std::fs::remove_file(f.root.join("foundation.sqlite3")).unwrap();
    let h = host(&f);
    assert_eq!(h.status(), HostStatus::RecoveryRequired);
    assert_eq!(h.open_application().await, Err(HostError::Unavailable));
    assert!(!f.root.join("foundation.sqlite3").exists());
}

#[tokio::test]
async fn host_rejects_another_installations_registry_owner() {
    let f = Fixture::new().await;
    let other = Fixture::new().await;
    let result = CommunityHost::new(
        &f.root,
        CommunityLifecycle::new(
            other.registry.clone(),
            f.keys.clone(),
            CommunityTokenStore::with_backend(f.secret.clone()),
        ),
        Vault::with_backend(Arc::new(NoModels)),
        None,
    );
    assert!(matches!(result, Err(HostError::Unavailable)));
}

#[tokio::test]
async fn wrong_durable_pin_does_not_open_ready_application() {
    let f = Fixture::new().await;
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_host_binding(id,registry_binding) VALUES(1,?)")
        .bind("a".repeat(64))
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let h = host(&f);
    assert_eq!(h.open_application().await, Err(HostError::Unavailable));
    assert_eq!(h.status(), HostStatus::RecoveryRequired);
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Unavailable)
    ));
}

#[tokio::test]
async fn host_bootstrap_resumes_each_real_native_restore_interruption() {
    for point in 1..=4 {
        let source = Fixture::new().await;
        source.seed().await;
        create(&source).await;
        let target = Fixture::new().await;
        let core = target.core();
        core.fault.store(point, Ordering::SeqCst);
        assert_eq!(
            core.restore(
                &source.bundle(),
                "restart-host",
                RestorePolicy::FreshInstallationRelocation
            )
            .await,
            Err(BackupError::Io)
        );
        drop(core);
        let h = host(&target);
        assert_eq!(h.status(), HostStatus::RecoveryRequired);
        assert_eq!(h.open_application().await, Err(HostError::Unavailable));
        h.restore(
            &source.bundle(),
            "restart-host",
            RestorePolicy::FreshInstallationRelocation,
        )
        .await
        .unwrap();
        assert_eq!(h.status(), HostStatus::Offline);
        h.open_application().await.unwrap();
        h.close_application().await.unwrap();
        assert_eq!(target.main_count().await, 1);
    }
}

#[tokio::test]
async fn host_cleanup_uses_committed_receipt_when_original_input_is_lost() {
    let f = Fixture::new().await;
    f.seed().await;
    create(&f).await;
    let core = f.core();
    core.fault.store(4, Ordering::SeqCst);
    assert_eq!(
        core.restore(&f.bundle(), "cleanup-host", RestorePolicy::SameInstallation)
            .await,
        Err(BackupError::Io)
    );
    std::fs::remove_dir_all(f.bundle()).unwrap();
    let h = host(&f);
    assert!(h.receipt("cleanup-host").unwrap().is_some());
    h.finish_cleanup("cleanup-host").await.unwrap();
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn cancelled_host_waiter_retains_gate_until_two_store_backup_finishes() {
    let f = Fixture::new().await;
    f.seed().await;
    let h = host(&f);
    h.open_application().await.unwrap();
    let lease = f.registry.maintenance().await;
    let worker = tokio::spawn({
        let h = h.clone();
        let path = f.bundle();
        async move { h.create_backup(&path, "cancel-host").await }
    });
    wait_status(&h, HostStatus::Maintenance).await;
    // Wait until the core holds Main ownership and is waiting on registry.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if crate::app_ownership::acquire(&f.root.join("foundation.sqlite3"), false).is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    worker.abort();
    let _ = worker.await;
    assert_eq!(h.status(), HostStatus::Maintenance);
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Busy)
    ));
    assert_eq!(h.open_application().await, Err(HostError::Busy));
    assert_eq!(
        h.create_backup(&f.temp.0.join("second"), "other").await,
        Err(HostError::Busy)
    );
    drop(lease);
    wait_status(&h, HostStatus::Offline).await;
    assert_eq!(
        read_json(&f.bundle().join("manifest.json")).unwrap()["requestId"],
        "cancel-host"
    );
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
}

struct PausedPicker {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl crate::imports::FilePicker for PausedPicker {
    fn pick(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<PathBuf>, ()>> + Send + '_>>
    {
        Box::pin(async {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(vec![])
        })
    }
}
#[tokio::test]
async fn maintenance_stops_new_dispatch_and_drains_cancelled_existing_dispatch() {
    let f = Fixture::new().await;
    let (p, _) = f.seed().await;
    let mut c = connection(&f.root.join("foundation.sqlite3"), true, false)
        .await
        .unwrap();
    let conversation: String = sqlx::query_scalar("SELECT id FROM conversations LIMIT 1")
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let picker = Arc::new(PausedPicker {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let h = host_picker(&f, Some(picker.clone()));
    h.open_application().await.unwrap();
    let dispatch = tokio::spawn({
        let h = h.clone();
        let id = p["id"].as_str().unwrap().to_string();
        async move {
            h.dispatch(request_cmd(
                "material.select",
                Some(&id),
                None,
                json!({"conversationId":conversation}),
            ))
            .await
        }
    });
    tokio::time::timeout(Duration::from_secs(15), picker.entered.notified())
        .await
        .unwrap();
    let backup = tokio::spawn({
        let h = h.clone();
        let path = f.bundle();
        async move { h.create_backup(&path, "drained-dispatch").await }
    });
    wait_status(&h, HostStatus::Maintenance).await;
    dispatch.abort();
    let _ = dispatch.await;
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Busy)
    ));
    assert!(!f.bundle().exists());
    assert!(!backup.is_finished());
    picker.release.notify_one();
    backup.await.unwrap().unwrap();
    assert_eq!(h.status(), HostStatus::Offline);
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
}

#[tokio::test]
async fn host_does_not_apply_or_bypass_legacy_restore_ledger() {
    for body in [
        b"{\"requestId\":\"legacy-pending\"}".as_slice(),
        b"broken ledger".as_slice(),
    ] {
        let f = Fixture::new().await;
        let requests = f.root.join("restore-ledger/requests");
        std::fs::create_dir_all(&requests).unwrap();
        std::fs::write(requests.join("pending.json"), body).unwrap();
        let original = hash(&f.root.join("foundation.sqlite3")).unwrap();
        let h = host(&f);
        assert_eq!(h.status(), HostStatus::RecoveryRequired);
        assert_eq!(h.open_application().await, Err(HostError::Unavailable));
        assert_eq!(
            h.create_backup(&f.bundle(), "blocked-by-legacy").await,
            Err(HostError::Unavailable)
        );
        assert!(!f.bundle().exists());
        assert_eq!(hash(&f.root.join("foundation.sqlite3")).unwrap(), original);
        assert!(requests.join("pending.json").exists());
    }
}

#[tokio::test]
async fn busy_foreign_store_is_not_forced_closed_and_host_can_retry_after_drain() {
    let f = Fixture::new().await;
    let other = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    let h = host(&f);
    assert_eq!(
        h.create_backup(&f.bundle(), "external-owner").await,
        Err(HostError::Busy)
    );
    assert!(!f.bundle().exists());
    assert!(!other.pool.is_closed());
    other.pool.close().await;
    drop(other);
    // Cooperative ownership remains the final arbiter after timeout.
    crate::app_ownership::drained_exclusive(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    h.create_backup(&f.bundle(), "external-owner")
        .await
        .unwrap();
    assert_eq!(h.status(), HostStatus::Offline);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_host_clones_preserve_admission_and_terminal_phase() {
    let f = Fixture::new().await;
    let h = host(&f);
    for _ in 0..8 {
        let start = Arc::new(tokio::sync::Barrier::new(8));
        let mut workers = vec![];
        for n in 0..8 {
            let h = h.clone();
            let start = start.clone();
            workers.push(tokio::spawn(async move {
                start.wait().await;
                let r = if n % 2 == 0 {
                    h.open_application().await
                } else {
                    h.close_application().await
                };
                assert!(r.is_ok() || r == Err(HostError::Busy), "{r:?}");
            }));
        }
        for worker in workers {
            worker.await.unwrap();
        }
        match h.status() {
            HostStatus::Ready => {
                dispatched(
                    &h,
                    request_cmd("profile.list", None, None, json!({"limit":50})),
                )
                .await;
            }
            HostStatus::Offline => {
                assert!(matches!(
                    h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
                        .await,
                    Err(HostError::Unavailable)
                ));
            }
            other => panic!("terminal host phase after joined workers: {other:?}"),
        }
    }
    h.close_application().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn host_preserves_offline_cores_symlink_root_rejection() {
    let f = Fixture::new().await;
    let link = f.temp.0.join("alias");
    std::os::unix::fs::symlink(&f.root, &link).unwrap();
    assert!(matches!(
        CommunityHost::new(
            &link,
            CommunityLifecycle::new(
                f.registry.clone(),
                f.keys.clone(),
                CommunityTokenStore::with_backend(f.secret.clone())
            ),
            Vault::with_backend(Arc::new(NoModels)),
            None
        ),
        Err(HostError::Backup(BackupError::InvalidBundle))
    ));
}

fn legacy_request() -> Value {
    json!({"manifestVersion":2,"requestId":"legacy-R","backupId":"manual-original","backupSha256":"ab".repeat(32),"safetyBackupId":"safety-original","requestedAt":"2026-10-27T12:00:00+00:00"})
}
fn legacy_result() -> Value {
    json!({"applied":true,"requestId":"legacy-R","backupId":"manual-original","safetyBackupId":"safety-original","appliedAt":"2026-10-27T12:01:00+00:00","sourceSchemaVersion":16,"restoredSchemaVersion":16})
}
fn write_legacy_pair(f: &Fixture, req: &Value, result: &Value) -> (PathBuf, PathBuf) {
    let key = format!("{:x}", Sha256::digest(b"ai-home:restore-request:legacy-R"));
    let request = f
        .root
        .join("restore-ledger/requests")
        .join(format!("{key}.json"));
    let result_path = f
        .root
        .join("restore-ledger/results")
        .join(format!("{key}.json"));
    for p in [&request, &result_path] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    }
    std::fs::write(&request, req.to_string()).unwrap();
    std::fs::write(&result_path, result.to_string()).unwrap();
    (request, result_path)
}
async fn assert_legacy_pair_blocked(req: Value, result: Value) {
    let f = Fixture::new().await;
    let input = f.temp.0.join("valid-native-source");
    f.core().create(&input, "before-legacy").await.unwrap();
    let before = hash(&f.root.join("foundation.sqlite3")).unwrap();
    let (request_path, result_path) = write_legacy_pair(&f, &req, &result);
    let request_bytes = std::fs::read(&request_path).unwrap();
    let result_bytes = std::fs::read(&result_path).unwrap();
    let h = host(&f);
    assert_eq!(
        h.status(),
        HostStatus::RecoveryRequired,
        "request={req}, result={result}"
    );
    assert_eq!(h.open_application().await, Err(HostError::Unavailable));
    assert!(matches!(
        h.dispatch(request_cmd("profile.list", None, None, json!({"limit":50})))
            .await,
        Err(HostError::Unavailable)
    ));
    let output = f.temp.0.join("must-not-publish");
    assert_eq!(
        h.create_backup(&output, "denied-capture").await,
        Err(HostError::Unavailable)
    );
    assert_eq!(
        h.restore(&input, "denied-restore", RestorePolicy::SameInstallation)
            .await,
        Err(HostError::Unavailable)
    );
    assert!(h.receipt("denied-restore").unwrap().is_none());
    assert_eq!(h.status(), HostStatus::RecoveryRequired);
    assert!(!output.exists());
    assert!(!crate::app_ownership::pending(&f.root));
    assert!(!f.registry.restore_fence().exists());
    assert_eq!(hash(&f.root.join("foundation.sqlite3")).unwrap(), before);
    assert_eq!(std::fs::read(request_path).unwrap(), request_bytes);
    assert_eq!(std::fs::read(result_path).unwrap(), result_bytes);
}

#[tokio::test]
async fn legacy_terminal_null_empty_or_nonobject_result_never_unlocks_host() {
    for result in [Value::Null, json!({}), json!([]), json!("completed")] {
        assert_legacy_pair_blocked(legacy_request(), result).await;
    }
}
#[tokio::test]
async fn legacy_terminal_false_or_nonboolean_applied_never_unlocks_host() {
    for applied in [json!(false), json!("true"), json!(1)] {
        let mut r = legacy_result();
        r["applied"] = applied;
        assert_legacy_pair_blocked(legacy_request(), r).await;
    }
}
#[tokio::test]
async fn legacy_terminal_other_request_never_unlocks_host() {
    let mut r = legacy_result();
    r["requestId"] = json!("another-request");
    assert_legacy_pair_blocked(legacy_request(), r).await;
}
#[tokio::test]
async fn legacy_terminal_other_backup_never_unlocks_host() {
    let mut r = legacy_result();
    r["backupId"] = json!("manual-other");
    assert_legacy_pair_blocked(legacy_request(), r).await;
}
#[tokio::test]
async fn legacy_terminal_other_safety_backup_never_unlocks_host() {
    let mut r = legacy_result();
    r["safetyBackupId"] = json!("safety-other");
    assert_legacy_pair_blocked(legacy_request(), r).await;
}
#[tokio::test]
async fn legacy_terminal_missing_or_mistyped_fields_never_unlock_host() {
    for field in [
        "applied",
        "requestId",
        "backupId",
        "safetyBackupId",
        "appliedAt",
        "sourceSchemaVersion",
        "restoredSchemaVersion",
    ] {
        let mut r = legacy_result();
        r.as_object_mut().unwrap().remove(field);
        assert_legacy_pair_blocked(legacy_request(), r).await;
        let mut r = legacy_result();
        r[field] = Value::Null;
        assert_legacy_pair_blocked(legacy_request(), r).await;
    }
    for (field, value) in [
        ("appliedAt", json!("not-a-date")),
        ("sourceSchemaVersion", json!(0)),
        ("restoredSchemaVersion", json!(17)),
        ("restoredSchemaVersion", json!(15)),
        ("sourceSchemaVersion", json!("16")),
    ] {
        let mut r = legacy_result();
        r[field] = value;
        assert_legacy_pair_blocked(legacy_request(), r).await;
    }
}
#[tokio::test]
async fn legacy_request_shape_is_checked_even_with_existing_terminal_result() {
    for field in [
        "manifestVersion",
        "requestId",
        "backupId",
        "backupSha256",
        "safetyBackupId",
        "requestedAt",
    ] {
        let mut req = legacy_request();
        req.as_object_mut().unwrap().remove(field);
        assert_legacy_pair_blocked(req, legacy_result()).await;
        let mut req = legacy_request();
        req[field] = Value::Null;
        assert_legacy_pair_blocked(req, legacy_result()).await;
    }
    for (field, value) in [
        ("manifestVersion", json!(1)),
        ("backupSha256", json!("bad")),
        ("backupId", json!("../outside")),
        ("requestedAt", json!("invalid-time")),
        ("requestId", json!("x".repeat(129))),
    ] {
        let mut req = legacy_request();
        req[field] = value;
        assert_legacy_pair_blocked(req, legacy_result()).await;
    }
}
#[tokio::test]
async fn legacy_unknown_fields_are_not_silently_treated_as_current_format() {
    let mut req = legacy_request();
    req["futureField"] = json!(true);
    assert_legacy_pair_blocked(req, legacy_result()).await;
    let mut result = legacy_result();
    result["futureField"] = json!(true);
    assert_legacy_pair_blocked(legacy_request(), result).await;
}
#[tokio::test]
async fn legacy_valid_historical_pair_allows_open_and_both_native_operations() {
    let f = Fixture::new().await;
    let (request_path, result_path) = write_legacy_pair(&f, &legacy_request(), &legacy_result());
    let request_bytes = std::fs::read(&request_path).unwrap();
    let result_bytes = std::fs::read(&result_path).unwrap();
    let h = host(&f);
    assert_eq!(h.status(), HostStatus::Offline);
    h.open_application().await.unwrap();
    dispatched(
        &h,
        request_cmd("profile.list", None, None, json!({"limit":50})),
    )
    .await;
    h.create_backup(&f.bundle(), "valid-history-capture")
        .await
        .unwrap();
    h.restore(
        &f.bundle(),
        "valid-history-restore",
        RestorePolicy::SameInstallation,
    )
    .await
    .unwrap();
    h.open_application().await.unwrap();
    h.close_application().await.unwrap();
    assert_eq!(std::fs::read(request_path).unwrap(), request_bytes);
    assert_eq!(std::fs::read(result_path).unwrap(), result_bytes);
}
#[tokio::test]
async fn legacy_actual_writer_receipt_stays_valid_after_later_main_changes() {
    let f = Fixture::new().await;
    let store = Store::open(&f.root.join("foundation.sqlite3"))
        .await
        .unwrap();
    let app = Application::with_vault(store, Vault::with_backend(Arc::new(NoModels)));
    let backup = ok(&app, request_cmd("backup.create", None, None, json!({}))).await;
    ok(
        &app,
        request_cmd(
            "backup.restore.request",
            None,
            None,
            json!({"backupId":backup["id"]}),
        ),
    )
    .await;
    app.close_for_maintenance().await;
    crate::backup::apply_pending_restore(&f.root)
        .await
        .unwrap()
        .unwrap();
    let old_hash = hash(&f.root.join("foundation.sqlite3")).unwrap();
    let h = host(&f);
    h.open_application().await.unwrap();
    dispatched(
        &h,
        request_cmd(
            "app.preferences.set",
            None,
            Some(1),
            json!({"locale":"pl","themeId":"orbital","themeVariant":"system"}),
        ),
    )
    .await;
    h.close_application().await.unwrap();
    assert_ne!(hash(&f.root.join("foundation.sqlite3")).unwrap(), old_hash);
    let next = host(&f);
    assert_eq!(next.status(), HostStatus::Offline);
    next.open_application().await.unwrap();
    next.create_backup(&f.bundle(), "after-legacy-change")
        .await
        .unwrap();
}
