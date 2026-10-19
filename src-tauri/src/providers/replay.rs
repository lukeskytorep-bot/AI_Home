//! Private native replay. Hidden protocol data never becomes an IPC DTO.
use super::*;
pub(crate) fn scope_value(s: &ScopeStamp) -> Value {
    serde_json::json!([
        s.profile_id,
        s.connection_id,
        s.route_id,
        s.route_revision,
        s.credential_revision,
        s.model_id,
        s.normalized_endpoint,
        match s.provider {
            ProviderKind::Google => "google",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Openrouter => "openrouter",
            ProviderKind::Openai => "openai",
            ProviderKind::CustomOpenai => "custom_openai",
        }
    ])
}
impl ProviderChatResponse {
    pub(crate) fn private_replay(&self, provider: ProviderKind) -> Option<Value> {
        if !matches!(
            provider,
            ProviderKind::Google | ProviderKind::Anthropic | ProviderKind::Openrouter
        ) {
            return None;
        }
        self.reasoning_details
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| serde_json::json!(v))
    }
}
pub(crate) fn restore(
    scope: &ScopeStamp,
    value: Value,
) -> Result<ProviderContinuationState, ProtocolError> {
    let values = value
        .as_array()
        .ok_or(ProtocolError::InvalidResponse)?
        .clone();
    match scope.provider {
        ProviderKind::Google => {
            let mut parts = vec![];
            for v in values {
                parts.push(GoogleThoughtPart {
                    text: v["text"]
                        .as_str()
                        .ok_or(ProtocolError::InvalidResponse)?
                        .into(),
                    thought: v
                        .get("thought")
                        .map(|x| x.as_bool().ok_or(ProtocolError::InvalidResponse))
                        .transpose()?,
                    thought_signature: v
                        .get("thoughtSignature")
                        .map(|x| {
                            x.as_str()
                                .map(String::from)
                                .ok_or(ProtocolError::InvalidResponse)
                        })
                        .transpose()?,
                });
            }
            Ok(ProviderContinuationState::Google(GoogleContinuationState {
                stamp: scope.clone(),
                parts,
            }))
        }
        ProviderKind::Openrouter => Ok(ProviderContinuationState::OpenRouter(
            OpenRouterContinuationState {
                stamp: scope.clone(),
                reasoning_details: values,
            },
        )),
        ProviderKind::Anthropic => Ok(ProviderContinuationState::Anthropic(
            AnthropicContinuationState {
                stamp: scope.clone(),
                blocks: values,
            },
        )),
        _ => Err(ProtocolError::InvalidResponse),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> ScopeStamp {
        ScopeStamp {
            profile_id: "p".into(),
            connection_id: "c".into(),
            route_id: "r".into(),
            route_revision: 1,
            credential_revision: 1,
            model_id: "gemini-test".into(),
            provider: ProviderKind::Google,
            normalized_endpoint: "https://generativelanguage.googleapis.com/v1beta".into(),
        }
    }
    #[test]
    fn google_signed_visible_parts_survive_private_round_trip() {
        let s = scope();
        let response=parse_response(ProviderKind::Google,serde_json::json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"<think>literal</think>Answer","thoughtSignature":"opaque-signature"}]}}]})).unwrap();
        let raw = response.private_replay(ProviderKind::Google).unwrap();
        let stored = serde_json::to_string(&raw).unwrap();
        let state = restore(&s, serde_json::from_str(&stored).unwrap()).unwrap();
        let mut message = ProviderMessage::text("assistant", response.content());
        message.continuation_state = Some(state);
        let r = ProviderChatRequest {
            provider: s.provider,
            model_id: s.model_id.clone(),
            scope: s.clone(),
            messages: vec![message, ProviderMessage::text("user", "Next")],
            temperature: None,
            max_output_tokens: Some(1024),
            reasoning_effort: None,
            reasoning_max_tokens: None,
            reasoning_transport_kind: None,
            reasoning_transport_value: None,
            custom_output_token_field: None,
            provider_routing: None,
        };
        let built = prepare_chat(&r, &s.normalized_endpoint).unwrap();
        assert!(built.body.to_string().contains("opaque-signature"));
        assert!(built
            .body
            .to_string()
            .contains("<think>literal</think>Answer"));
    }
    #[test]
    fn replay_stamp_changes_with_route_key_model_and_endpoint() {
        let s = scope();
        let original = scope_value(&s);
        for i in 0..4 {
            let mut changed = s.clone();
            match i {
                0 => changed.route_revision += 1,
                1 => changed.credential_revision += 1,
                2 => changed.model_id = "different".into(),
                _ => changed.normalized_endpoint = "https://elsewhere.invalid".into(),
            };
            assert_ne!(scope_value(&changed), original);
        }
    }
    #[test]
    fn malformed_private_google_state_fails_closed() {
        assert!(restore(
            &scope(),
            serde_json::json!([{"thoughtSignature":"missing-text"}])
        )
        .is_err());
    }
}
