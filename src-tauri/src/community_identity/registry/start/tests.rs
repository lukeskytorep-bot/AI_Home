use super::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
#[derive(Default)]
struct Memory {
    data: Mutex<HashMap<String, Zeroizing<String>>>,
    creates: AtomicUsize,
}
impl CommunitySecretStore for Memory {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, s: &str) -> Result<(), IdentityError> {
        let mut data = self.data.lock().unwrap();
        if data.contains_key(r) {
            return Err(IdentityError::AlreadyExists);
        }
        data.insert(r.into(), Zeroizing::new(s.into()));
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.data.lock().unwrap().remove(r);
        Ok(())
    }
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn path(&self) -> std::path::PathBuf {
        self.0.join("registry.sqlite")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scope() -> CommunityScope {
    CommunityScope::new(
        "profile-a",
        "https://example.test/sub/api/v1/",
        "https://example.test",
        "test-community",
    )
    .unwrap()
}
async fn setup() -> (Temp, RegistrationRegistry, CommunityKeyStore, Arc<Memory>) {
    let temp = Temp(std::env::temp_dir().join(format!("ai-home-start-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&temp.0).unwrap();
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let backend = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(backend.clone());
    registry
        .prepare(
            &scope(),
            &RegistrationProfile::new("Agent Żółw", "Public about").unwrap(),
            "original-start-0123456789",
        )
        .await
        .unwrap();
    registry.provision(&keys, &scope(), 1).await.unwrap();
    (temp, registry, keys, backend)
}
async fn response(
    registry: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    state: &str,
    expires: DateTime<Utc>,
) -> Value {
    let intent = registry.load_intent(keys, &scope()).await.unwrap();
    let id = format!("reg_{}", "1".repeat(32));
    let nonce = super::super::super::crypto::b64(&[7u8; 32]);
    let expiry = expires.to_rfc3339_opts(SecondsFormat::Secs, true);
    let statement = super::super::super::registration_statement(
        &intent.scope,
        &intent.identity,
        &intent.profile,
        &id,
        &nonce,
        &expiry,
    );
    json!({"ok":true,"request_id":"server-request","registration":{"registration_id":id,"state":state,"nonce":nonce,"expires_at":expiry,"community_origin":intent.scope.origin,"community_instance":intent.scope.instance,"profile_format":"COMMUNITY-PROFILE/1","profile_sha256":intent.profile.sha256(),"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}})
}
async fn pending(r: &RegistrationRegistry, k: &CommunityKeyStore) -> Value {
    response(r, k, "pending", Utc::now() + chrono::Duration::minutes(10)).await
}
fn wire(status: u16, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut text = format!("HTTP/1.1 {status} Test\r\nConnection: close\r\n{headers}");
    let lower = headers.to_ascii_lowercase();
    if !lower.contains("content-length:") && !lower.contains("transfer-encoding:") {
        text.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    text.push_str("\r\n");
    let mut bytes = text.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
async fn server(
    raw: Vec<u8>,
    gate: Option<oneshot::Receiver<()>>,
    timeout: Duration,
) -> (
    RegistrationStart,
    oneshot::Receiver<String>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = oneshot::channel();
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 1024];
            let n = socket.read(&mut buffer).await.unwrap();
            if n == 0 {
                return;
            }
            bytes.extend_from_slice(&buffer[..n]);
            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length = headers
                    .lines()
                    .find_map(|s| {
                        s.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let _ = sent.send(String::from_utf8(bytes).unwrap());
        if let Some(gate) = gate {
            let _ = gate.await;
        }
        let _ = socket.write_all(&raw).await;
    });
    let mut client = RegistrationStart::new(timeout).unwrap();
    client.loopback = Some(Url::parse(&format!("http://{address}/")).unwrap());
    (client, received, handle)
}
async fn good(
    value: &Value,
) -> (
    RegistrationStart,
    oneshot::Receiver<String>,
    tokio::task::JoinHandle<()>,
) {
    server(
        wire(
            200,
            "Content-Type: application/json\r\n",
            value.to_string().as_bytes(),
        ),
        None,
        Duration::from_secs(5),
    )
    .await
}
fn body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}
fn is_challenge(value: StartOutcome) -> ValidatedRegistration {
    if let StartOutcome::Challenge(v) = value {
        v
    } else {
        panic!("expected pending challenge")
    }
}

#[tokio::test]
async fn real_post_has_exact_route_body_and_durable_signable_challenge() {
    let (_temp, registry, keys, backend) = setup().await;
    let value = pending(&registry, &keys).await;
    let (client, received, handle) = good(&value).await;
    let challenge = is_challenge(
        client
            .start(&registry, &keys, &scope(), "secret-invite")
            .await
            .unwrap(),
    );
    let request = received.await.unwrap();
    assert!(request.starts_with("POST /sub/api/v1/registration/start/ HTTP/1.1"));
    let lower = request
        .split_once("\r\n\r\n")
        .unwrap()
        .0
        .to_ascii_lowercase();
    assert!(!lower.contains("authorization:"));
    assert!(!lower.contains("cookie:"));
    let expected = registry
        .load_intent(&keys, &scope())
        .await
        .unwrap()
        .start_request("secret-invite")
        .unwrap();
    assert_eq!(
        body(&request),
        serde_json::from_slice::<Value>(expected.as_bytes()).unwrap()
    );
    assert_eq!(
        request.split_once("\r\n\r\n").unwrap().1.as_bytes(),
        expected.as_bytes()
    );
    let packet = keys
        .complete_packet(&scope(), &challenge, Utc::now())
        .await
        .unwrap();
    let p = registry.get(&scope()).await.unwrap();
    let public = p.public_identity().unwrap();
    assert!(verify_signature(
        challenge.statement_bytes(),
        public.identity_public_key(),
        packet["identity_signature"].as_str().unwrap(),
    )
    .unwrap());
    assert_eq!(
        client.progress(&registry, &scope()).await.unwrap().unwrap(),
        StartProgress {
            attempts: 1,
            has_context: true,
            last_failure: None
        }
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
    handle.await.unwrap();
}

#[tokio::test]
async fn close_reopen_reads_cached_context_without_invite_or_post() {
    let (temp, registry, keys, _) = setup().await;
    let value = pending(&registry, &keys).await;
    let (client, received, handle) = good(&value).await;
    let a = is_challenge(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap(),
    );
    received.await.unwrap();
    handle.await.unwrap();
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let (idle, mut no_post, unused) = good(&value).await;
    let b = is_challenge(
        idle.cached(&registry, &keys, &scope())
            .await
            .unwrap()
            .unwrap(),
    );
    assert_eq!(a.statement_bytes(), b.statement_bytes());
    is_challenge(
        idle.start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap(),
    );
    assert!(matches!(
        no_post.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    unused.abort();
}

#[tokio::test]
async fn lost_response_restart_explicit_retry_preserves_exact_request_and_keys() {
    let (temp, registry, keys, backend) = setup().await;
    let value = pending(&registry, &keys).await;
    let (_release, gate) = oneshot::channel();
    let (client, received, handle) = server(Vec::new(), Some(gate), Duration::from_secs(10)).await;
    let worker = {
        let r = registry.clone();
        let k = keys.clone();
        tokio::spawn(async move { client.start(&r, &k, &scope(), "same-invite").await })
    };
    let before = received.await.unwrap();
    worker.abort();
    let _ = worker.await;
    handle.abort();
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    let (client, received, handle) = good(&value).await;
    assert!(client
        .cached(&registry, &keys, &scope())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        client.progress(&registry, &scope()).await.unwrap().unwrap(),
        StartProgress {
            attempts: 1,
            has_context: false,
            last_failure: None
        }
    );
    client
        .start(&registry, &keys, &scope(), "same-invite")
        .await
        .unwrap();
    let after = received.await.unwrap();
    assert_eq!(body(&before), body(&after));
    assert_eq!(
        before.split_once("\r\n\r\n").unwrap().1,
        after.split_once("\r\n\r\n").unwrap().1
    );
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(
        client
            .progress(&registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        2
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn changed_invite_is_rejected_before_another_post() {
    let (_temp, registry, keys, _) = setup().await;
    let (client, received, handle) =
        server(wire(503, "", b"unavailable"), None, Duration::from_secs(5)).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "first-invite")
            .await
            .unwrap_err(),
        StartError::Http(503)
    );
    received.await.unwrap();
    handle.await.unwrap();
    let (client, mut received, handle) = good(&pending(&registry, &keys).await).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "different-invite")
            .await
            .unwrap_err(),
        StartError::Registry(RegistryError::Conflict)
    );
    assert!(matches!(
        received.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    handle.abort();
}

#[tokio::test]
async fn unvalidated_extra_response_fields_and_server_request_id_are_not_persisted() {
    let (temp, registry, keys, _) = setup().await;
    let mut value = pending(&registry, &keys).await;
    value["token"] = json!("echo-token-secret");
    value["request_id"] = json!("secret-invite");
    value["registration"]["extra"] = json!("secret-invite");
    let (client, received, handle) = good(&value).await;
    client
        .start(&registry, &keys, &scope(), "secret-invite")
        .await
        .unwrap();
    received.await.unwrap();
    handle.await.unwrap();
    for (_, bytes) in crate::community_identity::test_storage::bytes_after_close(registry, &temp.0).await {
        let text = String::from_utf8_lossy(&bytes);
        for forbidden in [
            "secret-invite",
            "echo-token-secret",
            "invite_token",
            "identity_pkcs8=",
            "operational_pkcs8=",
        ] {
            assert!(!text.contains(forbidden));
        }
    }
}

#[tokio::test]
async fn committed_start_requires_recovery_even_with_expired_original_challenge() {
    let (_temp, registry, keys, _) = setup().await;
    let value = response(
        &registry,
        &keys,
        "committed",
        Utc::now() - chrono::Duration::days(2),
    )
    .await;
    let (client, received, handle) = good(&value).await;
    assert!(matches!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap(),
        StartOutcome::RecoveryRequired { .. }
    ));
    received.await.unwrap();
    handle.await.unwrap();
    assert!(matches!(
        client
            .cached(&registry, &keys, &scope())
            .await
            .unwrap()
            .unwrap(),
        StartOutcome::RecoveryRequired { .. }
    ));
}

#[tokio::test]
async fn committed_response_still_checks_scope_before_marking_recovery() {
    let (_temp, registry, keys, _) = setup().await;
    let mut value = response(&registry, &keys, "committed", Utc::now()).await;
    value["registration"]["community_instance"] = json!("wrong-instance");
    let (client, received, handle) = good(&value).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Identity(IdentityError::ScopeMismatch)
    );
    received.await.unwrap();
    handle.await.unwrap();
    assert!(
        !client
            .progress(&registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .has_context
    );
}

#[tokio::test]
async fn committed_response_cannot_inject_a_statement_to_be_signed() {
    let (_temp, registry, keys, _) = setup().await;
    let mut value = response(&registry, &keys, "committed", Utc::now()).await;
    value["registration"]["statement"] = json!("sign arbitrary text");
    let (client, received, handle) = good(&value).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Identity(IdentityError::Invalid)
    );
    received.await.unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn pending_profile_hash_mismatch_is_not_saved() {
    let (_temp, registry, keys, _) = setup().await;
    let mut value = pending(&registry, &keys).await;
    value["registration"]["profile_sha256"] = json!("0".repeat(64));
    let (client, received, handle) = good(&value).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Identity(IdentityError::Invalid)
    );
    received.await.unwrap();
    handle.await.unwrap();
    assert!(client
        .cached(&registry, &keys, &scope())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn corrupted_saved_context_blocks_cache_and_resend() {
    let (_temp, registry, keys, _) = setup().await;
    let value = pending(&registry, &keys).await;
    let (client, received, handle) = good(&value).await;
    client
        .start(&registry, &keys, &scope(), "invite")
        .await
        .unwrap();
    received.await.unwrap();
    handle.await.unwrap();
    let mut corrupt = value;
    corrupt["registration"]["statement"] = json!("tampered");
    sqlx::query("UPDATE community_registration_starts SET response=?")
        .bind(corrupt.to_string())
        .execute(&registry.pool)
        .await
        .unwrap();
    assert_eq!(
        client.cached(&registry, &keys, &scope()).await.unwrap_err(),
        StartError::Identity(IdentityError::Invalid)
    );
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Identity(IdentityError::Invalid)
    );
    assert_eq!(
        client
            .progress(&registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
}

#[tokio::test]
async fn expired_cached_pending_context_is_not_resent_or_accepted() {
    let (_temp, registry, keys, _) = setup().await;
    let value = pending(&registry, &keys).await;
    let (client, received, handle) = good(&value).await;
    client
        .start(&registry, &keys, &scope(), "invite")
        .await
        .unwrap();
    received.await.unwrap();
    handle.await.unwrap();
    let expired = response(
        &registry,
        &keys,
        "pending",
        Utc::now() - chrono::Duration::days(1),
    )
    .await;
    sqlx::query("UPDATE community_registration_starts SET response=?")
        .bind(expired.to_string())
        .execute(&registry.pool)
        .await
        .unwrap();
    assert_eq!(
        client.cached(&registry, &keys, &scope()).await.unwrap_err(),
        StartError::Identity(IdentityError::Expired)
    );
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Identity(IdentityError::Expired)
    );
}

#[tokio::test]
async fn malformed_json_is_safe_and_preserves_uncertain_attempt() {
    let (_temp, registry, keys, _) = setup().await;
    let (client, received, handle) = server(
        wire(200, "Content-Type: application/json\r\n", b"{"),
        None,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::InvalidResponse
    );
    received.await.unwrap();
    handle.await.unwrap();
    assert_eq!(
        client.progress(&registry, &scope()).await.unwrap().unwrap(),
        StartProgress {
            attempts: 1,
            has_context: false,
            last_failure: Some("invalid_response".into())
        }
    );
}

#[tokio::test]
async fn redirects_are_not_followed_and_invite_is_not_forwarded() {
    let (_temp, registry, keys, _) = setup().await;
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let location = format!(
        "Location: http://{}/steal\r\n",
        target.local_addr().unwrap()
    );
    let (client, received, handle) =
        server(wire(307, &location, b""), None, Duration::from_secs(5)).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "secret-invite")
            .await
            .unwrap_err(),
        StartError::RedirectDenied
    );
    received.await.unwrap();
    handle.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), target.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rate_limits_have_only_bounded_numeric_retry_after() {
    let (_temp, registry, keys, _) = setup().await;
    for (value, expected) in [
        ("12", Some(12)),
        ("99999999999999999999999999", None),
        ("secret-invite", None),
    ] {
        let (client, received, handle) = server(
            wire(429, &format!("Retry-After: {value}\r\n"), b"echo-secret"),
            None,
            Duration::from_secs(5),
        )
        .await;
        assert_eq!(
            client
                .start(&registry, &keys, &scope(), "invite")
                .await
                .unwrap_err(),
            StartError::RateLimited {
                retry_after_seconds: expected
            }
        );
        received.await.unwrap();
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn content_length_limit_is_checked_before_reading_body() {
    let (_temp, registry, keys, _) = setup().await;
    let (client, received, handle) = server(
        wire(
            200,
            "Content-Type: application/json\r\nContent-Length: 20000\r\n",
            b"",
        ),
        None,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::ResponseTooLarge
    );
    received.await.unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn chunked_body_cannot_bypass_response_limit() {
    let (_temp, registry, keys, _) = setup().await;
    let body = format!("{:x}\r\n{}\r\n0\r\n\r\n", 20000, "x".repeat(20000));
    let (client, received, handle) = server(
        wire(
            200,
            "Content-Type: application/json\r\nTransfer-Encoding: chunked\r\n",
            body.as_bytes(),
        ),
        None,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::ResponseTooLarge
    );
    received.await.unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn wrong_content_type_is_not_a_challenge() {
    let (_temp, registry, keys, _) = setup().await;
    let (client, received, handle) = server(
        wire(
            200,
            "Content-Type: text/html\r\n",
            pending(&registry, &keys).await.to_string().as_bytes(),
        ),
        None,
        Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::InvalidResponse
    );
    received.await.unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn whole_http_deadline_includes_waiting_for_headers() {
    let (_temp, registry, keys, _) = setup().await;
    let (_release, gate) = oneshot::channel();
    let (client, _received, handle) =
        server(Vec::new(), Some(gate), Duration::from_millis(200)).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Timeout
    );
    handle.abort();
    assert_eq!(
        client
            .progress(&registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .last_failure
            .as_deref(),
        Some("timeout")
    );
}

#[tokio::test]
async fn missing_vault_keys_prevent_any_post_or_dispatch_marker() {
    let (_temp, registry, keys, backend) = setup().await;
    backend.data.lock().unwrap().clear();
    let (client, mut received, handle) = server(Vec::new(), None, Duration::from_secs(5)).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Registry(RegistryError::Identity(IdentityError::NotFound))
    );
    assert!(client
        .progress(&registry, &scope())
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        received.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    handle.abort();
}

#[tokio::test]
async fn durable_marker_failure_prevents_http() {
    let (_temp, registry, keys, _) = setup().await;
    sqlx::query("CREATE TRIGGER reject_start BEFORE INSERT ON community_registration_starts BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&registry.pool).await.unwrap();
    let (client, mut received, handle) = server(Vec::new(), None, Duration::from_secs(5)).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Registry(RegistryError::Storage)
    );
    assert!(matches!(
        received.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    handle.abort();
}

#[tokio::test]
async fn response_commit_failure_keeps_durable_unknown_state() {
    let (_temp, registry, keys, _) = setup().await;
    sqlx::query("CREATE TRIGGER reject_response BEFORE UPDATE OF response ON community_registration_starts BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&registry.pool).await.unwrap();
    let (client, received, handle) = good(&pending(&registry, &keys).await).await;
    assert_eq!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap_err(),
        StartError::Registry(RegistryError::Storage)
    );
    received.await.unwrap();
    handle.await.unwrap();
    assert!(client
        .cached(&registry, &keys, &scope())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        client
            .progress(&registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
}

#[tokio::test]
async fn late_pending_response_cannot_overwrite_newer_committed_context() {
    let (_temp, registry, keys, _) = setup().await;
    let value = pending(&registry, &keys).await;
    let (release, gate) = oneshot::channel();
    let (first, received, handle) = server(
        wire(
            200,
            "Content-Type: application/json\r\n",
            value.to_string().as_bytes(),
        ),
        Some(gate),
        Duration::from_secs(10),
    )
    .await;
    let worker = {
        let r = registry.clone();
        let k = keys.clone();
        tokio::spawn(async move { first.start(&r, &k, &scope(), "invite").await })
    };
    received.await.unwrap();
    let mut committed = value;
    committed["registration"]["state"] = json!("committed");
    let (client, received, second) = good(&committed).await;
    assert!(matches!(
        client
            .start(&registry, &keys, &scope(), "invite")
            .await
            .unwrap(),
        StartOutcome::RecoveryRequired { .. }
    ));
    received.await.unwrap();
    second.await.unwrap();
    release.send(()).unwrap();
    assert_eq!(
        worker.await.unwrap().unwrap_err(),
        StartError::Registry(RegistryError::Conflict)
    );
    handle.await.unwrap();
    assert!(matches!(
        client
            .cached(&registry, &keys, &scope())
            .await
            .unwrap()
            .unwrap(),
        StartOutcome::RecoveryRequired { .. }
    ));
}

#[tokio::test]
async fn late_failure_does_not_clobber_newer_success() {
    let (_temp, registry, keys, _) = setup().await;
    let value = pending(&registry, &keys).await;
    let (release, gate) = oneshot::channel();
    let (first, received, handle) = server(
        wire(500, "", b"echo-secret"),
        Some(gate),
        Duration::from_secs(10),
    )
    .await;
    let worker = {
        let r = registry.clone();
        let k = keys.clone();
        tokio::spawn(async move { first.start(&r, &k, &scope(), "invite").await })
    };
    received.await.unwrap();
    let (client, received, second) = good(&value).await;
    client
        .start(&registry, &keys, &scope(), "invite")
        .await
        .unwrap();
    received.await.unwrap();
    second.await.unwrap();
    release.send(()).unwrap();
    assert_eq!(worker.await.unwrap().unwrap_err(), StartError::Http(500));
    handle.await.unwrap();
    assert_eq!(
        client.progress(&registry, &scope()).await.unwrap().unwrap(),
        StartProgress {
            attempts: 2,
            has_context: true,
            last_failure: None
        }
    );
}

#[tokio::test]
async fn registry_v1_migration_preserves_preparation_and_never_posts() {
    let (temp, registry, keys, backend) = setup().await;
    let before = registry.get(&scope()).await.unwrap();
    sqlx::query("DROP TABLE community_profile_lifecycle").execute(&registry.pool).await.unwrap();
    // Recreate the exact v1 table inventory; preparation DDL is unchanged.
    sqlx::query("DROP TABLE community_registration_dispatches").execute(&registry.pool).await.unwrap();
    sqlx::query("DROP TABLE community_registration_finishes").execute(&registry.pool).await.unwrap();
    sqlx::query("DROP TABLE community_registration_starts")
        .execute(&registry.pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=1")
        .execute(&registry.pool)
        .await
        .unwrap();
    registry.close().await;
    let registry = RegistrationRegistry::open(&temp.path()).await.unwrap();
    assert_eq!(registry.get(&scope()).await.unwrap(), before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&registry.pool)
            .await
            .unwrap(),
        4
    );
    let client = RegistrationStart::new(Duration::from_secs(5)).unwrap();
    assert!(client
        .progress(&registry, &scope())
        .await
        .unwrap()
        .is_none());
    assert!(client
        .cached(&registry, &keys, &scope())
        .await
        .unwrap()
        .is_none());
    assert_eq!(backend.creates.load(Ordering::SeqCst), 1);
}

#[test]
fn timeout_configuration_is_bounded() {
    for t in [Duration::ZERO, Duration::from_secs(31)] {
        assert!(matches!(
            RegistrationStart::new(t),
            Err(StartError::InvalidConfiguration)
        ));
    }
}
