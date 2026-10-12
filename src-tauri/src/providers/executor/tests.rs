use super::super::transport::{BoundCredential, CallRecorder, Cancellation, HttpTransport};
use super::super::{
    parse_response, ProviderChatRequest, ProviderKind, ProviderMessage, ScopeStamp,
};
use super::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
fn request(endpoint: String) -> ProviderChatRequest {
    ProviderChatRequest {
        provider: ProviderKind::CustomOpenai,
        model_id: "mock-model".into(),
        messages: vec![ProviderMessage::text("user", "Hello")],
        scope: ScopeStamp {
            profile_id: "p1".into(),
            connection_id: "c1".into(),
            route_id: "r1".into(),
            route_revision: 1,
            credential_revision: 1,
            model_id: "mock-model".into(),
            provider: ProviderKind::CustomOpenai,
            normalized_endpoint: endpoint,
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
struct Server {
    base: String,
    wires: Arc<Mutex<Vec<Vec<u8>>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn response(status: u16, body: &str, headers: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
    .into_bytes()
}
fn good() -> Vec<u8> {
    response(
        200,
        r#"{"choices":[{"message":{"content":"Complete"},"finish_reason":"stop"}]}"#,
        "",
    )
}
async fn server(responses: Vec<Vec<u8>>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!(
        "http://127.0.0.1:{}/v1",
        listener.local_addr().unwrap().port()
    );
    let wires = Arc::new(Mutex::new(vec![]));
    let captured = wires.clone();
    let task = tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = Vec::new();
            let end = loop {
                let mut b = [0; 2048];
                let n = socket.read(&mut b).await.unwrap();
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&b[..n]);
                if let Some(start) = data.windows(4).position(|p| p == b"\r\n\r\n") {
                    break start + 4;
                }
            };
            let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|s| {
                    s.strip_prefix("content-length:")
                        .and_then(|n| n.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while data.len() < end + len {
                let mut b = [0; 2048];
                let n = socket.read(&mut b).await.unwrap();
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&b[..n]);
            }
            captured.lock().unwrap().push(data);
            let _ = socket.write_all(&response).await;
            let _ = socket.shutdown().await;
        }
    });
    Server { base, wires, task }
}
struct Recorder(Mutex<Vec<u8>>);
impl CallRecorder for Recorder {
    fn reserved(
        &self,
        n: u8,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move {
            self.0.lock().unwrap().push(n);
            Ok(())
        })
    }
}
async fn execute(s: &Server, b: &CallBudget) -> Result<String, TransportError> {
    let r = request(s.base.clone());
    let c = BoundCredential::new(&r.scope, "mock-secret".into()).unwrap();
    HttpTransport::new()
        .unwrap()
        .execute_chat(&r, &c, b)
        .await
        .map(|v| v.content().to_string())
}
fn budget() -> CallBudget {
    CallBudget::standard(Cancellation::default())
}
#[test]
fn categories_match_harness_and_never_retry_authority_limits() {
    for status in [408, 425, 429, 500, 502, 503, 504, 524, 529] {
        assert_eq!(
            category(TransportError::HttpStatus {
                status,
                retry_after_ms: None
            }),
            Category::Standard
        );
    }
    for status in [400, 401, 403, 404, 409, 422] {
        assert_eq!(
            category(TransportError::HttpStatus {
                status,
                retry_after_ms: None
            }),
            Category::Never
        );
    }
    for e in [
        TransportError::Cancelled,
        TransportError::Storage,
        TransportError::Configuration,
        TransportError::CredentialMismatch,
        TransportError::ResponseTooLarge,
        TransportError::RequestTooLarge,
        TransportError::CallLimit,
        TransportError::Protocol(ProtocolError::OutputIncomplete),
        TransportError::Protocol(ProtocolError::PolicyBlocked),
        TransportError::Protocol(ProtocolError::RouteMismatch),
        TransportError::InvalidModelList,
    ] {
        assert_eq!(category(e), Category::Never);
    }
    assert_eq!(category(TransportError::BodyRead), Category::SingleRecovery);
}
#[test]
fn retry_after_and_jitter_are_bounded() {
    assert_eq!(
        delay_ms(
            TransportError::HttpStatus {
                status: 429,
                retry_after_ms: Some(100_000)
            },
            0,
            0.0
        ),
        30_000
    );
    assert_eq!(delay_ms(TransportError::Network, 0, 0.5), 250);
    assert_eq!(delay_ms(TransportError::Network, 10, 1.0), 8000);
    assert_eq!(delay_ms(TransportError::Network, 0, f64::NAN), 0);
}
#[tokio::test]
async fn transient_http_retries_once_with_same_body_and_auth() {
    let s = server(vec![
        response(503, "private body", "Retry-After: 0\r\n"),
        good(),
    ])
    .await;
    let b = budget();
    assert_eq!(execute(&s, &b).await.unwrap(), "Complete");
    assert_eq!(b.physical_calls(), 2);
    let wires = s.wires.lock().unwrap();
    assert_eq!(wires[0], wires[1]);
}
#[tokio::test]
async fn a_second_transient_failure_stops() {
    let s = server(vec![
        response(503, "", "Retry-After: 0\r\n"),
        response(503, "", "Retry-After: 0\r\n"),
        good(),
    ])
    .await;
    let b = budget();
    assert!(matches!(
        execute(&s, &b).await,
        Err(TransportError::HttpStatus { status: 503, .. })
    ));
    assert_eq!(b.physical_calls(), 2);
    assert_eq!(s.wires.lock().unwrap().len(), 2);
}
#[tokio::test]
async fn invalid_json_has_only_one_recovery() {
    let s = server(vec![
        response(200, "invalid", ""),
        response(200, "invalid again", ""),
        good(),
    ])
    .await;
    let b = budget();
    assert_eq!(
        execute(&s, &b).await.err(),
        Some(TransportError::InvalidJson)
    );
    assert_eq!(b.physical_calls(), 2);
}
#[tokio::test]
async fn explicit_empty_response_can_recover() {
    let s = server(vec![
        response(
            200,
            r#"{"choices":[{"message":{"content":""},"finish_reason":"stop"}]}"#,
            "",
        ),
        good(),
    ])
    .await;
    let b = budget();
    assert_eq!(execute(&s, &b).await.unwrap(), "Complete");
    assert_eq!(b.physical_calls(), 2);
}
#[tokio::test]
async fn recovery_and_standard_retry_share_four_call_budget() {
    let s = server(vec![
        response(503, "", "Retry-After: 0\r\n"),
        response(200, "bad JSON", ""),
        response(503, "", "Retry-After: 0\r\n"),
        good(),
    ])
    .await;
    let recorder = Arc::new(Recorder(Mutex::new(vec![])));
    let b = budget().with_recorder(recorder.clone());
    assert_eq!(execute(&s, &b).await.unwrap(), "Complete");
    assert_eq!(b.physical_calls(), 4);
    assert_eq!(*recorder.0.lock().unwrap(), vec![1, 2, 3, 4]);
    let wires = s.wires.lock().unwrap();
    assert_eq!(wires.len(), 4);
    assert!(wires.iter().all(|w| *w == wires[0]));
}
#[tokio::test]
async fn smaller_total_budget_cannot_be_reset_by_recovery() {
    let s = server(vec![
        response(503, "", "Retry-After: 0\r\n"),
        response(200, "bad JSON", ""),
        good(),
    ])
    .await;
    let b = CallBudget::new(Duration::from_secs(4), 2, Cancellation::default()).unwrap();
    assert_eq!(execute(&s, &b).await.err(), Some(TransportError::CallLimit));
    assert_eq!(b.physical_calls(), 2);
}
#[tokio::test]
async fn cancelled_backoff_makes_no_second_call() {
    let s = server(vec![response(429, "", "Retry-After: 30\r\n"), good()]).await;
    let cancel = Cancellation::default();
    let b = CallBudget::standard(cancel.clone());
    let future = execute(&s, &b);
    tokio::pin!(future);
    tokio::time::timeout(Duration::from_secs(3),async{tokio::select!{r=&mut future=>panic!("early result {r:?}"),_=async{while s.wires.lock().unwrap().is_empty(){tokio::time::sleep(Duration::from_millis(2)).await;}}=>()}}).await.unwrap();
    assert_eq!(s.wires.lock().unwrap().len(), 1);
    cancel.cancel();
    assert_eq!(future.await.err(), Some(TransportError::Cancelled));
    assert_eq!(b.physical_calls(), 1);
}
#[tokio::test]
async fn retry_after_cannot_outlive_the_operation_deadline() {
    let s = server(vec![response(429, "", "Retry-After: 30\r\n"), good()]).await;
    let b = CallBudget::new(Duration::from_secs(2), 4, Cancellation::default()).unwrap();
    assert_eq!(execute(&s, &b).await.err(), Some(TransportError::Timeout));
    assert_eq!(b.physical_calls(), 1);
}
#[tokio::test]
async fn refusals_limits_invalid_schema_and_auth_never_replay() {
    for (wire, error) in [
        (
            response(401, "mock-secret private body", ""),
            TransportError::HttpStatus {
                status: 401,
                retry_after_ms: None,
            },
        ),
        (
            response(
                200,
                r#"{"choices":[{"message":{"content":"Partial"},"finish_reason":"length"}]}"#,
                "",
            ),
            TransportError::Protocol(ProtocolError::OutputIncomplete),
        ),
        (
            response(
                200,
                r#"{"choices":[{"message":{"content":"Blocked"},"finish_reason":"content_filter"}]}"#,
                "",
            ),
            TransportError::Protocol(ProtocolError::PolicyBlocked),
        ),
        (
            response(200, "{}", ""),
            TransportError::Protocol(ProtocolError::InvalidResponse),
        ),
    ] {
        let s = server(vec![wire, good()]).await;
        let b = budget();
        assert_eq!(execute(&s, &b).await.err(), Some(error));
        assert_eq!(b.physical_calls(), 1);
    }
}
#[tokio::test]
async fn successful_semantic_response_is_never_replayed() {
    let s = server(vec![good(), good()]).await;
    let b = budget();
    assert_eq!(execute(&s, &b).await.unwrap(), "Complete");
    assert_eq!(b.physical_calls(), 1);
    assert_eq!(s.wires.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn truncated_body_has_single_recovery() {
    let s = server(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\nConnection: close\r\n\r\n{".to_vec(),
        good(),
    ])
    .await;
    let b = budget();
    assert_eq!(execute(&s, &b).await.unwrap(), "Complete");
    assert_eq!(b.physical_calls(), 2);
}
#[tokio::test]
async fn invalid_credentials_are_rejected_without_attempt() {
    let r = request("http://127.0.0.1:1/v1".into());
    let c = BoundCredential::new(&r.scope, "key".into()).unwrap();
    let mut changed = r.clone();
    changed.scope.credential_revision = 2;
    let b = budget();
    assert!(matches!(
        HttpTransport::new()
            .unwrap()
            .execute_chat(&changed, &c, &b)
            .await,
        Err(TransportError::CredentialMismatch)
    ));
    assert_eq!(b.physical_calls(), 0);
}
#[test]
fn completion_metadata_is_classified_without_private_strings() {
    for (provider, payload) in [
        (
            ProviderKind::Google,
            json!({"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"partial"}]}}]}),
        ),
        (
            ProviderKind::Anthropic,
            json!({"stop_reason":"max_tokens","content":[{"type":"text","text":"partial"}]}),
        ),
        (
            ProviderKind::Anthropic,
            json!({"stop_reason":"pause_turn","content":[]}),
        ),
    ] {
        assert!(matches!(
            parse_response(provider, payload),
            Err(ProtocolError::OutputIncomplete)
        ));
    }
    for (provider, payload) in [
        (
            ProviderKind::Google,
            json!({"promptFeedback":{"blockReason":"SAFETY"}}),
        ),
        (
            ProviderKind::Anthropic,
            json!({"stop_reason":"refusal","content":[]}),
        ),
        (
            ProviderKind::Openai,
            json!({"choices":[{"message":{"refusal":"private refusal","content":""},"finish_reason":"stop"}]}),
        ),
    ] {
        assert!(matches!(
            parse_response(provider, payload),
            Err(ProtocolError::PolicyBlocked)
        ));
    }
}
#[tokio::test]
async fn paginated_discovery_and_retry_keep_one_total_budget() {
    let s = server(vec![
        response(503, "", "Retry-After: 0\r\n"),
        response(200, r#"{"data":[{"id":"one"}],"has_more":true}"#, ""),
        response(200, r#"{"data":[{"id":"two"}]}"#, ""),
    ])
    .await;
    let r = request(s.base.clone());
    let c = BoundCredential::new(&r.scope, "key".into()).unwrap();
    let b = budget();
    let models = HttpTransport::new()
        .unwrap()
        .discover_models(&r.scope, &c, &b)
        .await
        .unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(b.physical_calls(), 3);
}
#[tokio::test]
async fn runner_exits_after_ok_without_outer_retries() {
    let count = AtomicUsize::new(0);
    let b = budget();
    let v = run(&b, || async {
        count.fetch_add(1, Ordering::SeqCst);
        Ok::<_, TransportError>(7)
    })
    .await
    .unwrap();
    assert_eq!(v, 7);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn google_text_with_function_call_is_incomplete_without_retry() {
    let payload = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"I will check"},{"functionCall":{"name":"lookup","args":{}}}]}}]});
    assert!(matches!(
        parse_response(ProviderKind::Google, payload.clone()),
        Err(ProtocolError::OutputIncomplete)
    ));
    let b = budget();
    let attempts = AtomicUsize::new(0);
    let result = run(&b, || async {
        attempts.fetch_add(1, Ordering::SeqCst);
        parse_response(ProviderKind::Google, payload.clone()).map_err(TransportError::Protocol)
    })
    .await;
    assert_eq!(
        result.err(),
        Some(TransportError::Protocol(ProtocolError::OutputIncomplete))
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn malformed_tool_calls_is_invalid_without_recovery_post() {
    let payload = json!({"choices":[{"finish_reason":"stop","message":{"content":"","tool_calls":"malformed"}}]});
    assert!(matches!(
        parse_response(ProviderKind::CustomOpenai, payload.clone()),
        Err(ProtocolError::InvalidResponse)
    ));
    let s = server(vec![response(200, &payload.to_string(), ""), good()]).await;
    let b = budget();
    assert_eq!(
        execute(&s, &b).await.err(),
        Some(TransportError::Protocol(ProtocolError::InvalidResponse))
    );
    assert_eq!(b.physical_calls(), 1);
    assert_eq!(s.wires.lock().unwrap().len(), 1);
}

#[test]
fn malformed_google_function_fields_are_not_empty_recovery() {
    for call in [
        json!(null),
        json!("malformed"),
        json!({}),
        json!({"name":"lookup","args":"bad"}),
    ] {
        let p = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":""},{"functionCall":call}]}}]});
        assert!(matches!(
            parse_response(ProviderKind::Google, p),
            Err(ProtocolError::InvalidResponse)
        ));
    }
}
#[test]
fn optional_openai_tool_fields_preserve_text_and_reject_wrong_types() {
    for field in [json!(null), json!([])] {
        let p = json!({"choices":[{"finish_reason":"stop","message":{"content":"Complete","tool_calls":field}}]});
        assert!(parse_response(ProviderKind::Openai, p).is_ok());
    }
    for field in [json!({}), json!(false), json!(1), json!([null])] {
        let p = json!({"choices":[{"finish_reason":"stop","message":{"content":"","tool_calls":field}}]});
        assert!(matches!(
            parse_response(ProviderKind::Openai, p),
            Err(ProtocolError::InvalidResponse)
        ));
    }
    let p = json!({"choices":[{"finish_reason":"stop","message":{"content":"Text","function_call":{"name":"lookup","arguments":"{}"}}}]});
    assert!(matches!(
        parse_response(ProviderKind::Openai, p),
        Err(ProtocolError::OutputIncomplete)
    ));
}
#[test]
fn anthropic_tool_block_requires_handling_even_with_end_turn() {
    let p = json!({"stop_reason":"end_turn","content":[{"type":"text","text":"Text"},{"type":"tool_use","id":"tool1","name":"lookup","input":{}}]});
    assert!(matches!(
        parse_response(ProviderKind::Anthropic, p),
        Err(ProtocolError::OutputIncomplete)
    ));
}
