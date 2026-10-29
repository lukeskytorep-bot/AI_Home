use super::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};
#[derive(Default)]
struct Memory {
    data: Mutex<HashMap<String, Zeroizing<String>>>,
    creates: AtomicUsize,
    fail_write_readback: AtomicBool,
    fail_read: AtomicBool,
}
impl CommunitySecretStore for Memory {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        if self.fail_read.swap(false, Ordering::SeqCst) {
            return Err(IdentityError::VaultUnavailable);
        }
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, s: &str) -> Result<(), IdentityError> {
        let mut d = self.data.lock().unwrap();
        if d.contains_key(r) {
            return Err(IdentityError::AlreadyExists);
        }
        d.insert(r.into(), Zeroizing::new(s.into()));
        self.creates.fetch_add(1, Ordering::SeqCst);
        if self.fail_write_readback.swap(false, Ordering::SeqCst) {
            self.fail_read.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.data.lock().unwrap().remove(r);
        Ok(())
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
fn reg() -> String {
    format!("reg_{}", "1".repeat(32))
}
fn agent() -> String {
    format!("agt_{}", "2".repeat(32))
}
fn key() -> String {
    format!("key_{}", "3".repeat(32))
}
fn token(g: i64) -> String {
    format!("eqai_{}", super::super::super::crypto::b64(&[g as u8; 32]))
}
fn envelope(r: Value) -> Value {
    json!({"ok":true,"request_id":"mock-request","registration":r})
}
fn result(g: i64, retry: bool) -> Value {
    if retry {
        envelope(
            json!({"registration_id":reg(),"state":"committed","agent_id":agent(),"token":null,"token_reissue_required":true,"retry":true}),
        )
    } else {
        envelope(
            json!({"registration_id":reg(),"state":"committed","agent_id":agent(),"token":token(g),"token_generation":g,"token_reissue_required":false,"retry":false}),
        )
    }
}
/// Independent serializer follows PHP registration-contract.php, not Proof::bytes.
fn message(c: &Value, purpose: &str) -> String {
    let route = format!(
        "/api/v1/registration/{}{}",
        reg(),
        if purpose == "status" {
            "/status"
        } else {
            "/token/reissue"
        }
    );
    let body = if purpose == "status" {
        String::new()
    } else {
        format!(
            "expected_token_generation={}\n",
            c["expected_token_generation"].as_i64().unwrap()
        )
    };
    let format = if purpose == "status" {
        "AGENT-REGISTRATION-STATUS-PROOF/1"
    } else {
        "AGENT-REGISTRATION-REISSUE-PROOF/1"
    };
    let mut s = format!("{format}\n");
    for k in [
        "community_origin",
        "community_instance",
        "registration_id",
        "agent_id",
        "operational_key_id",
    ] {
        s.push_str(&format!("{k}={}\n", c[k].as_str().unwrap()));
    }
    s.push_str(&format!(
        "purpose={purpose}\nmethod={}\nroute={route}\n",
        if purpose == "status" { "GET" } else { "POST" }
    ));
    for k in ["challenge_id", "nonce", "expires_at"] {
        s.push_str(&format!("{k}={}\n", c[k].as_str().unwrap()));
    }
    s.push_str(&format!("body_sha256={}\nsignature_algorithm={ALGORITHM}\npublic_key_format={PUBLIC_FORMAT}\nsignature_format={SIGNATURE_FORMAT}\n",super::super::super::crypto::digest(body.as_bytes())));
    s
}
#[derive(Clone, Copy)]
enum Fault {
    Drop,
    WrongReg,
    BadToken,
    BadGeneration,
    WrongScope,
    Expired,
    WrongPurpose,
    WrongKey,
    StatusRollback,
    NewGeneration,
    Replay,
}
struct Mock {
    start_context:Value,
    committed: AtomicBool,
    generation: AtomicI64,
    public: PublicIdentity,
    statement: Vec<u8>,
    requests: Mutex<Vec<(String, String, String)>>,
    challenges: Mutex<HashMap<String, Value>>,
    faults: Mutex<HashMap<String, Fault>>,
    hold: Mutex<Option<String>>,
    processed: Notify,
    release: Notify,
}
impl Mock {
    fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, p, _)| p.contains(path))
            .count()
    }
    fn fault(&self, action: &str, fault: Fault) {
        self.faults.lock().unwrap().insert(action.into(), fault);
    }
    fn handle(
        &self,
        method: &str,
        path: &str,
        headers: &HashMap<String, String>,
        body: &str,
    ) -> Option<Value> {
        self.requests
            .lock()
            .unwrap()
            .push((method.into(), path.into(), body.into()));
        let b: Value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(body).unwrap()
        };
        let (action, mut value) = if path == "/sub/api/v1/registration/start/" {
            assert_eq!(method,"POST");
            assert!(b["invite_token"].as_str().is_some_and(|s|!s.is_empty()));
            ("start",self.start_context.clone())
        } else if path == "/sub/api/v1/registration/complete/" {
            assert_eq!(method, "POST");
            assert_eq!(b["registration_id"], reg());
            assert!(verify_signature(
                &self.statement,
                self.public.identity_public_key(),
                b["identity_signature"].as_str().unwrap()
            )
            .unwrap());
            assert!(verify_signature(
                &self.statement,
                self.public.operational_public_key(),
                b["operational_signature"].as_str().unwrap()
            )
            .unwrap());
            let retry = self.committed.swap(true, Ordering::SeqCst);
            if !retry {
                self.generation.store(1, Ordering::SeqCst);
            }
            ("complete", result(1, retry))
        } else if path == "/sub/api/v1/registration/proof-challenge/" {
            assert_eq!(method, "POST");
            assert!(self.committed.load(Ordering::SeqCst));
            assert_eq!(b["registration_id"], reg());
            let purpose = b["purpose"].as_str().unwrap();
            let id = format!("chl_{}", uuid::Uuid::new_v4().simple());
            let c = json!({"challenge_id":id,"registration_id":reg(),"purpose":purpose,"nonce":super::super::super::crypto::b64(&[7;32]),"expires_at":(Utc::now()+chrono::Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Secs,true),"agent_id":agent(),"operational_key_id":key(),"expected_token_generation":if purpose=="status"{Value::Null}else{json!(self.generation.load(Ordering::SeqCst))},"community_origin":"https://example.test","community_instance":"test-community"});
            self.challenges.lock().unwrap().insert(id, c.clone());
            (
                if purpose == "status" {
                    "challenge.status"
                } else {
                    "challenge.reissue"
                },
                json!({"ok":true,"request_id":"mock-request","challenge":c}),
            )
        } else if path == format!("/sub/api/v1/registration/status/?registration_id={}", reg()) {
            assert_eq!(method, "GET");
            assert!(body.is_empty());
            let id = &headers["x-community-challenge-id"];
            let c = self.challenges.lock().unwrap().remove(id).unwrap();
            assert!(verify_signature(
                message(&c, "status").as_bytes(),
                self.public.operational_public_key(),
                &headers["x-community-signature"]
            )
            .unwrap());
            (
                "status",
                envelope(
                    json!({"registration_id":reg(),"state":"committed","agent_id":agent(),"token_generation":self.generation.load(Ordering::SeqCst),"token":null}),
                ),
            )
        } else if path == "/sub/api/v1/registration/token/reissue/" {
            assert_eq!(method, "POST");
            assert_eq!(b["registration_id"], reg());
            let c = self
                .challenges
                .lock()
                .unwrap()
                .remove(b["challenge_id"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                b["expected_token_generation"],
                c["expected_token_generation"]
            );
            assert_eq!(
                b["expected_token_generation"].as_i64().unwrap(),
                self.generation.load(Ordering::SeqCst)
            );
            assert!(verify_signature(
                message(&c, "reissue").as_bytes(),
                self.public.operational_public_key(),
                b["signature"].as_str().unwrap()
            )
            .unwrap());
            let g = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            ("reissue", result(g, false))
        } else {
            panic!("unexpected local test route {path}")
        };
        assert!(!headers.contains_key("authorization"));
        assert!(!headers.contains_key("cookie"));
        if let Some(fault) = self.faults.lock().unwrap().remove(action) {
            match fault {
                Fault::Drop => return None,
                Fault::WrongReg => {
                    value["registration"]["registration_id"] =
                        json!(format!("reg_{}", "9".repeat(32)))
                }
                Fault::BadToken => value["registration"]["token"] = json!("not-a-token"),
                Fault::BadGeneration => value["registration"]["token_generation"] = json!(99),
                Fault::Replay => value = result(0, true),
                Fault::WrongScope => {
                    value["challenge"]["community_instance"] = json!("wrong-instance")
                }
                Fault::Expired => {
                    value["challenge"]["expires_at"] = json!((Utc::now()
                        - chrono::Duration::minutes(1))
                    .to_rfc3339_opts(SecondsFormat::Secs, true))
                }
                Fault::WrongPurpose => value["challenge"]["purpose"] = json!("wrong"),
                Fault::WrongKey => {
                    value["challenge"]["operational_key_id"] =
                        json!(format!("key_{}", "8".repeat(32)))
                }
                Fault::StatusRollback => value["registration"]["token_generation"] = json!(1),
                Fault::NewGeneration => {
                    value["challenge"]["expected_token_generation"] =
                        json!(self.generation.load(Ordering::SeqCst) + 1)
                }
            }
        }
        Some(value)
    }
}
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Server {
    handle: tokio::task::JoinHandle<()>,
    url: reqwest::Url,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
async fn server(mock: Arc<Mock>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let handle = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mock = mock.clone();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let (end, length) = loop {
                    let mut chunk = [0u8; 2048];
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        let text = String::from_utf8_lossy(&bytes[..end]);
                        let length = text
                            .lines()
                            .find_map(|s| {
                                s.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|s| s.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break (end, length);
                        }
                    }
                };
                let raw = String::from_utf8_lossy(&bytes[..end]);
                let mut lines = raw.lines();
                let parts: Vec<_> = lines.next().unwrap().split_whitespace().collect();
                let method = parts[0];
                let path = parts[1];
                let headers: HashMap<String, String> = lines
                    .filter_map(|s| {
                        s.split_once(':')
                            .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().into()))
                    })
                    .collect();
                let body = String::from_utf8(bytes[end + 4..end + 4 + length].to_vec()).unwrap();
                let response = mock.handle(method, path, &headers, &body);
                let hold = mock.hold.lock().unwrap().take();
                if hold.as_deref().is_some_and(|s| path.contains(s)) {
                    mock.processed.notify_one();
                    mock.release.notified().await;
                } else if let Some(hold) = hold {
                    *mock.hold.lock().unwrap() = Some(hold);
                }
                if let Some(response) = response {
                    let body = response.to_string();
                    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            });
        }
    });
    Server { handle, url }
}
struct Setup {
    temp: Temp,
    registry: RegistrationRegistry,
    keys: CommunityKeyStore,
    key_backend: Arc<Memory>,
    tokens: CommunityTokenStore,
    token_backend: Arc<Memory>,
    finish: RegistrationFinish,
    mock: Arc<Mock>,
    _server: Server,
}
async fn setup(committed: bool) -> Setup {
    setup_at(committed,"registry.sqlite").await
}
async fn setup_at(committed: bool,filename:&str) -> Setup {
    let temp = Temp(std::env::temp_dir().join(format!("ai-home-finish-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&temp.0).unwrap();
    let registry = RegistrationRegistry::open(&temp.0.join(filename))
        .await
        .unwrap();
    let key_backend = Arc::new(Memory::default());
    let keys = CommunityKeyStore::with_backend(key_backend.clone());
    registry
        .prepare(
            &scope(),
            &RegistrationProfile::new("Agent", "public").unwrap(),
            "original-start-0123456789",
        )
        .await
        .unwrap();
    let ready = registry.provision(&keys, &scope(), 1).await.unwrap();
    let intent = registry.load_intent(&keys, &scope()).await.unwrap();
    let nonce = super::super::super::crypto::b64(&[7; 32]);
    let expiry =
        (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let statement = super::super::super::registration_statement(
        &scope(),
        &intent.identity,
        &intent.profile,
        &reg(),
        &nonce,
        &expiry,
    );
    let context = json!({"ok":true,"request_id":"test","registration":{"registration_id":reg(),"state":if committed{"committed"}else{"pending"},"nonce":nonce,"expires_at":expiry,"community_origin":"https://example.test","community_instance":"test-community","profile_format":"COMMUNITY-PROFILE/1","profile_sha256":intent.profile.sha256(),"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}});
    sqlx::query("INSERT INTO community_registration_starts(scope_binding,request_sha256,attempts,response) VALUES(?,?,1,?)").bind(scope().binding()).bind("0".repeat(64)).bind(context.to_string()).execute(&registry.pool).await.unwrap();
    let mock = Arc::new(Mock {
        start_context:context.clone(),
        committed: AtomicBool::new(committed),
        generation: AtomicI64::new(if committed { 1 } else { 0 }),
        public: ready.public_identity().unwrap().clone(),
        statement: statement.into_bytes(),
        requests: Mutex::new(Vec::new()),
        challenges: Mutex::new(HashMap::new()),
        faults: Mutex::new(HashMap::new()),
        hold: Mutex::new(None),
        processed: Notify::new(),
        release: Notify::new(),
    });
    let server = server(mock.clone()).await;
    let token_backend = Arc::new(Memory::default());
    let tokens = CommunityTokenStore::with_backend(token_backend.clone());
    let mut finish = RegistrationFinish::new(Duration::from_secs(10), tokens.clone()).unwrap();
    finish.http.set_loopback(server.url.clone());
    Setup {
        temp,
        registry,
        keys,
        key_backend,
        tokens,
        token_backend,
        finish,
        mock,
        _server: server,
    }
}

#[path = "tests/host_tests.rs"]
mod host_tests;
async fn begin_recovery(s: &Setup) -> FinishProgress {
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::RecoveryRequired
    );
    s.finish
        .recover(&s.registry, &s.keys, &scope(), 1)
        .await
        .unwrap()
}

#[tokio::test]
async fn complete_persists_packet_and_token_before_reporting_ready() {
    let s = setup(false).await;
    let r = s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .unwrap();
    assert_eq!(r.generation, Some(1));
    assert!(r.ready_metadata);
    assert_eq!(
        s.finish
            .credential(&s.registry, &s.keys, &scope())
            .await
            .unwrap()
            .as_str(),
        token(1)
    );
    assert_eq!(s.mock.count("/complete/"), 1);
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap(),
        r
    );
    assert_eq!(s.mock.count("/complete/"), 1);
}
#[tokio::test]
async fn token_survives_real_database_close_reopen_without_http() {
    let s = setup(false).await;
    let a = s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .unwrap();
    let path = s.temp.0.join("registry.sqlite");
    s.registry.clone().close().await;
    let registry = RegistrationRegistry::open(&path).await.unwrap();
    assert_eq!(
        s.finish
            .current(&registry, &s.keys, &scope())
            .await
            .unwrap(),
        a
    );
    assert_eq!(s.mock.count("/complete/"), 1);
}
#[tokio::test]
async fn committed_start_runs_full_status_and_reissue_protocol() {
    let s = setup(true).await;
    let r = begin_recovery(&s).await;
    assert_eq!(r.generation, Some(2));
    assert_eq!(s.mock.count("/complete/"), 0);
    assert_eq!(s.mock.count("/proof-challenge/"), 2);
    assert_eq!(s.mock.count("/status/?"), 1);
    assert_eq!(s.mock.count("/token/reissue/"), 1);
    assert_eq!(
        s.finish
            .credential(&s.registry, &s.keys, &scope())
            .await
            .unwrap()
            .as_str(),
        token(2)
    );
}
#[tokio::test]
async fn lost_complete_response_replays_exact_packet_then_recovers_lost_token() {
    let s = setup(false).await;
    s.mock.fault("complete", Fault::Drop);
    assert!(matches!(
        s.finish.complete(&s.registry, &s.keys, &scope()).await,
        Err(FinishError::Http(_))
    ));
    assert_eq!(
        s.finish
            .retry_complete(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap_err(),
        FinishError::RecoveryRequired
    );
    let r = s
        .finish
        .recover(&s.registry, &s.keys, &scope(), 2)
        .await
        .unwrap();
    assert_eq!(r.generation, Some(2));
    let requests = s.mock.requests.lock().unwrap();
    let packets: Vec<_> = requests
        .iter()
        .filter(|(_, p, _)| p.contains("/complete/"))
        .map(|(_, _, b)| b)
        .collect();
    assert_eq!(packets[0], packets[1]);
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn lost_reissue_response_requires_fresh_proofs_and_new_generation() {
    let s = setup(true).await;
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::RecoveryRequired
    );
    s.mock.fault("reissue", Fault::Drop);
    assert!(matches!(
        s.finish.recover(&s.registry, &s.keys, &scope(), 1).await,
        Err(FinishError::Http(_))
    ));
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 2)
            .await
            .unwrap()
            .generation,
        Some(3)
    );
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn readback_failure_leaves_recoverable_candidate_not_false_ready() {
    let s = setup(false).await;
    s.token_backend
        .fail_write_readback
        .store(true, Ordering::SeqCst);
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::Identity(IdentityError::VaultUnavailable)
    );
    assert!(
        !s.finish
            .progress(&s.registry, &scope())
            .await
            .unwrap()
            .unwrap()
            .ready_metadata
    );
    let r = s
        .finish
        .recover(&s.registry, &s.keys, &scope(), 1)
        .await
        .unwrap();
    assert_eq!(r.generation, Some(2));
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn database_failure_after_vault_write_does_not_lose_recovery_reference() {
    let s = setup(false).await;
    sqlx::query("CREATE TRIGGER reject_ready BEFORE UPDATE ON community_registration_finishes WHEN json_extract(NEW.body,'$.state')='ready' BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&s.registry.pool).await.unwrap();
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::Registry(RegistryError::Storage)
    );
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM community_registration_dispatches WHERE token_ref IS NOT NULL"
        )
        .fetch_one(&s.registry.pool)
        .await
        .unwrap(),
        1
    );
    sqlx::query("DROP TRIGGER reject_ready")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap()
            .generation,
        Some(2)
    );
}
#[tokio::test]
async fn durable_dispatch_failure_prevents_complete_post() {
    let s = setup(false).await;
    sqlx::query("CREATE TRIGGER reject_dispatch BEFORE INSERT ON community_registration_dispatches BEGIN SELECT RAISE(ABORT,'fixture'); END").execute(&s.registry.pool).await.unwrap();
    assert_eq!(
        s.finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::Registry(RegistryError::Storage)
    );
    assert_eq!(s.mock.count("/complete/"), 0);
    sqlx::query("DROP TRIGGER reject_dispatch")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    assert_eq!(
        s.finish
            .retry_complete(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap()
            .generation,
        Some(1)
    );
}
#[tokio::test]
async fn stale_revision_does_not_dispatch_recovery() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 0)
            .await
            .unwrap_err(),
        FinishError::Registry(RegistryError::Conflict)
    );
    assert!(s.mock.requests.lock().unwrap().is_empty());
}
#[tokio::test]
async fn late_old_token_cannot_replace_newer_recovery_token() {
    let s = setup(false).await;
    *s.mock.hold.lock().unwrap() = Some("/complete/".into());
    let worker = {
        let f = s.finish.clone();
        let r = s.registry.clone();
        let k = s.keys.clone();
        tokio::spawn(async move { f.complete(&r, &k, &scope()).await })
    };
    tokio::time::timeout(Duration::from_secs(5), s.mock.processed.notified())
        .await
        .unwrap();
    let current = s
        .finish
        .recover(&s.registry, &s.keys, &scope(), 1)
        .await
        .unwrap();
    assert_eq!(current.generation, Some(2));
    s.mock.release.notify_one();
    assert_eq!(
        worker.await.unwrap().unwrap_err(),
        FinishError::Registry(RegistryError::Conflict)
    );
    assert_eq!(
        s.finish
            .current(&s.registry, &s.keys, &scope())
            .await
            .unwrap(),
        current
    );
    assert_eq!(
        s.finish
            .credential(&s.registry, &s.keys, &scope())
            .await
            .unwrap()
            .as_str(),
        token(2)
    );
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn cancelled_complete_can_be_recovered_without_new_keys() {
    let s = setup(false).await;
    *s.mock.hold.lock().unwrap() = Some("/complete/".into());
    let worker = {
        let f = s.finish.clone();
        let r = s.registry.clone();
        let k = s.keys.clone();
        tokio::spawn(async move { f.complete(&r, &k, &scope()).await })
    };
    tokio::time::timeout(Duration::from_secs(5), s.mock.processed.notified())
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    s.mock.release.notify_one();
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap()
            .generation,
        Some(2)
    );
    assert_eq!(s.key_backend.creates.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn malformed_or_mismatched_token_receipts_are_not_accepted() {
    for fault in [Fault::WrongReg, Fault::BadToken, Fault::BadGeneration] {
        let s = setup(false).await;
        s.mock.fault("complete", fault);
        assert!(s
            .finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .is_err());
        assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 0);
        assert!(
            !s.finish
                .progress(&s.registry, &scope())
                .await
                .unwrap()
                .unwrap()
                .ready_metadata
        );
    }
}
#[tokio::test]
async fn wrong_challenge_scope_or_purpose_or_expiry_stops_before_signed_request() {
    for fault in [Fault::WrongScope, Fault::WrongPurpose, Fault::Expired] {
        let s = setup(true).await;
        assert!(s
            .finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .is_err());
        s.mock.fault("challenge.status", fault);
        assert!(matches!(
            s.finish.recover(&s.registry, &s.keys, &scope(), 1).await,
            Err(FinishError::Identity(_))
        ));
        assert_eq!(s.mock.count("/status/?"), 0);
        assert_eq!(s.mock.count("/token/reissue/"), 0);
    }
}
#[tokio::test]
async fn changed_operational_key_id_between_proofs_is_rejected() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    s.mock.fault("challenge.reissue", Fault::WrongKey);
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap_err(),
        FinishError::Identity(IdentityError::ScopeMismatch)
    );
    assert_eq!(s.mock.count("/token/reissue/"), 0);
}
#[tokio::test]
async fn token_generation_rollback_is_rejected_without_reissue() {
    let s = setup(true).await;
    let r = begin_recovery(&s).await;
    s.mock.fault("status", Fault::StatusRollback);
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), r.revision)
            .await
            .unwrap_err(),
        FinishError::InvalidResponse
    );
    assert_eq!(s.mock.count("/token/reissue/"), 1);
}
#[tokio::test]
async fn missing_local_token_is_not_ready_and_explicit_recovery_restores_access() {
    let s = setup(false).await;
    let r = s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .unwrap();
    s.token_backend.data.lock().unwrap().clear();
    assert_eq!(
        s.finish
            .current(&s.registry, &s.keys, &scope())
            .await
            .unwrap_err(),
        FinishError::Identity(IdentityError::NotFound)
    );
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), r.revision)
            .await
            .unwrap()
            .generation,
        Some(2)
    );
}
#[tokio::test]
async fn missing_identity_keys_block_all_complete_http() {
    let s = setup(false).await;
    s.key_backend.data.lock().unwrap().clear();
    assert!(matches!(
        s.finish.complete(&s.registry, &s.keys, &scope()).await,
        Err(FinishError::Registry(RegistryError::Identity(
            IdentityError::NotFound
        )))
    ));
    assert!(s.mock.requests.lock().unwrap().is_empty());
}
#[tokio::test]
async fn persisted_complete_packet_tampering_cannot_be_replayed() {
    let s = setup(false).await;
    s.mock.fault("complete", Fault::Drop);
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    let mut r = read(&s.registry, &scope()).await.unwrap().unwrap();
    r.body["completeBody"]=json!(json!({"registration_id":reg(),"identity_signature":"tampered","operational_signature":"tampered"}).to_string());
    update(&s.registry, &scope(), &r).await.unwrap();
    assert!(s
        .finish
        .retry_complete(&s.registry, &s.keys, &scope(), 1)
        .await
        .is_err());
    assert_eq!(s.mock.count("/complete/"), 1);
}
#[tokio::test]
async fn sqlite_and_public_progress_do_not_contain_bearer_tokens() {
    let s = setup(false).await;
    let r = s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .unwrap();
    assert!(!format!("{r:?}").contains(&token(1)));
    for (_, bytes) in crate::community_identity::test_storage::bytes_after_close(s.registry, &s.temp.0).await {
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(&token(1)));
        assert!(!text.contains("identity_pkcs8="));
        assert!(!text.contains("token=eqai_"));
    }
}
#[tokio::test]
async fn immutable_token_candidates_are_scoped_and_removable() {
    let s = setup(false).await;
    let reference = CommunityTokenStore::reserve();
    s.tokens
        .save(
            &scope(),
            &reference,
            &reg(),
            &agent(),
            1,
            SecretToken::new(token(1)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        s.tokens
            .save(
                &scope(),
                &reference,
                &reg(),
                &agent(),
                2,
                SecretToken::new(token(2)).unwrap()
            )
            .await
            .unwrap_err(),
        IdentityError::AlreadyExists
    );
    assert_eq!(
        s.tokens
            .load(&scope(), &reference, &reg(), &agent(), 1)
            .await
            .unwrap()
            .as_str(),
        token(1)
    );
    assert!(s
        .tokens
        .load(&scope(), &reference, &reg(), &agent(), 2)
        .await
        .is_err());
    let other = CommunityScope::new(
        "profile-b",
        "https://example.test/sub/api/v1/",
        "https://example.test",
        "test-community",
    )
    .unwrap();
    assert!(s
        .tokens
        .load(&other, &reference, &reg(), &agent(), 1)
        .await
        .is_err());
    s.tokens
        .remove(&scope(), &reference, &reg(), &agent(), 1)
        .await
        .unwrap();
    s.tokens
        .remove(&scope(), &reference, &reg(), &agent(), 1)
        .await
        .unwrap();
}
#[tokio::test]
async fn token_store_never_accepts_provider_or_identity_references() {
    let s = setup(false).await;
    for reference in [
        "provider-api-key",
        "ai-home-community-keyset:v1:00000000000000000000000000000000",
    ] {
        assert!(matches!(
            s.tokens
                .load(&scope(), reference, &reg(), &agent(), 1)
                .await,
            Err(IdentityError::Invalid)
        ));
    }
}
#[test]
fn token_wire_format_is_exact_and_canonical() {
    for text in [
        "eqai_short".to_string(),
        format!("eqai_{}=", "A".repeat(43)),
        format!("wrong_{}", "A".repeat(43)),
        format!("eqai_{}!", "A".repeat(42)),
    ] {
        assert!(SecretToken::new(text).is_err());
    }
    assert!(SecretToken::new(token(7)).is_ok());
}
#[tokio::test]
async fn registry_v2_migration_preserves_start_and_never_sends_http() {
    let s = setup(false).await;
    let before = s.registry.get(&scope()).await.unwrap();
    sqlx::query("DROP TABLE community_profile_lifecycle").execute(&s.registry.pool).await.unwrap();
    sqlx::query("DROP TABLE community_registration_dispatches")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE community_registration_finishes")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version=2")
        .execute(&s.registry.pool)
        .await
        .unwrap();
    s.registry.clone().close().await;
    let registry = RegistrationRegistry::open(&s.temp.0.join("registry.sqlite"))
        .await
        .unwrap();
    assert_eq!(registry.get(&scope()).await.unwrap(), before);
    assert!(s
        .finish
        .progress(&registry, &scope())
        .await
        .unwrap()
        .is_none());
    assert!(s.mock.requests.lock().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&registry.pool)
            .await
            .unwrap(),
        4
    );
}
#[tokio::test]
async fn proofs_match_independent_python_contract_fixtures_and_signatures() {
    let s = setup(true).await;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/community-completion");
    let fields: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("fields.json")).unwrap()).unwrap();
    let now = DateTime::parse_from_rfc3339("2026-10-27T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    for purpose in ["status", "reissue"] {
        let mut c = fields.clone();
        c["purpose"] = json!(purpose);
        c["expected_token_generation"] = if purpose == "status" {
            Value::Null
        } else {
            json!(7)
        };
        let response = json!({"ok":true,"request_id":"fixture","challenge":c});
        let proof = Proof::validate(&scope(), &reg(), purpose, &response, None, None, now).unwrap();
        assert_eq!(
            proof.bytes(),
            std::fs::read(root.join(format!("{purpose}.txt"))).unwrap()
        );
        assert_eq!(
            proof.bytes(),
            message(&response["challenge"], purpose).as_bytes()
        );
        // Signing uses a fresh expiry for the runtime clock; fixture bytes above stay fixed.
        let mut fresh = response.clone();
        fresh["challenge"]["expires_at"] =
            json!((Utc::now() + chrono::Duration::minutes(10))
                .to_rfc3339_opts(SecondsFormat::Secs, true));
        let proof =
            Proof::validate(&scope(), &reg(), purpose, &fresh, None, None, Utc::now()).unwrap();
        let signature = s
            .keys
            .proof_signature(&scope(), &s.mock.public, &proof)
            .await
            .unwrap();
        assert!(verify_signature(
            proof.bytes(),
            s.mock.public.operational_public_key(),
            &signature
        )
        .unwrap());
        if let Ok(out) = std::env::var("AI_HOME_COMPLETION_PUBLIC_INTEROP_DIR") {
            std::fs::create_dir_all(&out).unwrap();
            std::fs::write(std::path::Path::new(&out).join(format!("{purpose}.json")),json!({"message":String::from_utf8(proof.bytes().to_vec()).unwrap(),"publicKey":s.mock.public.operational_public_key(),"signature":signature}).to_string()).unwrap();
        }
    }
}

