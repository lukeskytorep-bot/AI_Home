//! Classify completion without copying private provider text into an error.
use super::{ProtocolError, ProviderKind};
use serde_json::Value;
pub(super) fn validate(provider: ProviderKind, payload: &Value) -> Result<(), ProtocolError> {
    if payload.get("error").is_some() {
        return Err(ProtocolError::InvalidResponse);
    }
    let raw_reason = match provider {
        ProviderKind::Google => payload.pointer("/candidates/0/finishReason"),
        ProviderKind::Anthropic => payload.get("stop_reason"),
        _ => payload.pointer("/choices/0/finish_reason"),
    };
    if raw_reason.is_some_and(|v| !v.is_null() && !v.is_string()) {
        return Err(ProtocolError::InvalidResponse);
    }
    let (reason, complete, incomplete, blocked) = match provider {
        ProviderKind::Google => (
            payload
                .pointer("/candidates/0/finishReason")
                .and_then(Value::as_str),
            &["STOP"][..],
            &["MAX_TOKENS"][..],
            &[
                "SAFETY",
                "RECITATION",
                "BLOCKLIST",
                "PROHIBITED_CONTENT",
                "SPII",
                "IMAGE_SAFETY",
                "IMAGE_PROHIBITED_CONTENT",
                "IMAGE_RECITATION",
            ][..],
        ),
        ProviderKind::Anthropic => (
            payload["stop_reason"].as_str(),
            &["end_turn", "stop_sequence"][..],
            &["max_tokens", "pause_turn", "tool_use"][..],
            &["refusal"][..],
        ),
        _ => (
            payload
                .pointer("/choices/0/finish_reason")
                .and_then(Value::as_str),
            &["stop"][..],
            &["length", "tool_calls", "function_call"][..],
            &["content_filter"][..],
        ),
    };
    if provider == ProviderKind::Google
        && payload
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
            .is_some_and(|s| s != "BLOCK_REASON_UNSPECIFIED")
    {
        return Err(ProtocolError::PolicyBlocked);
    }
    if provider != ProviderKind::Google && provider != ProviderKind::Anthropic {
        if payload
            .pointer("/choices/0/message/refusal")
            .is_some_and(|v| !v.is_null() && v.as_str() != Some(""))
        {
            return Err(ProtocolError::PolicyBlocked);
        }
    }
    validate_tools(provider, payload)?;

    if let Some(reason) = reason {
        if blocked.contains(&reason) {
            return Err(ProtocolError::PolicyBlocked);
        }
        if incomplete.contains(&reason) {
            return Err(ProtocolError::OutputIncomplete);
        }
        if !complete.contains(&reason) {
            return Err(ProtocolError::InvalidResponse);
        }
    }
    // Only explicit, supported empty shapes qualify for single recovery.
    // Malformed schemas, refusals and limits never get relabelled as emptiness.
    let empty = match provider {
        ProviderKind::Google => payload
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .is_some_and(|v| v.is_empty()),
        ProviderKind::Anthropic => payload["content"].as_array().is_some_and(|v| v.is_empty()),
        _ => payload
            .pointer("/choices/0/message/content")
            .is_some_and(|v| {
                v.is_null()
                    || v.as_str().is_some_and(|s| s.trim().is_empty())
                    || v.as_array().is_some_and(|a| a.is_empty())
            }),
    };
    if empty {
        return Err(ProtocolError::EmptyResponse);
    }
    Ok(())
}

// Validate tool fields before emptiness. A tool request needs application
// handling, even when a provider also returns text and a normal stop reason.
fn named_object(value: &Value, arguments: &str) -> bool {
    value.is_object()
        && value
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
        && value.get(arguments).is_none_or(Value::is_object)
}
fn validate_tools(provider: ProviderKind, payload: &Value) -> Result<(), ProtocolError> {
    let mut pending = false;
    match provider {
        ProviderKind::Google => {
            if let Some(parts) = payload.pointer("/candidates/0/content/parts") {
                let parts = parts.as_array().ok_or(ProtocolError::InvalidResponse)?;
                for part in parts {
                    let part = part.as_object().ok_or(ProtocolError::InvalidResponse)?;
                    if let Some(call) = part.get("functionCall") {
                        if !named_object(call, "args") {
                            return Err(ProtocolError::InvalidResponse);
                        }
                        pending = true;
                    }
                }
            }
        }
        ProviderKind::Anthropic => {
            if let Some(parts) = payload.get("content") {
                let parts = parts.as_array().ok_or(ProtocolError::InvalidResponse)?;
                for part in parts {
                    if part.get("type").and_then(Value::as_str) == Some("tool_use") {
                        if !named_object(part, "input")
                            || !part.get("input").is_some_and(Value::is_object)
                            || !part
                                .get("id")
                                .and_then(Value::as_str)
                                .is_some_and(|s| !s.is_empty())
                        {
                            return Err(ProtocolError::InvalidResponse);
                        }
                        pending = true;
                    }
                }
            }
        }
        _ => {
            if let Some(calls) = payload.pointer("/choices/0/message/tool_calls") {
                // null means no tools in OpenAI-compatible responses.
                if !calls.is_null() {
                    let calls = calls.as_array().ok_or(ProtocolError::InvalidResponse)?;
                    for call in calls {
                        if !call.is_object() {
                            return Err(ProtocolError::InvalidResponse);
                        }
                    }
                    pending |= !calls.is_empty();
                }
            }
            if let Some(call) = payload.pointer("/choices/0/message/function_call") {
                if !call.is_null() {
                    if !call.is_object() {
                        return Err(ProtocolError::InvalidResponse);
                    }
                    pending = true;
                }
            }
        }
    }
    if pending {
        Err(ProtocolError::OutputIncomplete)
    } else {
        Ok(())
    }
}
