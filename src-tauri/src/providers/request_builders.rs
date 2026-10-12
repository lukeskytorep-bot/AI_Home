// Adapted from AI RV Harness (MIT); see docs/reuse/HARNESS_PROVIDER_LICENSE.txt.
use serde_json::{json, Map, Value};

use super::{ProviderChatRequest, ProviderContinuationState, ProviderKind, ProviderMessage};
use super::adapters::{endpoint, provider_family, ProviderFamily};

pub(super) fn build_chat_request(request: &ProviderChatRequest, base: &str) -> Result<(String, Value), String> {
    match provider_family(request.provider) {
        ProviderFamily::Google => build_google_request(request, base),
        ProviderFamily::Anthropic => Ok(build_anthropic_request(request, base)),
        ProviderFamily::OpenaiCompatible => Ok(build_openai_compatible_request(request, base)),
    }
}

pub(super) fn build_openai_compatible_request(request: &ProviderChatRequest, base: &str) -> (String, Value) {
    let mut body = Map::new();
    body.insert("model".into(), Value::String(request.model_id.clone()));
    body.insert("messages".into(), Value::Array(request.messages.iter().map(|message| openai_message(request.provider, message)).collect()));
    if let Some(value) = request.temperature {
        body.insert("temperature".into(), json!(value));
    }
    if let Some(value) = request.max_output_tokens {
        body.insert(openai_compatible_output_token_field(request).into(), json!(value));
    }
    if matches!(request.provider, ProviderKind::Openrouter) {
        if let Some(value) = request.reasoning_max_tokens {
            body.insert("reasoning".into(), json!({ "max_tokens": value }));
        } else if let Some(value) = request.reasoning_transport_value.as_deref().or(request.reasoning_effort.as_deref()) {
            let kind = request.reasoning_transport_kind.as_deref().unwrap_or("effort");
            if kind == "enabled_boolean" {
                body.insert("reasoning".into(), json!({ "enabled": value == "true" }));
            } else {
                body.insert("reasoning".into(), json!({ "effort": value }));
            }
        }
    } else if let Some(value) = request.reasoning_transport_value.as_deref().or(request.reasoning_effort.as_deref()) {
        body.insert("reasoning_effort".into(), json!(value));
    }
    if matches!(request.provider, ProviderKind::Openrouter) {
        if let Some(routing) = request.provider_routing.as_ref() {
            let mut provider = Map::new();
            if !routing.order.is_empty() {
                provider.insert("order".into(), json!(&routing.order));
            }
            if !routing.only.is_empty() {
                provider.insert("only".into(), json!(&routing.only));
            }
            if !routing.ignore.is_empty() {
                provider.insert("ignore".into(), json!(&routing.ignore));
            }
            if let Some(value) = routing.allow_fallbacks {
                provider.insert("allow_fallbacks".into(), json!(value));
            }
            if !provider.is_empty() {
                body.insert("provider".into(), Value::Object(provider));
            }
        }
    }
    (endpoint(base, "chat/completions"), Value::Object(body))
}

fn openai_compatible_output_token_field(request: &ProviderChatRequest) -> &'static str {
    match request.provider {
        ProviderKind::Openrouter | ProviderKind::Openai => "max_completion_tokens",
        ProviderKind::CustomOpenai => request.custom_output_token_field.unwrap_or(super::CustomOpenAiOutputTokenField::MaxTokens).as_str(),
        ProviderKind::Google | ProviderKind::Anthropic => unreachable!("native provider routed through OpenAI-compatible request builder"),
    }
}

fn openai_message(provider: ProviderKind, message: &ProviderMessage) -> Value {
    let content = if message.images.is_empty() {
        Value::String(message.content.clone())
    } else {
        let mut parts = vec![json!({ "type": "text", "text": message.content })];
        for image in &message.images {
            parts.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{};base64,{}", image.mime_type, image.data_base64) } }));
        }
        Value::Array(parts)
    };
    let mut object = Map::new();
    object.insert("role".into(), Value::String(message.role.clone()));
    object.insert("content".into(), content);
    if matches!(provider, ProviderKind::Openrouter) {
        if let Some(ProviderContinuationState::OpenRouter(state)) = message.continuation_state.as_ref() {
            object.insert("reasoning_details".into(), Value::Array(state.reasoning_details.clone()));
        }
    }
    Value::Object(object)
}

