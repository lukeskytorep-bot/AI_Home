use super::super::{ProviderMessage, ProviderUsage};
use super::*;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::mpsc,
    task::JoinHandle,
};

fn request(base: String) -> ProviderChatRequest {
    let provider = ProviderKind::CustomOpenai;
    ProviderChatRequest {
        provider,
        model_id: "test-model".into(),
        messages: vec![ProviderMessage::text("user", "Hello")],
        scope: ScopeStamp {
            profile_id: "p1".into(),
            connection_id: "c1".into(),
            route_id: "primary".into(),
            route_revision: 1,
            credential_revision: 1,
            model_id: "test-model".into(),
            provider,
            normalized_endpoint: base,
        },
        temperature: None,
        max_output_tokens: Some(64),
        reasoning_effort: None,
        reasoning_max_tokens: None,
        reasoning_transport_kind: None,
        reasoning_transport_value: None,
        custom_output_token_field: None,
        provider_routing: None,
    }
}
fn credential(r: &ProviderChatRequest) -> BoundCredential {
    BoundCredential::new(&r.scope, "test-secret".into()).unwrap()
}
struct Server {
    base: String,
    requests: mpsc::UnboundedReceiver<Vec<u8>>,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn server(responses: Vec<Vec<u8>>) -> Server {
    server_script(responses.into_iter().map(|bytes| (bytes, false)).collect()).await
}
async fn server_script(responses: Vec<(Vec<u8>, bool)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let (tx, requests) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        for (response, stall_after_write) in responses {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let header_end = loop {
                let mut buffer = [0; 2048];
                let size = stream.read(&mut buffer).await.unwrap();
                if size == 0 {
                    return;
                }
                data.extend_from_slice(&buffer[..size]);
                if let Some(start) = data.windows(4).position(|v| v == b"\r\n\r\n") {
                    break start + 4;
                }
            };
            let headers = String::from_utf8_lossy(&data[..header_end]).to_ascii_lowercase();
            let length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .and_then(|n| n.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while data.len() < header_end + length {
                let mut buffer = [0; 2048];
                let size = stream.read(&mut buffer).await.unwrap();
                if size == 0 {
                    return;
                }
                data.extend_from_slice(&buffer[..size]);
            }
            tx.send(data).unwrap();
            if response.is_empty() {
                std::future::pending::<()>().await;
            } else {
                stream.write_all(&response).await.unwrap();
                if stall_after_write {
                    std::future::pending::<()>().await;
                }
                stream.shutdown().await.unwrap();
            }
        }
    });
    Server {
        base,
        requests,
        task,
    }
}
fn response(status: &str, body: &str, headers: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n{headers}\r\n{body}", body.len()).into_bytes()
}
fn good() -> Vec<u8> {
    response(
        "200 OK",
        r#"{"choices":[{"message":{"content":"Hello back"}}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#,
        "",
    )
}
fn budget() -> CallBudget {
    CallBudget::standard(Cancellation::default())
}

