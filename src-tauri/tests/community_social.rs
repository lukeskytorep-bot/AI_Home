use ai_home::{
    application::Application,
    community_identity::verify_signature,
    community_social::canonical_post_statement,
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct TempDb(PathBuf);
impl TempDb {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ai_home_comm_social_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.join("app.sqlite3"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(p) = self.0.parent() {
            let _ = std::fs::remove_dir_all(p);
        }
    }
}

#[derive(Default, Clone)]
struct MemVault(Arc<Mutex<HashMap<String, String>>>);
impl SecretStore for MemVault {
    fn read(&self, key: &str) -> Result<Option<String>, VaultError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn write_new(&self, key: &str, secret: &str) -> Result<(), VaultError> {
        let mut g = self.0.lock().unwrap();
        if g.contains_key(key) {
            return Err(VaultError::Invalid);
        }
        g.insert(key.into(), secret.into());
        Ok(())
    }
}

fn req(command: &str, request_id: &str, profile_id: Option<&str>, payload: Value) -> Value {
    let mut env = json!({
        "contractVersion": 1,
        "requestId": request_id,
        "command": command,
        "payload": payload,
    });
    if let Some(pid) = profile_id {
        env["profileId"] = json!(pid);
    }
    env
}

async fn create_profile(app: &Application, req_id: &str, name: &str) -> String {
    let d = app
        .dispatch(req("profile.create", req_id, None, json!({"displayName": name})))
        .await;
    assert_eq!(d.response["ok"], true, "profile.create failed: {}", d.response);
    d.response["result"]["id"].as_str().unwrap().to_string()
}

async fn spawn_mock_community(
    disconnect_post: bool,
) -> (
    String,
    Arc<Mutex<Vec<(String, String, Option<String>, Option<Value>)>>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured: Arc<Mutex<Vec<(String, String, Option<String>, Option<Value>)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let cap_clone = captured.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let cap = cap_clone.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let mut header_end = None;
                let mut content_len = 0usize;
                loop {
                    let Ok(n) = socket.read(&mut tmp).await else {
                        return;
                    };
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if header_end.is_none() {
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            header_end = Some(pos + 4);
                            let hdr = String::from_utf8_lossy(&buf[..pos]);
                            for line in hdr.lines() {
                                if let Some(v) = line
                                    .strip_prefix("content-length:")
                                    .or_else(|| line.strip_prefix("Content-Length:"))
                                {
                                    content_len = v.trim().parse().unwrap_or(0);
                                }
                            }
                        }
                    }
                    if let Some(end) = header_end {
                        if buf.len() >= end + content_len {
                            break;
                        }
                    }
                }
                let Some(end) = header_end else { return };
                let hdr_text = String::from_utf8_lossy(&buf[..end]).to_string();
                let mut lines = hdr_text.lines();
                let first = lines.next().unwrap_or("");
                let mut parts = first.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let path = parts.next().unwrap_or("").to_string();
                let auth = lines.find_map(|l| {
                    let lower = l.to_ascii_lowercase();
                    if lower.starts_with("authorization:") {
                        Some(l[14..].trim().to_string())
                    } else {
                        None
                    }
                });
                let body_bytes = &buf[end..];
                let parsed_body: Option<Value> = if body_bytes.is_empty() {
                    None
                } else {
                    serde_json::from_slice(body_bytes).ok()
                };
                cap.lock()
                    .unwrap()
                    .push((method.clone(), path.clone(), auth, parsed_body));

                if method == "POST" && disconnect_post {
                    drop(socket);
                    return;
                }

                let resp_body = if method == "GET" {
                    json!({
                        "ok": true,
                        "request_id": "mock-feed-1",
                        "items": [
                            {"id": "msg_1", "title": "Hello agents", "author": "alice", "content": "Welcome!"}
                        ]
                    })
                    .to_string()
                } else {
                    json!({
                        "ok": true,
                        "id": "post_9001"
                    })
                    .to_string()
                };
                let http_resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    resp_body.len(),
                    resp_body
                );
                let _ = socket.write_all(http_resp.as_bytes()).await;
            });
        }
    });

    (format!("http://127.0.0.1:{}/api/v1", addr.port()), captured)
}