pub(super) fn build_google_request(request: &ProviderChatRequest, base: &str) -> Result<(String, Value), String> {
    let clean_model = request.model_id.trim().trim_start_matches("models/");
    if clean_model.contains('/') || clean_model.contains(':') {
        return Err("invalid Google model id".to_string());
    }
    let system_text = request
        .messages
        .iter()
        .filter(|message| message.role == "system")
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contents = request
        .messages
        .iter()
        .filter(|message| message.role != "system")
        .map(|message| {
            let role = if message.role == "assistant" { "model" } else { "user" };
            let mut parts = if let Some(ProviderContinuationState::Google(state)) = message.continuation_state.as_ref() {
                state.parts.iter().map(|part| {
                    let mut value = Map::new();
                    value.insert("text".into(), Value::String(part.text.clone()));
                    if let Some(thought) = part.thought {
                        value.insert("thought".into(), Value::Bool(thought));
                    }
                    if let Some(signature) = part.thought_signature.as_ref() {
                        value.insert("thoughtSignature".into(), Value::String(signature.clone()));
                    }
                    Value::Object(value)
                }).collect::<Vec<_>>()
            } else {
                vec![json!({ "text": message.content })]
            };
            for image in &message.images {
                parts.push(json!({ "inlineData": { "mimeType": image.mime_type, "data": image.data_base64 } }));
            }
            json!({ "role": role, "parts": parts })
        })
        .collect::<Vec<_>>();
    let mut generation = Map::new();
    if let Some(value) = request.temperature {
        generation.insert("temperature".into(), json!(value));
    }
    if let Some(value) = request.max_output_tokens {
        generation.insert("maxOutputTokens".into(), json!(value));
    }
    if let Some(value) = request.reasoning_transport_value.as_deref().or(request.reasoning_effort.as_deref()) {
        let kind = request.reasoning_transport_kind.as_deref().unwrap_or("thinking_level");
        if kind == "enabled_boolean" {
            generation.insert("thinkingConfig".into(), json!({ "thinkingLevel": if value == "true" { "high" } else { "minimal" } }));
        } else {
            generation.insert("thinkingConfig".into(), json!({ "thinkingLevel": value }));
        }
    }
    let mut body = Map::new();
    body.insert("contents".into(), Value::Array(contents));
    if !system_text.is_empty() {
        body.insert("systemInstruction".into(), json!({ "parts": [{ "text": system_text }] }));
    }
    if !generation.is_empty() {
        body.insert("generationConfig".into(), Value::Object(generation));
    }
    Ok((endpoint(base, &format!("models/{clean_model}:generateContent")), Value::Object(body)))
}

pub(super) fn build_anthropic_request(request: &ProviderChatRequest, base: &str) -> (String, Value) {
    let system_text = request
        .messages
        .iter()
        .filter(|message| message.role == "system")
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let messages = request
        .messages
        .iter()
        .filter(|message| message.role != "system")
        .map(|message| {
            if let Some(ProviderContinuationState::Anthropic(state)) = message.continuation_state.as_ref() {
                let mut blocks = state.blocks.clone();
                blocks.push(json!({ "type": "text", "text": message.content }));
                json!({ "role": message.role, "content": blocks })
            } else if message.images.is_empty() {
                json!({ "role": message.role, "content": message.content })
            } else {
                let mut blocks = vec![json!({ "type": "text", "text": message.content })];
                for image in &message.images {
                    blocks.push(json!({ "type": "image", "source": { "type": "base64", "media_type": image.mime_type, "data": image.data_base64 } }));
                }
                json!({ "role": message.role, "content": blocks })
            }
        })
        .collect::<Vec<_>>();
    let mut body = Map::new();
    body.insert("model".into(), json!(request.model_id));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("max_tokens".into(), json!(request.max_output_tokens.unwrap_or(8192)));
    if !system_text.is_empty() {
        body.insert("system".into(), json!(system_text));
    }
    if let Some(value) = request.temperature {
        body.insert("temperature".into(), json!(value));
    }
    // Exact Anthropic thinking controls vary by model generation. They are deliberately
    // not synthesized here; the dynamic registry currently leaves those levels unknown.
    (endpoint(base, "messages"), Value::Object(body))
}


pub(super) fn enable_openrouter_streaming(body: &mut Value) -> Result<(), String> {
    let object = body.as_object_mut().ok_or_else(|| "OpenRouter chat request body must be an object".to_string())?;
    object.insert("stream".into(), Value::Bool(true));
    Ok(())
}
