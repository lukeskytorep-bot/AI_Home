// Adapted from AI RV Harness (MIT); see docs/reuse/HARNESS_PROVIDER_LICENSE.txt.
use serde_json::Value;

use super::{ProviderChatResponse, ProviderKind, ProviderUsage};
use super::adapters::{provider_family, ProviderFamily};
use super::reasoning::{empty_response_error_with_reasoning, extract_openai_reasoning, extract_openai_text, normalize_reasoning_response};

pub(super) fn parse_chat_response(
    provider: ProviderKind,
    payload: Value,
    request_id: Option<String>,
) -> Result<ProviderChatResponse, String> {
    match provider_family(provider) {
        ProviderFamily::Google => parse_google_response(payload, request_id),
        ProviderFamily::Anthropic => parse_anthropic_response(payload, request_id),
        ProviderFamily::OpenaiCompatible => parse_openai_compatible_response(payload, request_id),
    }
}

pub(super) fn parse_openai_compatible_response(payload: Value, request_id: Option<String>) -> Result<ProviderChatResponse, String> {
    if let Some(error) = provider_payload_error(&payload) {
        return Err(error);
    }
    let actual_model = payload.get("model").and_then(Value::as_str).map(str::to_string);
    let actual_provider = selected_openrouter_provider(&payload);
    let finish_reason = payload.pointer("/choices/0/finish_reason").and_then(Value::as_str).map(str::to_string);
    let message = payload.pointer("/choices/0/message").unwrap_or(&Value::Null);
    let raw_content = extract_openai_text(message.get("content"));
    let reasoning_details = match message.get("reasoning_details") {
        None | Some(Value::Null) => None,
        Some(Value::Array(details)) => Some(details.clone()),
        Some(_) => return Err("provider response reasoning_details must be an array or null when present".to_string()),
    };
    let (native_reasoning, native_source) = extract_openai_reasoning(message, reasoning_details.as_deref());
    let normalized = normalize_reasoning_response(raw_content, native_reasoning, native_source);
    let content = normalized.content;
    if content.trim().is_empty() {
        return Err(empty_response_error_with_reasoning(
            finish_reason.as_deref(),
            None,
            normalized.reasoning_content.as_deref(),
            normalized.incomplete_tag,
        ));
    }
    let usage = payload.get("usage").unwrap_or(&Value::Null);
    let reasoning_tokens = usage
        .pointer("/completion_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("reasoning_tokens").and_then(Value::as_u64));
    Ok(ProviderChatResponse {
        content,
        reasoning_content: normalized.reasoning_content,
        reasoning_details,
        reasoning_source: normalized.reasoning_source,
        finish_reason,
        actual_model,
        usage: ProviderUsage {
            input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
            output_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
            reasoning_tokens,
            total_tokens: usage.get("total_tokens").and_then(Value::as_u64),
            cost_usd: usage.get("cost").and_then(Value::as_f64),
        },
        provider_request_id: request_id,
        actual_provider,
        continuation_diagnostics: None,
        debug_payload: None,
    })
}

fn selected_openrouter_provider(payload: &Value) -> Option<String> {
    payload.pointer("/openrouter_metadata/endpoints/available")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item.get("selected").and_then(Value::as_bool) == Some(true)))
        .and_then(|item| item.get("provider").and_then(Value::as_str))
        .or_else(|| payload.get("provider").and_then(Value::as_str))
        .map(str::to_string)
}

pub(super) fn parse_google_response(payload: Value, request_id: Option<String>) -> Result<ProviderChatResponse, String> {
    if let Some(error) = provider_payload_error(&payload) {
        return Err(error);
    }
    let actual_model = payload.get("modelVersion").and_then(Value::as_str).map(str::to_string);
    let finish_reason = payload.pointer("/candidates/0/finishReason").and_then(Value::as_str).map(str::to_string);
    let block_reason = payload.pointer("/promptFeedback/blockReason").and_then(Value::as_str);
    let parts = payload
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let raw_content = parts
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let native_reasoning = parts
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) == Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let has_continuation_state = parts.iter().any(|part| {
        part.get("thought").and_then(Value::as_bool) == Some(true)
            || part.get("thoughtSignature").and_then(Value::as_str).is_some_and(|value| !value.is_empty())
    });
    // Google native replay must preserve the complete supported model Part sequence,
    // not only hidden thought parts. Gemini 3 can attach thoughtSignature directly
    // to the visible text Part, as observed in the live Harness fixture.
    let reasoning_details = if has_continuation_state {
        parts.clone()
    } else {
        Vec::new()
    };
    let normalized = normalize_reasoning_response(
        raw_content.clone(),
        (!native_reasoning.trim().is_empty()).then_some(native_reasoning),
        Some("google_thought_parts".to_string()),
    );
    // Native continuation must replay the original visible text alongside its
    // Part sequence. Literal reasoning tags are signed content, not markup to strip.
    let content = if has_continuation_state { raw_content } else { normalized.content };
    if content.trim().is_empty() {
        return Err(empty_response_error_with_reasoning(
            finish_reason.as_deref(),
            block_reason,
            normalized.reasoning_content.as_deref(),
            normalized.incomplete_tag,
        ));
    }
    let usage = payload.get("usageMetadata").unwrap_or(&Value::Null);
    Ok(ProviderChatResponse {
        content,
        reasoning_content: normalized.reasoning_content,
        reasoning_details: (!reasoning_details.is_empty()).then_some(reasoning_details),
        reasoning_source: normalized.reasoning_source,
        finish_reason,
        actual_model,
        usage: ProviderUsage {
            input_tokens: usage.get("promptTokenCount").and_then(Value::as_u64),
            output_tokens: usage.get("candidatesTokenCount").and_then(Value::as_u64),
            reasoning_tokens: usage.get("thoughtsTokenCount").and_then(Value::as_u64),
            total_tokens: usage.get("totalTokenCount").and_then(Value::as_u64),
            cost_usd: None,
        },
        provider_request_id: request_id,
        actual_provider: None,
        continuation_diagnostics: None,
        debug_payload: None,
    })
}