#[tokio::test]
async fn community_social_feed_and_signed_publish_verified_cryptographically() {
    let db = TempDb::new("signed_publish");
    let vault = MemVault::default();
    let app = Application::with_vault(
        Store::open(&db.0).await.unwrap(),
        Vault::with_backend(Arc::new(vault)),
    );
    let pid = create_profile(&app, "prof-1", "Social Agent").await;
    let (api_root, captured) = spawn_mock_community(false).await;

    // Save read-only account first
    let saved = app
        .dispatch(req(
            "community.account.save",
            "save-1",
            Some(&pid),
            json!({
                "name": "Agent Commons",
                "apiRoot": api_root,
                "boardId": "general",
                "apiToken": "secret-community-token",
                "allowWrites": false,
                "enabled": true
            }),
        ))
        .await;
    assert_eq!(saved.response["ok"], true, "save failed: {}", saved.response);
    let account = &saved.response["result"]["account"];
    let account_id = account["id"].as_str().unwrap().to_string();
    let pub_spki = account["publicKeySpki"].as_str().unwrap().to_string();
    assert!(!pub_spki.is_empty());
    assert_eq!(account["hasCredential"], true);
    assert!(account.get("apiToken").is_none());
    assert!(account.get("signingKeyRef").is_none());

    // Idempotent replay returns same account
    let replay = app
        .dispatch(req(
            "community.account.save",
            "save-1",
            Some(&pid),
            json!({
                "name": "Agent Commons",
                "apiRoot": api_root,
                "boardId": "general",
                "apiToken": "secret-community-token",
                "allowWrites": false,
                "enabled": true
            }),
        ))
        .await;
    assert_eq!(replay.response["ok"], true);
    assert_eq!(replay.response["result"]["account"]["id"], account_id);

    // Public feed read works without sending Authorization header
    let feed = app
        .dispatch(req(
            "community.feed.read",
            "feed-1",
            Some(&pid),
            json!({"accountId": account_id}),
        ))
        .await;
    assert_eq!(feed.response["ok"], true, "feed failed: {}", feed.response);
    assert_eq!(feed.response["result"]["untrustedExternalContent"], true);
    assert_eq!(feed.response["result"]["signatureValidation"], "not_performed");
    assert_eq!(feed.response["result"]["data"]["items"][0]["id"], "msg_1");
    {
        let log = captured.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, "GET");
        assert!(log[0].2.is_none(), "public feed must not send Authorization");
    }

    // Write blocked when allowWrites == false
    let denied_ro = app
        .dispatch(req(
            "community.post.publish",
            "pub-denied-ro",
            Some(&pid),
            json!({"accountId": account_id, "title": "Hi", "content": "Hello world"}),
        ))
        .await;
    assert_eq!(denied_ro.response["ok"], true);
    assert_eq!(denied_ro.response["result"]["status"], "policy_denied");

    // Enable writes
    let updated = app
        .dispatch(req(
            "community.account.save",
            "save-2",
            Some(&pid),
            json!({
                "accountId": account_id,
                "name": "Agent Commons",
                "apiRoot": api_root,
                "boardId": "general",
                "allowWrites": true,
                "enabled": true
            }),
        ))
        .await;
    assert_eq!(updated.response["ok"], true);
    assert_eq!(updated.response["result"]["account"]["publicKeySpki"], pub_spki);

    // Private reflection content is blocked even with allowWrites == true
    let denied_refl = app
        .dispatch(req(
            "community.post.publish",
            "pub-denied-refl",
            Some(&pid),
            json!({
                "accountId": account_id,
                "title": "Secret",
                "content": "My private dream",
                "sourceTags": ["private_reflection"]
            }),
        ))
        .await;
    assert_eq!(denied_refl.response["ok"], true);
    assert_eq!(denied_refl.response["result"]["status"], "policy_denied");
    assert_eq!(
        denied_refl.response["result"]["reason"],
        "private reflection content cannot be published"
    );

    // Signed publish succeeds and signature verifies with p256 VerifyingKey
    let pub_ok = app
        .dispatch(req(
            "community.post.publish",
            "pub-ok-1",
            Some(&pid),
            json!({
                "accountId": account_id,
                "title": "Agent intro",
                "content": "Hello from AI Home!",
                "sourceTags": ["public_draft"]
            }),
        ))
        .await;
    assert_eq!(pub_ok.response["ok"], true, "publish failed: {}", pub_ok.response);
    assert_eq!(pub_ok.response["result"]["status"], "published");
    assert_eq!(pub_ok.response["result"]["remoteId"], "post_9001");
    assert_eq!(pub_ok.response["result"]["signed"], true);

    let log = captured.lock().unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].0, "POST");
    assert_eq!(log[1].1, "/api/v1/threads");
    assert_eq!(log[1].2.as_deref(), Some("Bearer secret-community-token"));
    let body = log[1].3.as_ref().unwrap();
    let sig = body["signature"].as_str().unwrap();
    let body_sha = body["bodySha256"].as_str().unwrap();
    let statement = canonical_post_statement(
        &api_root,
        "general",
        "",
        "Agent intro",
        body_sha,
        "pub-ok-1",
    );
    assert!(
        verify_signature(statement.as_bytes(), &pub_spki, sig).unwrap(),
        "ECDSA P-256 signature must verify against account publicKeySpki"
    );
}

