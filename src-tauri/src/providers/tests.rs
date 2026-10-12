use super::*;
use serde_json::json;
fn request(provider:ProviderKind)->ProviderChatRequest {
    let endpoint=normalize_endpoint(provider,if provider==ProviderKind::CustomOpenai{"http://localhost:11434/v1"}else{""}).unwrap();
    let model=if provider==ProviderKind::Google{"models/gemini-example"}else{"model-example"};
    ProviderChatRequest{provider,model_id:model.into(),messages:vec![ProviderMessage::text("system","Be helpful"),ProviderMessage::text("user","Hello")],scope:ScopeStamp{profile_id:"p1".into(),connection_id:"c1".into(),route_id:"r1".into(),route_revision:1,credential_revision:1,model_id:model.into(),provider,normalized_endpoint:endpoint},temperature:None,max_output_tokens:Some(300),reasoning_effort:None,reasoning_max_tokens:None,reasoning_transport_kind:None,reasoning_transport_value:None,custom_output_token_field:None,provider_routing:None}
}
fn prepare(r:&ProviderChatRequest)->PreparedChat{prepare_chat(r,&r.scope.normalized_endpoint).unwrap()}
#[test]fn fixed_provider_endpoints(){for p in [ProviderKind::Openrouter,ProviderKind::Openai,ProviderKind::Google,ProviderKind::Anthropic]{assert!(normalize_endpoint(p,"").unwrap().starts_with("https://"));assert_eq!(normalize_endpoint(p,"https://unexpected.example"),Err(ProtocolError::InvalidEndpoint));}}
#[test]fn custom_endpoint_rejects_credentials_query_fragment_and_remote_http(){for url in ["https://user:secret@example.com/v1","https://example.com/v1?api_key=secret","https://example.com/v1#key","http://example.com/v1","https://example.com:65536/v1","https://example.com\r\n/v1"]{assert_eq!(normalize_endpoint(ProviderKind::CustomOpenai,url),Err(ProtocolError::InvalidEndpoint));}assert_eq!(normalize_endpoint(ProviderKind::CustomOpenai," http://LOCALHOST:11434/v1/ ").unwrap(),"http://localhost:11434/v1");}
#[test]fn openai_compatible_output_fields(){let openai=prepare(&request(ProviderKind::Openai));assert!(openai.endpoint.ends_with("/chat/completions"));assert_eq!(openai.body["max_completion_tokens"],300);assert!(openai.body.get("reasoning_effort").is_none());let custom=prepare(&request(ProviderKind::CustomOpenai));assert_eq!(custom.body["max_tokens"],300);}
#[test]fn openrouter_preserves_reasoning_and_provider_routing(){let mut r=request(ProviderKind::Openrouter);r.reasoning_max_tokens=Some(100);r.provider_routing=Some(OpenRouterProviderRouting{order:vec!["provider-a".into()],allow_fallbacks:Some(false),..Default::default()});let p=prepare(&r);assert_eq!(p.body["reasoning"]["max_tokens"],100);assert_eq!(p.body["provider"]["allow_fallbacks"],false);assert_eq!(p.body["model"],"model-example");}
#[test]fn google_system_and_roles(){let mut r=request(ProviderKind::Google);r.messages.push(ProviderMessage::text("assistant","Hi"));let p=prepare(&r);assert!(p.endpoint.ends_with("models/gemini-example:generateContent"));assert_eq!(p.body["systemInstruction"]["parts"][0]["text"],"Be helpful");assert_eq!(p.body["contents"][1]["role"],"model");assert_eq!(p.body["generationConfig"]["maxOutputTokens"],300);}
#[test]fn anthropic_system_and_required_output_limit(){let p=prepare(&request(ProviderKind::Anthropic));assert!(p.endpoint.ends_with("/messages"));assert_eq!(p.body["system"],"Be helpful");assert_eq!(p.body["max_tokens"],300);assert_eq!(p.body["messages"].as_array().unwrap().len(),1);}
#[test]fn invalid_inputs_and_google_path_injection_rejected(){let mut r=request(ProviderKind::Google);r.model_id="models/gemini:bad".into();r.scope.model_id=r.model_id.clone();assert!(matches!(prepare_chat(&r,&r.scope.normalized_endpoint),Err(ProtocolError::InvalidRequest)));let mut r=request(ProviderKind::Openai);r.temperature=Some(f64::NAN);assert!(matches!(prepare_chat(&r,&r.scope.normalized_endpoint),Err(ProtocolError::InvalidRequest)));r.temperature=None;r.max_output_tokens=Some(0);assert!(matches!(prepare_chat(&r,&r.scope.normalized_endpoint),Err(ProtocolError::InvalidRequest)));}
#[test]fn response_visible_text_excludes_reasoning(){let r=parse_response(ProviderKind::Openrouter,json!({"choices":[{"message":{"content":"<think>private</think>Visible"}}]})).unwrap();assert_eq!(r.content(),"Visible");let r=parse_response(ProviderKind::Google,json!({"candidates":[{"content":{"parts":[{"thought":true,"text":"private"},{"text":"Visible"}]}}]})).unwrap();assert_eq!(r.content(),"Visible");}
#[test]fn anthropic_signed_turn_keeps_exact_visible_text(){let r=parse_response(ProviderKind::Anthropic,json!({"content":[{"type":"thinking","thinking":"private","signature":"opaque"},{"type":"text","text":"<think>literal visible tag</think>"}]})).unwrap();assert_eq!(r.content(),"<think>literal visible tag</think>");}
#[test]fn response_errors_are_safe_and_empty_output_is_not_success(){for p in [ProviderKind::Openai,ProviderKind::Openrouter,ProviderKind::Google,ProviderKind::Anthropic]{assert!(matches!(parse_response(p,json!({"error":{"message":"API-KEY-PRIVATE"}})),Err(ProtocolError::InvalidResponse)));assert!(matches!(parse_response(p,json!({})),Err(ProtocolError::InvalidResponse)));}}
#[test]fn usage_fields_are_preserved(){let r=parse_response(ProviderKind::Openai,json!({"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}})).unwrap();assert_eq!(r.usage().input_tokens,Some(12));assert_eq!(r.usage().total_tokens,Some(15));assert_eq!(r.finish_reason(),Some("stop"));}
#[test]fn openrouter_replay_does_not_cross_profile_model_route_or_credential_revision(){let mut r=request(ProviderKind::Openrouter);let stamp=r.scope.clone();r.messages.push(ProviderMessage{role:"assistant".into(),content:"reply".into(),images:vec![],continuation_state:Some(ProviderContinuationState::OpenRouter(OpenRouterContinuationState{stamp,reasoning_details:vec![json!({"type":"reasoning.encrypted","data":"opaque"})]}))});assert!(prepare(&r).body["messages"][2].get("reasoning_details").is_some());for key in 0..6{let mut modified=r.clone();match key{0=>modified.scope.profile_id="p2".into(),1=>{modified.model_id="other-model".into();modified.scope.model_id=modified.model_id.clone();},2=>modified.scope.route_revision=2,3=>modified.scope.credential_revision=2,4=>modified.scope.connection_id="c2".into(),_=>modified.scope.route_id="scribe-route".into()};assert!(matches!(prepare_chat(&modified,&modified.scope.normalized_endpoint),Err(ProtocolError::RouteMismatch)));}}
#[test]fn google_replay_preserves_full_part_sequence_and_rejects_changed_visible_content(){let mut r=request(ProviderKind::Google);r.messages.push(ProviderMessage{role:"assistant".into(),content:"Visible".into(),images:vec![],continuation_state:Some(ProviderContinuationState::Google(GoogleContinuationState{stamp:r.scope.clone(),parts:vec![GoogleThoughtPart{text:"Private".into(),thought:Some(true),thought_signature:Some("sig".into())},GoogleThoughtPart{text:"Visible".into(),thought:None,thought_signature:Some("visible-sig".into())}]}))});let p=prepare(&r);assert_eq!(p.body["contents"][1]["parts"][1]["thoughtSignature"],"visible-sig");r.messages[2].content="changed".into();assert!(matches!(prepare_chat(&r,&r.scope.normalized_endpoint),Err(ProtocolError::InvalidRequest)));}
#[test]fn image_payload_shapes_are_preserved(){for p in [ProviderKind::Openai,ProviderKind::Openrouter,ProviderKind::Google,ProviderKind::Anthropic]{let mut r=request(p);r.messages[1].images.push(ProviderImage{mime_type:"image/png".into(),data_base64:"aGVsbG8=".into()});let prepared=prepare(&r);assert!(prepared.body.to_string().contains("aGVsbG8="));}}

