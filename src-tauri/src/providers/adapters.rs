//! Adapted provider family/endpoints from Harness adapters.rs, with URL validation.
use super::{ProviderKind,ProtocolError};
#[derive(Clone,Copy)]pub(super) enum ProviderFamily{OpenaiCompatible,Google,Anthropic}
pub(super) fn provider_family(p:ProviderKind)->ProviderFamily{match p{ProviderKind::Google=>ProviderFamily::Google,ProviderKind::Anthropic=>ProviderFamily::Anthropic,_=>ProviderFamily::OpenaiCompatible}}
pub(super) fn endpoint(base:&str,suffix:&str)->String{format!("{}/{}",base.trim_end_matches('/'),suffix.trim_start_matches('/'))}
pub fn normalize_endpoint(p:ProviderKind,value:&str)->Result<String,ProtocolError>{
    let fixed=match p{ProviderKind::Openrouter=>Some("https://openrouter.ai/api/v1"),ProviderKind::Openai=>Some("https://api.openai.com/v1"),ProviderKind::Google=>Some("https://generativelanguage.googleapis.com/v1beta"),ProviderKind::Anthropic=>Some("https://api.anthropic.com/v1"),ProviderKind::CustomOpenai=>None};
    let value=value.trim().trim_end_matches('/');
    if let Some(fixed)=fixed{return if value.is_empty()||value==fixed{Ok(fixed.into())}else{Err(ProtocolError::InvalidEndpoint)}}
    // Harness URL parser, with strict credential/query/fragment validation.
    if value.chars().any(char::is_control) { return Err(ProtocolError::InvalidEndpoint); }
    let url = reqwest::Url::parse(value).map_err(|_| ProtocolError::InvalidEndpoint)?;
    let local_http = url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost") | Some("127.0.0.1") | Some("[::1]"));
    if url.scheme() != "https" && !local_http || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some() || url.port() == Some(0) {
        return Err(ProtocolError::InvalidEndpoint);
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}