#[tokio::test]
async fn community_social_uncertain_disconnect_persists_and_reconciles_autonomous_recovery() {
    let db = TempDb::new("uncertain_reconcile");
    let vault = MemVault::default();
    let (api_root, _captured) = spawn_mock_community(true).await;

    let (pid, account_id) = {
        let app = Application::with_vault(
            Store::open(&db.0).await.unwrap(),
            Vault::with_backend(Arc::new(vault.clone())),
        );
        // Create Autonomous instance (creates backing profile too)
        let created = app
            .dispatch(req(
                "autonomous.instance.create",
                "auto-create-1",
                None,
                json!({"displayName": "Auto Social"}),
            ))
            .await;
        assert_eq!(created.response["ok"], true, "create auto failed: {}", created.response);
        let pid = created.response["result"]["profileId"]
            .as_str()
            .unwrap()
            .to_string();

        let saved = app
            .dispatch(req(
                "community.account.save",
                "save-auto-1",
                Some(&pid),
                json!({
                    "name": "Flaky Forum",
                    "apiRoot": api_root,
                    "allowWrites": true,
                    "enabled": true
                }),
            ))
            .await;
        assert_eq!(saved.response["ok"], true);
        let account_id = saved.response["result"]["account"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        // Publish hits server disconnect -> uncertain
        let unc = app
            .dispatch(req(
                "community.post.publish",
                "pub-unc-1",
                Some(&pid),
                json!({"accountId": account_id, "content": "Did this land?"}),
            ))
            .await;
        assert_eq!(unc.response["ok"], true);
        assert_eq!(unc.response["result"]["status"], "uncertain");
        assert_eq!(unc.response["result"]["receiptRef"], "pub-unc-1");

        // Autonomous instance is now recovery_required
        let status = app
            .dispatch(req("autonomous.instance.status", "st-1", Some(&pid), json!({})))
            .await;
        assert_eq!(status.response["result"]["instance"]["status"], "recovery_required");

        (pid, account_id)
    };

    // Reopen SQLite from disk (simulate restart)
    let app = Application::with_vault(
        Store::open(&db.0).await.unwrap(),
        Vault::with_backend(Arc::new(vault)),
    );

    // Subsequent publish is still blocked after restart
    let blocked = app
        .dispatch(req(
            "community.post.publish",
            "pub-after-restart",
            Some(&pid),
            json!({"accountId": account_id, "content": "Retry blocked"}),
        ))
        .await;
    assert_eq!(blocked.response["ok"], true);
    assert_eq!(blocked.response["result"]["status"], "policy_denied");
    assert_eq!(blocked.response["result"]["reason"], "reconciliation_required");

    // Reconcile with wrong receiptRef fails validation
    let bad_rec = app
        .dispatch(req(
            "community.account.save",
            "rec-bad",
            Some(&pid),
            json!({
                "accountId": account_id,
                "name": "Flaky Forum",
                "apiRoot": api_root,
                "allowWrites": true,
                "enabled": true,
                "reconcileReceiptRef": "wrong-ref"
            }),
        ))
        .await;
    assert_eq!(bad_rec.response["ok"], false);
    assert_eq!(bad_rec.response["error"]["code"], "VALIDATION_ERROR");

    // Reconcile with matching receiptRef clears pendingReconciliation AND restores Autonomous status -> ready
    let good_rec = app
        .dispatch(req(
            "community.account.save",
            "rec-good",
            Some(&pid),
            json!({
                "accountId": account_id,
                "name": "Flaky Forum",
                "apiRoot": api_root,
                "allowWrites": true,
                "enabled": true,
                "reconcileReceiptRef": "pub-unc-1"
            }),
        ))
        .await;
    assert_eq!(good_rec.response["ok"], true, "reconcile failed: {}", good_rec.response);
    assert!(good_rec.response["result"]["account"]["pendingReconciliation"].is_null());

    let status_after = app
        .dispatch(req("autonomous.instance.status", "st-2", Some(&pid), json!({})))
        .await;
    assert_eq!(
        status_after.response["result"]["instance"]["status"],
        "ready",
        "Autonomous instance must return to ready after reconciliation"
    );
}

#[tokio::test]
async fn community_social_blocks_during_ai_sleep_and_cleans_on_profile_delete() {
    let db = TempDb::new("sleep_and_delete");
    let vault = MemVault::default();
    let store = Store::open(&db.0).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::with_vault(store, Vault::with_backend(Arc::new(vault)));
    let (api_root, _captured) = spawn_mock_community(false).await;

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            "auto-sleep-1",
            None,
            json!({"displayName": "Sleepy Agent"}),
        ))
        .await;
    let pid = created.response["result"]["profileId"]
        .as_str()
        .unwrap()
        .to_string();
    let inst_id = created.response["result"]["instanceId"]
        .as_str()
        .unwrap()
        .to_string();

    let saved = app
        .dispatch(req(
            "community.account.save",
            "save-sleep-1",
            Some(&pid),
            json!({
                "name": "Forum",
                "apiRoot": api_root,
                "allowWrites": true,
                "enabled": true
            }),
        ))
        .await;
    let account_id = saved.response["result"]["account"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Insert active AI-chosen event_only sleep WakePlan
    let wake = json!({
        "schemaVersion": 1,
        "id": "wake-sleep-1",
        "instanceId": inst_id,
        "revision": 1,
        "origin": "ai_decision",
        "decisionId": "dec-sleep",
        "mode": "event_only",
        "interruptEvents": [],
        "status": "active",
        "createdAt": "2026-11-27T00:00:00Z"
    });
    sqlx::query(
        "INSERT INTO experimental_wake_plans(profile_id, instance_id, id, revision, body) VALUES(?,?,?,1,?)",
    )
    .bind(&pid)
    .bind(&inst_id)
    .bind("wake-sleep-1")
    .bind(wake.to_string())
    .execute(&pool)
    .await
    .unwrap();

    let blocked = app
        .dispatch(req(
            "community.post.publish",
            "pub-while-sleeping",
            Some(&pid),
            json!({"accountId": account_id, "content": "Should not send while sleeping"}),
        ))
        .await;
    assert_eq!(blocked.response["ok"], true);
    assert_eq!(blocked.response["result"]["status"], "policy_denied");
    assert_eq!(blocked.response["result"]["reason"], "autonomous instance is sleeping");

    // Pause then archive Autonomous instance first, then permanently delete profile
    let paused = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            "pause-auto-1",
            Some(&pid),
            json!({"action": "pause"}),
        ))
        .await;
    assert_eq!(paused.response["ok"], true, "pause failed: {}", paused.response);
    let archived = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            "arch-auto-1",
            Some(&pid),
            json!({"action": "archive"}),
        ))
        .await;
    assert_eq!(archived.response["ok"], true, "archive failed: {}", archived.response);
    let prof_rev: i64 = sqlx::query_scalar("SELECT revision FROM profiles WHERE id=?")
        .bind(&pid)
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut del_req = req(
        "profile.delete.permanent",
        "del-prof-1",
        Some(&pid),
        json!({"confirm": true}),
    );
    del_req["expectedRevision"] = json!(prof_rev);
    let del = app.dispatch(del_req).await;
    assert_eq!(del.response["ok"], true, "delete failed: {}", del.response);

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM community_accounts WHERE profile_id=?")
        .bind(&pid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);

    // Replay of tombstoned receipt returns POLICY_DENIED
    let replay_deleted = app
        .dispatch(req(
            "community.account.save",
            "save-sleep-1",
            Some(&pid),
            json!({
                "name": "Forum",
                "apiRoot": api_root,
                "allowWrites": true,
                "enabled": true
            }),
        ))
        .await;
    assert_eq!(replay_deleted.response["ok"], false);
    assert_eq!(replay_deleted.response["error"]["code"], "POLICY_DENIED");
}
