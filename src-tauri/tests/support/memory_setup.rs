// Reused local loopback/vault fixture from tests/chat.rs; no live API.
use ai_home::{
    application::Application,
    domain::schema,
    persistence::Store,
    vault::{SecretStore, Vault, VaultError},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
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
    wires: Arc<Mutex<Vec<String>>>,
    responses: Arc<Mutex<std::collections::VecDeque<(u16, String)>>>,
    hold: Arc<AtomicBool>,
    release: Arc<Semaphore>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let wires = Arc::new(Mutex::new(vec![]));
    let responses = Arc::new(Mutex::new(
        std::collections::VecDeque::<(u16, String)>::new(),
    ));
    let hold = Arc::new(AtomicBool::new(false));
    let release = Arc::new(Semaphore::new(0));
    let (w, q, h, g) = (
        wires.clone(),
        responses.clone(),
        hold.clone(),
        release.clone(),
    );
    let task = tokio::spawn(async move {
        let mut children = tokio::task::JoinSet::new();
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let (w, q, h, g) = (w.clone(), q.clone(), h.clone(), g.clone());
            children.spawn(async move{let mut data=vec![];let end=loop{let mut b=[0;2048];let n=socket.read(&mut b).await.unwrap();if n==0{return;}data.extend_from_slice(&b[..n]);if let Some(i)=data.windows(4).position(|x|x==b"\r\n\r\n"){break i+4;}};let head=String::from_utf8_lossy(&data[..end]).to_lowercase();let len=head.lines().find_map(|x|x.strip_prefix("content-length:").and_then(|s|s.trim().parse::<usize>().ok())).unwrap_or(0);while data.len()<end+len{let mut b=[0;2048];let n=socket.read(&mut b).await.unwrap();if n==0{return;}data.extend_from_slice(&b[..n]);}let raw=String::from_utf8(data).unwrap();let post=raw.starts_with("POST ");if post{w.lock().unwrap().push(raw);if h.load(Ordering::SeqCst){g.acquire().await.unwrap().forget();}}let(status,body)=if post{q.lock().unwrap().pop_front().unwrap_or((200,r#"{"choices":[{"message":{"content":"Reply 🪐"},"finish_reason":"stop"}]}"#.into()))}else{(200,r#"{"data":[{"id":"mock-model"}]}"#.into())};let response=format!("HTTP/1.1 {status} Mock\r\nContent-Length: {}\r\nConnection: close\r\nRetry-After: 0\r\n\r\n{body}",body.len());let _=socket.write_all(response.as_bytes()).await;});
        }
    });
    Server {
        endpoint,
        wires,
        responses,
        hold,
        release,
        task,
    }
}
// Fixture watchdog only: tolerate shared CI scheduling without changing provider budgets.
async fn finished(app: &Application, p: &str, id: &Value) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let op = ok(
                app,
                "operation.get",
                json!({"operationId":id}),
                Some(p),
                None,
            )
            .await;
            if !matches!(op["status"].as_str(), Some("queued") | Some("running")) {
                assert!(schema::dto("Operation", &op), "{op}");
                return op;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap()
}
async fn configured() -> (
    Application,
    sqlx::SqlitePool,
    Arc<MemoryVault>,
    Server,
    String,
    Value,
    Value,
) {
    let (app, pool, vault) = setup().await;
    configure(app, pool, vault).await
}
async fn configure(
    app: Application,
    pool: sqlx::SqlitePool,
    vault: Arc<MemoryVault>,
) -> (
    Application,
    sqlx::SqlitePool,
    Arc<MemoryVault>,
    Server,
    String,
    Value,
    Value,
) {
    let server = server().await;
    let p = profile(&app, "Chat").await;
    let pid = p["id"].as_str().unwrap().to_string();
    let c = connection(&app, &pid, &server.endpoint).await;
    let accepted = ok(
        &app,
        "connection.test",
        json!({"connectionId":c["id"]}),
        Some(&pid),
        Some(1),
    )
    .await;
    assert_eq!(
        finished(&app, &pid, &accepted["operation"]["id"]).await["status"],
        "succeeded"
    );
    let route=ok(&app,"route.save",json!({"route":{"role":"primary","connectionId":c["id"],"modelId":"mock-model","outputLimitTokens":1024}}),Some(&pid),Some(1)).await;
    ok(
        &app,
        "route.assign",
        json!({"role":"primary","routeId":route["id"]}),
        Some(&pid),
        Some(2),
    )
    .await;
    ok(
        &app,
        "chat.settings.set",
        json!({"routeId":route["id"],"contextTokens":32768}),
        Some(&pid),
        Some(1),
    )
    .await;
    let conv = ok(
        &app,
        "conversation.create",
        json!({"title":"Chat","language":"en"}),
        Some(&pid),
        None,
    )
    .await;
    (app, pool, vault, server, pid, conv, c)
}
fn send_request(p: &str, c: &Value, text: &str) -> Value {
    request(
        "chat.send",
        json!({"conversationId":c["id"],"content":text,"language":"pl","attachmentIds":[]}),
        Some(p),
        c["revision"].as_i64(),
    )
}