#[test]
fn provider_auth_headers_and_no_secret_query() {
    let client = Client::new();
    for (p, name, expected) in [
        (ProviderKind::Openai, "authorization", "Bearer opaque"),
        (ProviderKind::CustomOpenai, "authorization", "Bearer opaque"),
        (ProviderKind::Openrouter, "authorization", "Bearer opaque"),
        (ProviderKind::Google, "x-goog-api-key", "opaque"),
        (ProviderKind::Anthropic, "x-api-key", "opaque"),
    ] {
        let r = authenticated(client.get("https://example.com/v1/models"), p, "opaque")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(r.headers()[name], expected);
        assert!(r.headers()[name].is_sensitive());
        assert!(r.url().query().is_none());
        if p == ProviderKind::Anthropic {
            assert_eq!(r.headers()["anthropic-version"], "2023-06-01");
        }
        if p == ProviderKind::Openrouter {
            assert_eq!(r.headers()["X-OpenRouter-Title"], "AI Home");
            assert!(r.headers().get("HTTP-Referer").is_none());
        }
    }
}
#[test]
fn credential_rejects_header_injection_and_missing_binding() {
    let mut r = request("http://localhost:11434/v1".into());
    for secret in ["", "key\r\nInjected: bad", "spaces forbidden", "ą"] {
        assert!(matches!(
            BoundCredential::new(&r.scope, secret.into()),
            Err(TransportError::Configuration)
        ));
    }
    r.scope.connection_id.clear();
    assert!(matches!(
        BoundCredential::new(&r.scope, "key".into()),
        Err(TransportError::Configuration)
    ));
}
#[test]
fn credential_scope_is_independent_of_primary_scribe_route() {
    let r = request("http://localhost:11434/v1".into());
    let c = credential(&r);
    let mut s = r.scope.clone();
    s.route_id = "scribe".into();
    s.route_revision = 2;
    s.model_id = "different".into();
    assert!(c.matches(&s));
    for field in 0..5 {
        let mut s = r.scope.clone();
        match field {
            0 => s.profile_id = "p2".into(),
            1 => s.connection_id = "c2".into(),
            2 => s.provider = ProviderKind::Google,
            3 => s.normalized_endpoint = "http://localhost:11435/v1".into(),
            _ => s.credential_revision = 2,
        }
        assert!(!c.matches(&s));
    }
}
#[test]
fn retry_after_seconds_dates_and_invalid() {
    let now = std::time::UNIX_EPOCH + Duration::from_secs(1_445_412_470);
    assert_eq!(retry_after_ms_at("5", now), Some(5000));
    assert_eq!(retry_after_ms_at("999999999999", now), Some(30_000));
    assert_eq!(
        retry_after_ms_at("Wed, 21 Oct 2015 07:28:00 GMT", now),
        Some(10_000)
    );
    assert_eq!(
        retry_after_ms_at("Wed, 21 Oct 2015 07:27:00 GMT", now),
        Some(0)
    );
    assert_eq!(retry_after_ms_at("not a date", now), None);
}
#[test]
fn url_canonicalization_and_rejections() {
    assert_eq!(
        super::super::normalize_endpoint(ProviderKind::CustomOpenai, "https://EXAMPLE.com:443/v1/")
            .unwrap(),
        "https://example.com/v1"
    );
    assert_eq!(
        super::super::normalize_endpoint(ProviderKind::CustomOpenai, "http://[::1]:11434/v1")
            .unwrap(),
        "http://[::1]:11434/v1"
    );
    for endpoint in [
        "https://example.com/v1?key=private",
        "https://example.com/v1#fragment",
        "https://user:private@example.com",
        "http://remote.example",
        "https://example.com:0/v1",
        "https://example.com\n/v1",
    ] {
        assert!(super::super::normalize_endpoint(ProviderKind::CustomOpenai, endpoint).is_err());
    }
}
#[tokio::test]
async fn successful_chat_uses_native_http_and_parser() {
    let mut s = server(vec![good()]).await;
    let r = request(s.base.clone());
    let result = HttpTransport::new()
        .unwrap()
        .chat(&r, &credential(&r), &budget())
        .await
        .unwrap();
    assert_eq!(result.content(), "Hello back");
    let ProviderUsage { total_tokens, .. } = result.usage();
    assert_eq!(*total_tokens, Some(5));
    let raw = s.requests.recv().await.unwrap();
    let wire = String::from_utf8(raw).unwrap();
    assert!(wire.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(wire.contains("Bearer test-secret"));
    let body: Value = serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["messages"][0]["content"], "Hello");
}
#[tokio::test]
async fn credential_mismatch_and_protocol_failure_never_dispatch() {
    let r = request("http://127.0.0.1:1/v1".into());
    let c = credential(&r);
    let b = budget();
    let t = HttpTransport::new().unwrap();
    let mut changed = r.clone();
    changed.scope.profile_id = "p2".into();
    assert!(matches!(
        t.chat(&changed, &c, &b).await,
        Err(TransportError::CredentialMismatch)
    ));
    changed = r.clone();
    changed.scope.model_id = "other".into();
    assert!(matches!(
        t.chat(&changed, &c, &b).await,
        Err(TransportError::Protocol(ProtocolError::RouteMismatch))
    ));
    assert_eq!(b.physical_calls(), 0);
}
#[tokio::test]
async fn http_errors_are_safe_and_not_retried() {
    let mut s = server(vec![response(
        "429 Too Many Requests",
        "test-secret private prompt",
        "Retry-After: 10\r\n",
    )])
    .await;
    let r = request(s.base.clone());
    let b = budget();
    let error = HttpTransport::new()
        .unwrap()
        .chat(&r, &credential(&r), &b)
        .await
        .err()
        .unwrap();
    assert_eq!(
        error,
        TransportError::HttpStatus {
            status: 429,
            retry_after_ms: Some(10_000)
        }
    );
    assert!(!format!("{error:?} {error}").contains("test-secret"));
    assert_eq!(b.physical_calls(), 1);
    assert!(s.requests.recv().await.is_some());
}
#[tokio::test]
async fn redirect_is_rejected_without_followup() {
    let mut s = server(vec![response(
        "307 Temporary Redirect",
        "",
        "Location: http://127.0.0.1:1/steal\r\n",
    )])
    .await;
    let r = request(s.base.clone());
    let b = budget();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .chat(&r, &credential(&r), &b)
            .await,
        Err(TransportError::HttpStatus { status: 307, .. })
    ));
    assert_eq!(b.physical_calls(), 1);
    assert!(s.requests.recv().await.is_some());
}
#[tokio::test]
async fn invalid_json_and_empty_semantic_output_fail() {
    for (body, expected) in [
        ("bad private data", TransportError::InvalidJson),
        (
            "{}",
            TransportError::Protocol(ProtocolError::InvalidResponse),
        ),
    ] {
        let s = server(vec![response("200 OK", body, "")]).await;
        let r = request(s.base.clone());
        assert_eq!(
            HttpTransport::new()
                .unwrap()
                .chat(&r, &credential(&r), &budget())
                .await
                .err()
                .unwrap(),
            expected
        );
    }
}
#[tokio::test]
async fn declared_body_limit_is_checked() {
    let s = server(vec![good()]).await;
    let r = request(s.base.clone());
    assert!(matches!(
        HttpTransport::with_body_limit(10)
            .unwrap()
            .chat(&r, &credential(&r), &budget())
            .await,
        Err(TransportError::ResponseTooLarge)
    ));
}
#[tokio::test]
async fn chunked_body_limit_is_checked_without_content_length() {
    let raw=b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n6\r\n123456\r\n6\r\n123456\r\n0\r\n\r\n".to_vec();
    let s = server(vec![raw]).await;
    let r = request(s.base.clone());
    assert!(matches!(
        HttpTransport::with_body_limit(10)
            .unwrap()
            .chat(&r, &credential(&r), &budget())
            .await,
        Err(TransportError::ResponseTooLarge)
    ));
}
#[tokio::test]
async fn precancellation_is_sticky_and_does_not_dispatch() {
    let r = request("http://127.0.0.1:1/v1".into());
    let cancel = Cancellation::default();
    cancel.cancel();
    let b = CallBudget::standard(cancel);
    let t = HttpTransport::new().unwrap();
    for _ in 0..2 {
        assert!(matches!(
            t.chat(&r, &credential(&r), &b).await,
            Err(TransportError::Cancelled)
        ));
    }
    assert_eq!(b.physical_calls(), 0);
}
#[tokio::test]
async fn cancellation_during_request_drops_http_future() {
    let mut s = server(vec![vec![]]).await;
    let r = request(s.base.clone());
    let c = credential(&r);
    let cancel = Cancellation::default();
    let b = CallBudget::standard(cancel.clone());
    let t = HttpTransport::new().unwrap();
    let future = t.chat(&r, &c, &b);
    tokio::pin!(future);
    tokio::select! { result=&mut future=>panic!("unexpected early result: {:?}",result.err()), raw=s.requests.recv()=>assert!(raw.is_some()) }
    cancel.cancel();
    assert!(matches!(future.await, Err(TransportError::Cancelled)));
    assert_eq!(b.physical_calls(), 1);
}
#[tokio::test]
async fn truncated_body_is_network_failure() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{".to_vec();
    // A complete header plus partial body followed by close is a network failure.
    let s = server(vec![raw]).await;
    let r = request(s.base.clone());
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .chat(&r, &credential(&r), &budget())
            .await,
        Err(TransportError::BodyRead)
    ));
}
#[tokio::test]
async fn total_deadline_includes_wait_for_headers() {
    let mut s = server(vec![vec![]]).await;
    let r = request(s.base.clone());
    // Client/TLS setup and credential validation must not consume the short
    // test deadline before the HTTP request can be dispatched on a busy runner.
    let t = HttpTransport::new().unwrap();
    let c = credential(&r);
    let b = CallBudget::new(Duration::from_secs(2), 1, Cancellation::default()).unwrap();
    let future = t.chat(&r, &c, &b);
    tokio::pin!(future);
    // Prove the server received the request and is deliberately withholding
    // headers. An early pre-dispatch timeout must not count as a passing test.
    tokio::select! {
        result = &mut future => panic!("request ended before server receipt: {:?}", result.err()),
        raw = s.requests.recv() => assert!(raw.is_some(), "server did not receive the request"),
    }
    assert_eq!(b.physical_calls(), 1);
    assert!(matches!(future.await, Err(TransportError::Timeout)));
    assert_eq!(b.physical_calls(), 1);
}
#[tokio::test]
async fn shared_budget_limits_sequential_physical_calls() {
    let s = server(vec![good()]).await;
    let r = request(s.base.clone());
    let c = credential(&r);
    let b = CallBudget::new(Duration::from_secs(2), 1, Cancellation::default()).unwrap();
    let t = HttpTransport::new().unwrap();
    assert!(t.chat(&r, &c, &b).await.is_ok());
    assert!(matches!(
        t.chat(&r, &c, &b.clone()).await,
        Err(TransportError::CallLimit)
    ));
    assert_eq!(b.physical_calls(), 1);
}
#[tokio::test]
async fn discovery_is_paginated_with_shared_budget() {
    let mut s = server(vec![
        response("200 OK", r#"{"data":[{"id":"m1"}],"has_more":true}"#, ""),
        response("200 OK", r#"{"data":[{"id":"m2"}],"has_more":false}"#, ""),
    ])
    .await;
    let r = request(s.base.clone());
    let b = budget();
    let models = HttpTransport::new()
        .unwrap()
        .discover_models(&r.scope, &credential(&r), &b)
        .await
        .unwrap();
    assert_eq!(
        models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        vec!["m1", "m2"]
    );
    assert_eq!(b.physical_calls(), 2);
    let first = String::from_utf8(s.requests.recv().await.unwrap()).unwrap();
    let second = String::from_utf8(s.requests.recv().await.unwrap()).unwrap();
    assert!(first.starts_with("GET /v1/models HTTP/1.1"));
    assert!(second.starts_with("GET /v1/models?after=m1 HTTP/1.1"));
}
#[tokio::test]
async fn discovery_rejects_repeated_models_and_missing_page_budget() {
    let first = response("200 OK", r#"{"data":[{"id":"m1"}],"has_more":true}"#, "");
    let s = server(vec![first.clone(), first.clone()]).await;
    let r = request(s.base.clone());
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .discover_models(&r.scope, &credential(&r), &budget())
            .await,
        Err(TransportError::InvalidModelList)
    ));
    let s = server(vec![first]).await;
    let r = request(s.base.clone());
    let b = CallBudget::new(Duration::from_secs(2), 1, Cancellation::default()).unwrap();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .discover_models(&r.scope, &credential(&r), &b)
            .await,
        Err(TransportError::CallLimit)
    ));
}
#[tokio::test]
async fn fourth_incomplete_page_is_explicit_failure() {
    let pages = (1..=4)
        .map(|n| {
            response(
                "200 OK",
                &json!({"data":[{"id":format!("m{n}")}],"has_more":true}).to_string(),
                "",
            )
        })
        .collect();
    let s = server(pages).await;
    let r = request(s.base.clone());
    let b = budget();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .discover_models(&r.scope, &credential(&r), &b)
            .await,
        Err(TransportError::DiscoveryIncomplete)
    ));
    assert_eq!(b.physical_calls(), 4);
}

