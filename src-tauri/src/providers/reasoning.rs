// Adapted from AI RV Harness (MIT); see docs/reuse/HARNESS_PROVIDER_LICENSE.txt.
use serde_json::Value;

pub(super) fn extract_openai_text(content: Option<&Value>) -> String {
    extract_final_text_parts(content)
}

fn extract_reasoning_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(text) => Some(text.as_str()),
                Value::Object(_) => part.get("text").and_then(Value::as_str)
                    .or_else(|| part.pointer("/text/value").and_then(Value::as_str))
                    .or_else(|| part.get("thinking").and_then(Value::as_str))
                    .or_else(|| part.get("reasoning").and_then(Value::as_str)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn extract_final_text_parts(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                Value::String(text) => Some(text.as_str()),
                Value::Object(_) => {
                    let kind = part.get("type").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
                    let reasoning_part = kind.contains("reason") || kind.contains("think") || kind.contains("thought");
                    if reasoning_part {
                        None
                    } else {
                        part.get("text").and_then(Value::as_str)
                            .or_else(|| part.pointer("/text/value").and_then(Value::as_str))
                            .or_else(|| part.get("thinking").and_then(Value::as_str))
                            .or_else(|| part.get("reasoning").and_then(Value::as_str))
                    }
                },
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

#[derive(Debug)]
pub(super) struct NormalizedReasoningResponse {
    pub(super) content: String,
    pub(super) reasoning_content: Option<String>,
    pub(super) reasoning_source: Option<String>,
    pub(super) incomplete_tag: bool,
}

pub(super) fn extract_openai_reasoning(message: &Value, details: Option<&[Value]>) -> (Option<String>, Option<String>) {
    for (field, source) in [
        ("reasoning", "openai_reasoning"),
        ("reasoning_content", "openai_reasoning_content"),
        ("thinking", "openai_thinking"),
    ] {
        let text = extract_reasoning_text(message.get(field));
        if !text.trim().is_empty() {
            return (Some(text), Some(source.to_string()));
        }
    }
    let embedded = extract_embedded_reasoning(message.get("content"));
    if !embedded.trim().is_empty() {
        return (Some(embedded), Some("openai_reasoning_details".to_string()));
    }
    let text = details
        .unwrap_or_default()
        .iter()
        .filter_map(|detail| detail.get("text").and_then(Value::as_str)
            .or_else(|| detail.pointer("/summary/0/text").and_then(Value::as_str)))
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        (None, None)
    } else {
        (Some(text), Some("openai_reasoning_details".to_string()))
    }
}

fn extract_embedded_reasoning(content: Option<&Value>) -> String {
    content
        .and_then(Value::as_array)
        .map(|parts| parts
            .iter()
            .filter(|part| {
                let kind = part.get("type").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
                kind.contains("reason") || kind.contains("think") || kind.contains("thought")
            })
            .filter_map(|part| part.get("text").and_then(Value::as_str)
                .or_else(|| part.pointer("/text/value").and_then(Value::as_str))
                .or_else(|| part.get("thinking").and_then(Value::as_str))
                .or_else(|| part.get("reasoning").and_then(Value::as_str)))
            .collect::<Vec<_>>()
            .join(""))
        .unwrap_or_default()
}

pub(super) fn normalize_reasoning_response(
    raw_content: String,
    native_reasoning: Option<String>,
    native_source: Option<String>,
) -> NormalizedReasoningResponse {
    let tagged = split_tagged_reasoning(&raw_content);
    let has_native_reasoning = native_reasoning.as_deref().is_some_and(|value| !value.trim().is_empty());
    NormalizedReasoningResponse {
        content: tagged.as_ref().map(|value| value.content.clone()).unwrap_or(raw_content),
        reasoning_content: if has_native_reasoning {
            native_reasoning
        } else {
            tagged.as_ref().and_then(|value| (!value.reasoning.trim().is_empty()).then(|| value.reasoning.clone()))
        },
        reasoning_source: if has_native_reasoning {
            native_source
        } else if tagged.is_some() {
            Some("tagged_content".to_string())
        } else {
            None
        },
        incomplete_tag: tagged.as_ref().is_some_and(|value| value.incomplete),
    }
}

#[derive(Debug)]
pub(super) struct TaggedReasoningSplit {
    pub(super) content: String,
    pub(super) reasoning: String,
    pub(super) incomplete: bool,
}

pub(super) fn split_tagged_reasoning(value: &str) -> Option<TaggedReasoningSplit> {
    const TAGS: [(&str, &str); 6] = [
        ("<think>", "</think>"),
        ("<thinking>", "</thinking>"),
        ("<reason>", "</reason>"),
        ("<reasoning>", "</reasoning>"),
        ("<thought>", "</thought>"),
        ("<|begin_of_thought|>", "<|end_of_thought|>"),
    ];
    let lower = value.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut content = String::new();
    let mut reasoning_parts = Vec::new();
    let mut found = false;
    let mut incomplete = false;

    while cursor < value.len() {
        let next = TAGS.iter().filter_map(|(open, close)| {
            lower[cursor..].find(*open).map(|offset| (cursor + offset, *open, *close))
        }).min_by_key(|entry| entry.0);
        let Some((start, open, close)) = next else {
            content.push_str(&value[cursor..]);
            break;
        };
        found = true;
        content.push_str(&value[cursor..start]);
        let reasoning_start = start + open.len();
        if let Some(close_offset) = lower[reasoning_start..].find(close) {
            let reasoning_end = reasoning_start + close_offset;
            reasoning_parts.push(value[reasoning_start..reasoning_end].to_string());
            cursor = reasoning_end + close.len();
        } else {
            reasoning_parts.push(value[reasoning_start..].to_string());
            cursor = value.len();
            incomplete = true;
        }
    }

    found.then(|| TaggedReasoningSplit {
        content,
        reasoning: reasoning_parts.join("\n\n"),
        incomplete,
    })
}

fn empty_response_error(finish_reason: Option<&str>, block_reason: Option<&str>) -> String {
    if let Some(reason) = block_reason {
        return format!("provider blocked the response [block-reason={reason}]");
    }
    match finish_reason {
        Some(reason) if matches!(reason.to_ascii_lowercase().as_str(), "content_filter" | "safety" | "blocked" | "recitation") =>
            format!("provider blocked the response [finish-reason={reason}]"),
        Some(reason) => format!("provider returned an empty assistant response [finish-reason={reason}]"),
        None => "provider returned an empty assistant response".to_string(),
    }
}

pub(super) fn empty_response_error_with_reasoning(
    finish_reason: Option<&str>,
    block_reason: Option<&str>,
    reasoning_content: Option<&str>,
    incomplete_tag: bool,
) -> String {
    if block_reason.is_some() {
        return empty_response_error(finish_reason, block_reason);
    }
    if incomplete_tag || reasoning_content.is_some_and(|value| !value.trim().is_empty()) {
        return match finish_reason {
            Some(reason) => format!("provider returned reasoning without a final assistant response [finish-reason={reason}]"),
            None => "provider returned reasoning without a final assistant response".to_string(),
        };
    }
    empty_response_error(finish_reason, block_reason)
}
