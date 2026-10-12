//! Adapted provider family/endpoints from Harness adapters.rs, without HTTP/auth.
use super::{ProviderKind,ProtocolError};
#[derive(Clone,Copy)]pub(super) enum ProviderFamily{OpenaiCompatible,Google,Anthropic}
pub(super) fn provider_family(p:ProviderKind)->ProviderFamily{match p{ProviderKind::Google=>ProviderFamily::Google,ProviderKind::Anthropic=>ProviderFamily::Anthropic,_=>ProviderFamily::OpenaiCompatible}}
pub(super) fn endpoint(base:&str,suffix:&str)->String{format!("{}/{}",base.trim_end_matches('/'),suffix.trim_start_matches('/'))}
pub fn normalize_endpoint(p:ProviderKind,value:&str)->Result<String,ProtocolError>{
    let fixed=match p{ProviderKind::Openrouter=>Some("https://openrouter.ai/api/v1"),ProviderKind::Openai=>Some("https://api.openai.com/v1"),ProviderKind::Google=>Some("https://generativelanguage.googleapis.com/v1beta"),ProviderKind::Anthropic=>Some("https://api.anthropic.com/v1"),ProviderKind::CustomOpenai=>None};
    let value=value.trim().trim_end_matches('/');
    if let Some(fixed)=fixed{return if value.is_empty()||value==fixed{Ok(fixed.into())}else{Err(ProtocolError::InvalidEndpoint)}}
    // Restricted ASCII URL subset. Native HTTP integration will use a URL parser.
    let re=regex::Regex::new(r"^(https?)://([A-Za-z0-9.-]+|\[::1\])(?::([0-9]{1,5}))?(/[A-Za-z0-9._~!$&'()*+,;=:@%/-]*)?$").unwrap();
    let c=re.captures(value).ok_or(ProtocolError::InvalidEndpoint)?;
    let host=c[2].to_ascii_lowercase();
    if host.starts_with('.')||host.ends_with('.')||host.contains(".."){return Err(ProtocolError::InvalidEndpoint)}
    if &c[1]=="http"&&!matches!(host.as_str(),"localhost"|"127.0.0.1"|"[::1]"){return Err(ProtocolError::InvalidEndpoint)}
    if c.get(3).is_some_and(|p|p.as_str().parse::<u16>().ok().is_none_or(|p|p==0)){return Err(ProtocolError::InvalidEndpoint)}
    Ok(format!("{}://{}{}{}",&c[1],host,c.get(3).map(|p|format!(":{}",p.as_str())).unwrap_or_default(),c.get(4).map(|p|p.as_str()).unwrap_or_default()))
}