#[tokio::test]
async fn slow_response_body_uses_total_deadline() {
    let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{".to_vec();
    let s = server_script(vec![(partial, true)]).await;
    let r = request(s.base.clone());
    let b = CallBudget::new(Duration::from_millis(100), 1, Cancellation::default()).unwrap();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .chat(&r, &credential(&r), &b)
            .await,
        Err(TransportError::Timeout)
    ));
}
#[tokio::test]
async fn cancel_after_partial_response_body() {
    let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{".to_vec();
    let mut s = server_script(vec![(partial, true)]).await;
    let r = request(s.base.clone());
    let c = credential(&r);
    let cancel = Cancellation::default();
    let b = CallBudget::standard(cancel.clone());
    let t = HttpTransport::new().unwrap();
    let future = t.chat(&r, &c, &b);
    tokio::pin!(future);
    tokio::select! { result=&mut future=>panic!("unexpected early result: {:?}",result.err()), raw=s.requests.recv()=>assert!(raw.is_some()) }
    tokio::select! { result=&mut future=>panic!("unexpected partial-body result: {:?}",result.err()), _=tokio::time::sleep(Duration::from_millis(10))=>() }
    cancel.cancel();
    assert!(matches!(future.await, Err(TransportError::Cancelled)));
}
#[tokio::test]
async fn shared_call_budget_is_atomic_under_concurrency() {
    let b = budget();
    let mut tasks = Vec::new();
    for _ in 0..12 {
        let clone = b.clone();
        tasks.push(tokio::spawn(async move { clone.reserve() }));
    }
    let mut accepted = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(()) => accepted += 1,
            Err(TransportError::CallLimit) => (),
            other => panic!("unexpected budget result: {other:?}"),
        }
    }
    assert_eq!(accepted, 4);
    assert_eq!(b.physical_calls(), 4);
}
#[tokio::test]
async fn oversized_encoded_request_is_rejected_before_dispatch() {
    let mut r = request("http://127.0.0.1:1/v1".into());
    // Per-message limits fit, while the combined encoded request exceeds its cap.
    r.messages = vec![ProviderMessage::text("user", &"x".repeat(2 * 1024 * 1024)); 9];
    let b = budget();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .chat(&r, &credential(&r), &b)
            .await,
        Err(TransportError::RequestTooLarge)
    ));
    assert_eq!(b.physical_calls(), 0);
}