#[tokio::test]
async fn generation_change_between_status_and_challenge_stops_without_reissue() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    s.mock.fault("challenge.reissue", Fault::NewGeneration);
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap_err(),
        FinishError::Registry(RegistryError::Conflict)
    );
    assert_eq!(s.mock.count("/token/reissue/"), 0);
}
#[tokio::test]
async fn late_reissue_candidate_cannot_replace_a_newer_generation() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    *s.mock.hold.lock().unwrap() = Some("/token/reissue/".into());
    let worker = {
        let f = s.finish.clone();
        let r = s.registry.clone();
        let k = s.keys.clone();
        tokio::spawn(async move { f.recover(&r, &k, &scope(), 1).await })
    };
    tokio::time::timeout(Duration::from_secs(5), s.mock.processed.notified())
        .await
        .unwrap();
    let current = s
        .finish
        .recover(&s.registry, &s.keys, &scope(), 2)
        .await
        .unwrap();
    assert_eq!(current.generation, Some(3));
    s.mock.release.notify_one();
    assert_eq!(
        worker.await.unwrap().unwrap_err(),
        FinishError::Registry(RegistryError::Conflict)
    );
    assert_eq!(
        s.finish
            .current(&s.registry, &s.keys, &scope())
            .await
            .unwrap(),
        current
    );
    assert_eq!(
        s.finish
            .credential(&s.registry, &s.keys, &scope())
            .await
            .unwrap()
            .as_str(),
        token(3)
    );
}
#[tokio::test]
async fn status_cannot_return_a_token_instead_of_public_metadata() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    s.mock.fault("status", Fault::BadToken);
    assert_eq!(
        s.finish
            .recover(&s.registry, &s.keys, &scope(), 1)
            .await
            .unwrap_err(),
        FinishError::InvalidResponse
    );
    assert_eq!(s.mock.count("/token/reissue/"), 0);
    assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn malformed_reissue_receipt_remains_recoverable() {
    for (fault, expected) in [
        (Fault::BadGeneration, FinishError::InvalidResponse),
        (Fault::Replay, FinishError::RecoveryRequired),
    ] {
        let s = setup(true).await;
        assert!(s
            .finish
            .complete(&s.registry, &s.keys, &scope())
            .await
            .is_err());
        s.mock.fault("reissue", fault);
        assert_eq!(
            s.finish
                .recover(&s.registry, &s.keys, &scope(), 1)
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(s.token_backend.creates.load(Ordering::SeqCst), 0);
        assert_eq!(
            s.finish
                .recover(&s.registry, &s.keys, &scope(), 2)
                .await
                .unwrap()
                .generation,
            Some(3)
        );
    }
}
#[tokio::test]
async fn expired_proof_cannot_be_signed() {
    let s = setup(true).await;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/community-completion");
    let mut fields: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("fields.json")).unwrap()).unwrap();
    let past = Utc::now() - chrono::Duration::hours(1);
    fields["expires_at"] =
        json!((past + chrono::Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Secs, true));
    fields["purpose"] = json!("status");
    fields["expected_token_generation"] = Value::Null;
    let proof = Proof::validate(
        &scope(),
        &reg(),
        "status",
        &json!({"ok":true,"request_id":"fixture","challenge":fields}),
        None,
        None,
        past,
    )
    .unwrap();
    assert_eq!(
        s.keys
            .proof_signature(&scope(), &s.mock.public, &proof)
            .await
            .unwrap_err(),
        IdentityError::Expired
    );
}
#[tokio::test]
async fn independent_database_pools_have_one_recovery_cas_winner() {
    let s = setup(true).await;
    assert!(s
        .finish
        .complete(&s.registry, &s.keys, &scope())
        .await
        .is_err());
    let other = RegistrationRegistry::open(&s.temp.0.join("registry.sqlite"))
        .await
        .unwrap();
    let scope = scope();
    let (a, b) = tokio::join!(
        s.finish.recover(&s.registry, &s.keys, &scope, 1),
        s.finish.recover(&other, &s.keys, &scope, 1)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        a.err().or(b.err()).unwrap(),
        FinishError::Registry(RegistryError::Conflict)
    );
    assert_eq!(s.mock.count("/token/reissue/"), 1);
}