pub(super) fn parse_anthropic_response(payload: Value, request_id: Option<String>) -> Result<ProviderChatResponse, String> {
    if let Some(error) = provider_payload_error(&payload) {
        return Err(error);
    }
    let actual_model = payload.get("model").and_then(Value::as_str).map(str::to_string);
    let finish_reason = payload.get("stop_reason").and_then(Value::as_str).map(str::to_string);
    let blocks = payload.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
    let raw_content = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let native_reasoning = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("thinking"))
        .filter_map(|block| block.get("thinking").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    let has_continuation_state = blocks.iter().any(|block| {
        matches!(block.get("type").and_then(Value::as_str), Some("thinking") | Some("redacted_thinking"))
    });
    if has_continuation_state {
        let mut saw_text = false;
        let mut text_blocks = 0usize;
        for block in &blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("thinking") | Some("redacted_thinking") if !saw_text => {}
                Some("text") if block.as_object().is_some_and(|object| object.len() == 2)
                    && block.get("text").and_then(Value::as_str).is_some() => {
                    saw_text = true;
                    text_blocks += 1;
                }
                _ => return Err("unsupported Anthropic continuation block layout".to_string()),
            }
        }
        if text_blocks != 1 {
            return Err("unsupported Anthropic continuation block layout".to_string());
        }
    }
    let reasoning_details = blocks
        .iter()
        .filter(|block| matches!(block.get("type").and_then(Value::as_str), Some("thinking") | Some("redacted_thinking")))
        .cloned()
        .collect::<Vec<_>>();
    let normalized = normalize_reasoning_response(
        raw_content.clone(),
        (!native_reasoning.trim().is_empty()).then_some(native_reasoning),
        Some("anthropic_thinking".to_string()),
    );
    // A signed Anthropic assistant turn must be replayed with the same text block value.
    // Do not apply tagged-content normalization once native continuation blocks exist.
    let content = if has_continuation_state { raw_content } else { normalized.content };
    if content.trim().is_empty() {
        return Err(empty_response_error_with_reasoning(
            finish_reason.as_deref(),
            None,
            normalized.reasoning_content.as_deref(),
            normalized.incomplete_tag,
        ));
    }
    let usage = payload.get("usage").unwrap_or(&Value::Null);
    let input = usage.get("input_tokens").and_then(Value::as_u64);
    let output = usage.get("output_tokens").and_then(Value::as_u64);
    Ok(ProviderChatResponse {
        content,
        reasoning_content: normalized.reasoning_content,
        reasoning_details: (!reasoning_details.is_empty()).then_some(reasoning_details),
        reasoning_source: normalized.reasoning_source,
        finish_reason,
        actual_model,
        usage: ProviderUsage {
            input_tokens: input,
            output_tokens: output,
            reasoning_tokens: None,
            total_tokens: match (input, output) {
                (Some(a), Some(b)) => a.checked_add(b),
                _ => None,
            },
            cost_usd: None,
        },
        provider_request_id: request_id,
        actual_provider: None,
        continuation_diagnostics: None,
        debug_payload: None,
    })
}


fn provider_payload_error(payload: &Value) -> Option<String> {
    let error = payload.get("error")?;
    let message = error.get("message").and_then(Value::as_str)
        .or_else(|| error.as_str())
        .unwrap_or("provider returned an error payload");
    let code = error.get("code").and_then(|value| value.as_str().map(str::to_string).or_else(|| value.as_i64().map(|number| number.to_string())));
    let kind = error.get("type").and_then(Value::as_str);
    let safe_message = message.chars().take(500).collect::<String>();
    Some(format!(
        "provider error payload{}{}: {}",
        code.as_deref().map(|value| format!(" code={value}")).unwrap_or_default(),
        kind.map(|value| format!(" type={value}")).unwrap_or_default(),
        safe_message,
    ))
}