#[test]
fn google_signed_literal_text_survives_parser_to_replay() {
    let visible = "<think>przykład znacznika</think>Odpowiedź";
    for include_thought in [false, true] {
        let mut original_parts = Vec::new();
        if include_thought {
            original_parts.push(json!({"text":"Private", "thought":true, "thoughtSignature":"private-sig"}));
        }
        original_parts.push(json!({"text":visible, "thoughtSignature":"visible-sig"}));
        original_parts.push(json!({"text":" · dalszy tekst", "thought":false}));
        let parsed = parse_response(ProviderKind::Google, json!({
            "candidates":[{"content":{"parts":original_parts.clone()}}]
        })).unwrap();
        assert_eq!(parsed.content(), format!("{visible} · dalszy tekst"));
        let replay_parts = parsed.reasoning_details.as_ref().unwrap().iter().map(|part| GoogleThoughtPart {
            text: part["text"].as_str().unwrap().to_string(),
            thought: part.get("thought").and_then(Value::as_bool),
            thought_signature: part.get("thoughtSignature").and_then(Value::as_str).map(str::to_string),
        }).collect();
        let mut r = request(ProviderKind::Google);
        r.messages.push(ProviderMessage {
            role:"assistant".into(), content:parsed.content().into(), images:vec![],
            continuation_state:Some(ProviderContinuationState::Google(GoogleContinuationState {
                stamp:r.scope.clone(), parts:replay_parts,
            })),
        });
        let replay = prepare(&r);
        assert_eq!(replay.body["contents"][1]["parts"], json!(original_parts));
    }
}
