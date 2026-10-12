// AI Home discovery adapter; Harness provider_discover_models endpoint/auth flow
// adapted for strict parsing, bounded pagination and per-operation cancellation.
use super::transport::{BoundCredential, CallBudget, HttpTransport, TransportError};
use super::{ProviderKind, ScopeStamp};
use serde_json::Value;
use std::collections::HashSet;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredModel {
    pub id: String,
    pub label: String,
}
impl HttpTransport {
    /// Complete within the shared budget or explicit failure; no silent truncation.
    /// Availability is not proof of chat/vision/reasoning capability or a test attestation.
    pub async fn discover_models(
        &self,
        scope: &ScopeStamp,
        credential: &BoundCredential,
        budget: &CallBudget,
    ) -> Result<Vec<DiscoveredModel>, TransportError> {
        let mut models = Vec::new();
        let mut ids = HashSet::new();
        let mut cursors = HashSet::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut query = Vec::new();
            match scope.provider {
                ProviderKind::Google => {
                    query.push(("pageSize".into(), "1000".into()));
                    if let Some(token) = &cursor {
                        query.push(("pageToken".into(), token.clone()));
                    }
                }
                ProviderKind::Anthropic => {
                    query.push(("limit".into(), "1000".into()));
                    if let Some(token) = &cursor {
                        query.push(("after_id".into(), token.clone()));
                    }
                }
                _ => {
                    if let Some(token) = &cursor {
                        query.push(("after".into(), token.clone()));
                    }
                }
            }
            let payload = self.execute_model_page(scope, credential, &query, budget).await?;
            let (page, next) = parse_models(scope.provider, &payload)?;
            for model in page {
                if !ids.insert(model.id.clone()) {
                    return Err(TransportError::InvalidModelList);
                }
                models.push(model);
                if models.len() > 4000 {
                    return Err(TransportError::DiscoveryIncomplete);
                }
            }
            budget.ensure_active()?;
            let Some(next) = next else {
                return Ok(models);
            };
            if !cursors.insert(next.clone()) {
                return Err(TransportError::InvalidModelList);
            }
            if budget.physical_calls() >= 4 {
                return Err(TransportError::DiscoveryIncomplete);
            }
            cursor = Some(next);
        }
    }
}
fn text<'a>(value: &'a Value, field: &str, maximum: usize) -> Result<&'a str, TransportError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty() && s.len() <= maximum && !s.chars().any(char::is_control))
        .ok_or(TransportError::InvalidModelList)
}
fn parse_models(
    provider: ProviderKind,
    value: &Value,
) -> Result<(Vec<DiscoveredModel>, Option<String>), TransportError> {
    if value.get("error").is_some() {
        return Err(TransportError::InvalidModelList);
    }
    let array_key = if provider == ProviderKind::Google {
        "models"
    } else {
        "data"
    };
    let entries = value
        .get(array_key)
        .and_then(Value::as_array)
        .ok_or(TransportError::InvalidModelList)?;
    if entries.len() > 4000 {
        return Err(TransportError::DiscoveryIncomplete);
    }
    let mut models = Vec::new();
    for entry in entries {
        let id = text(
            entry,
            if provider == ProviderKind::Google {
                "name"
            } else {
                "id"
            },
            512,
        )?;
        if provider == ProviderKind::Google {
            let methods = entry
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .ok_or(TransportError::InvalidModelList)?;
            if methods.iter().any(|v| !v.is_string()) {
                return Err(TransportError::InvalidModelList);
            }
            if !methods
                .iter()
                .any(|v| v.as_str() == Some("generateContent"))
            {
                continue;
            }
            if !id.starts_with("models/")
                || id[7..].is_empty()
                || !id[7..]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                return Err(TransportError::InvalidModelList);
            }
        }
        let label_key = match provider {
            ProviderKind::Google => "displayName",
            ProviderKind::Anthropic => "display_name",
            _ => "name",
        };
        let label = if entry.get(label_key).is_some() {
            text(entry, label_key, 1024)?
        } else {
            id
        };
        models.push(DiscoveredModel {
            id: id.into(),
            label: label.into(),
        });
    }
    let next = match provider {
        ProviderKind::Google => match value.get("nextPageToken") {
            None => None,
            Some(Value::String(s)) if s.is_empty() => None,
            Some(_) => Some(text(value, "nextPageToken", 4096)?.to_string()),
        },
        ProviderKind::Anthropic => match value.get("has_more").and_then(Value::as_bool) {
            Some(false) => None,
            Some(true) if !entries.is_empty() => Some(text(value, "last_id", 512)?.into()),
            _ => return Err(TransportError::InvalidModelList),
        },
        _ => match value.get("has_more") {
            None | Some(Value::Bool(false)) => None,
            Some(Value::Bool(true)) if !entries.is_empty() => {
                Some(text(entries.last().unwrap(), "id", 512)?.into())
            }
            _ => return Err(TransportError::InvalidModelList),
        },
    };
    Ok((models, next))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn google_filters_embedding_and_preserves_cursor() {
        let (models, next) = parse_models(ProviderKind::Google, &json!({"models":[
            {"name":"models/text", "displayName":"Text", "supportedGenerationMethods":["generateContent"]},
            {"name":"models/embed", "supportedGenerationMethods":["embedContent"]}
        ], "nextPageToken":"page/2?opaque"})).unwrap();
        assert_eq!(
            models,
            vec![DiscoveredModel {
                id: "models/text".into(),
                label: "Text".into()
            }]
        );
        assert_eq!(next.as_deref(), Some("page/2?opaque"));
    }
    #[test]
    fn model_formats_and_pagination() {
        for p in [
            ProviderKind::Openai,
            ProviderKind::Openrouter,
            ProviderKind::CustomOpenai,
        ] {
            assert_eq!(
                parse_models(p, &json!({"data":[{"id":"m"}]})).unwrap().0[0].label,
                "m"
            );
        }
        let (models, next) = parse_models(
            ProviderKind::Anthropic,
            &json!({"data":[{"id":"m", "display_name":"Claude"}],"has_more":true,"last_id":"m"}),
        )
        .unwrap();
        assert_eq!(models[0].label, "Claude");
        assert_eq!(next.as_deref(), Some("m"));
    }
    #[test]
    fn invalid_lists_do_not_become_empty_success() {
        for payload in [
            json!({}),
            json!({"data":null}),
            json!({"error":{"message":"private"}}),
            json!({"data":[{}]}),
            json!({"data":[],"has_more":true}),
        ] {
            assert!(parse_models(ProviderKind::Openai, &payload).is_err());
        }
        assert!(parse_models(ProviderKind::Anthropic, &json!({"data":[]})).is_err());
        assert!(parse_models(
            ProviderKind::Google,
            &json!({"models":[{"name":"models/m","supportedGenerationMethods":null}]})
        )
        .is_err());
    }
}
