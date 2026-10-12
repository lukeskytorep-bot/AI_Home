//! Native provider protocols and bounded HTTP. No keyring or frontend IPC yet.
use serde_json::Value;
mod adapters;
mod request_builders;
mod response_parsers;
mod reasoning;
pub mod transport;
mod discovery;
pub use discovery::DiscoveredModel;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ProviderKind {Openrouter,Openai,Google,Anthropic,CustomOpenai}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ScopeStamp {
    pub profile_id:String,pub connection_id:String,pub route_id:String,
    pub route_revision:u64,pub credential_revision:u64,pub model_id:String,
    pub provider:ProviderKind,pub normalized_endpoint:String,
}
#[derive(Clone)]
pub enum ProviderContinuationState {
    OpenRouter(OpenRouterContinuationState),Google(GoogleContinuationState),Anthropic(AnthropicContinuationState),
}
impl ProviderContinuationState {
    fn stamp(&self)->&ScopeStamp{match self{Self::OpenRouter(s)=>&s.stamp,Self::Google(s)=>&s.stamp,Self::Anthropic(s)=>&s.stamp}}
}
#[derive(Clone)]pub struct OpenRouterContinuationState{pub stamp:ScopeStamp,pub reasoning_details:Vec<Value>}
#[derive(Clone)]pub struct GoogleContinuationState{pub stamp:ScopeStamp,pub parts:Vec<GoogleThoughtPart>}
#[derive(Clone)]pub struct AnthropicContinuationState{pub stamp:ScopeStamp,pub blocks:Vec<Value>}
#[derive(Clone)]pub struct GoogleThoughtPart{pub text:String,pub thought:Option<bool>,pub thought_signature:Option<String>}
#[derive(Clone)]pub struct ProviderImage{pub mime_type:String,pub data_base64:String}
#[derive(Clone)]pub struct ProviderMessage{pub role:String,pub content:String,pub images:Vec<ProviderImage>,pub continuation_state:Option<ProviderContinuationState>}
impl ProviderMessage {pub fn text(role:&str,content:&str)->Self{Self{role:role.into(),content:content.into(),images:vec![],continuation_state:None}}}
#[derive(Clone,Copy)]pub enum CustomOpenAiOutputTokenField{MaxTokens,MaxCompletionTokens}
impl CustomOpenAiOutputTokenField{fn as_str(self)->&'static str{match self{Self::MaxTokens=>"max_tokens",Self::MaxCompletionTokens=>"max_completion_tokens"}}}
#[derive(Clone,Default)]pub struct OpenRouterProviderRouting{pub order:Vec<String>,pub only:Vec<String>,pub ignore:Vec<String>,pub allow_fallbacks:Option<bool>}
#[derive(Clone)]pub struct ProviderChatRequest{
    pub provider:ProviderKind,pub model_id:String,pub messages:Vec<ProviderMessage>,pub scope:ScopeStamp,
    pub temperature:Option<f64>,pub max_output_tokens:Option<u32>,
    pub reasoning_effort:Option<String>,pub reasoning_max_tokens:Option<u32>,
    pub reasoning_transport_kind:Option<String>,pub reasoning_transport_value:Option<String>,
    pub custom_output_token_field:Option<CustomOpenAiOutputTokenField>,pub provider_routing:Option<OpenRouterProviderRouting>,
}
#[derive(Default)]pub struct ProviderUsage{pub input_tokens:Option<u64>,pub output_tokens:Option<u64>,pub reasoning_tokens:Option<u64>,pub total_tokens:Option<u64>,pub cost_usd:Option<f64>}
// Deliberately not Serialize or Debug: hidden/provider data is not an IPC DTO.
pub struct ProviderChatResponse {
    content:String,reasoning_content:Option<String>,reasoning_details:Option<Vec<Value>>,reasoning_source:Option<String>,
    finish_reason:Option<String>,actual_model:Option<String>,usage:ProviderUsage,provider_request_id:Option<String>,actual_provider:Option<String>,
    continuation_diagnostics:Option<()>,debug_payload:Option<()>,
}
impl ProviderChatResponse {
    pub fn content(&self)->&str{&self.content}
    pub fn usage(&self)->&ProviderUsage{&self.usage}
    pub fn finish_reason(&self)->Option<&str>{self.finish_reason.as_deref()}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum ProtocolError{InvalidEndpoint,InvalidRequest,RouteMismatch,InvalidResponse}
pub struct PreparedChat{pub endpoint:String,pub body:Value}
/// No secret is accepted by protocol builders; native auth is added later.
pub fn prepare_chat(request:&ProviderChatRequest,endpoint:&str)->Result<PreparedChat,ProtocolError>{
    let base=adapters::normalize_endpoint(request.provider,endpoint)?;
    validate_request(request,&base)?;
    let (endpoint,body)=request_builders::build_chat_request(request,&base).map_err(|_|ProtocolError::InvalidRequest)?;
    Ok(PreparedChat{endpoint,body})
}
pub fn normalize_endpoint(provider:ProviderKind,value:&str)->Result<String,ProtocolError>{adapters::normalize_endpoint(provider,value)}
pub fn parse_response(provider:ProviderKind,payload:Value)->Result<ProviderChatResponse,ProtocolError>{
    // Raw provider errors may echo keys, prompts or private data; keep them internal.
    response_parsers::parse_chat_response(provider,payload,None).map_err(|_|ProtocolError::InvalidResponse)
}
fn validate_request(r:&ProviderChatRequest,base:&str)->Result<(),ProtocolError>{
    let s=&r.scope;
    if s.provider!=r.provider||s.model_id!=r.model_id||s.normalized_endpoint!=base{return Err(ProtocolError::RouteMismatch)}
    if [&s.profile_id,&s.connection_id,&s.route_id].iter().any(|v|v.is_empty()||v.len()>128)||s.route_revision==0||s.credential_revision==0{return Err(ProtocolError::InvalidRequest)}
    if r.model_id.trim().is_empty()||r.model_id.len()>512||r.model_id.chars().any(char::is_control)||r.messages.is_empty()||r.max_output_tokens.is_none_or(|n|n==0){return Err(ProtocolError::InvalidRequest)}
    if r.temperature.is_some_and(|v|!v.is_finite()||!(0.0..=2.0).contains(&v))||r.reasoning_max_tokens==Some(0){return Err(ProtocolError::InvalidRequest)}
    if r.messages.iter().any(|m|!matches!(m.role.as_str(),"system"|"user"|"assistant")||m.content.len()>2*1024*1024){return Err(ProtocolError::InvalidRequest)}
    if r.provider==ProviderKind::Google {
        let model=r.model_id.trim().trim_start_matches("models/");
        if !regex::Regex::new(r"^[A-Za-z0-9._-]+$").unwrap().is_match(model){return Err(ProtocolError::InvalidRequest)}
    }
    for m in &r.messages {
        if let Some(state)=&m.continuation_state {
            let replay_size=match state{ProviderContinuationState::OpenRouter(s)=>serde_json::to_string(&s.reasoning_details).unwrap().len(),ProviderContinuationState::Google(s)=>s.parts.iter().map(|p|p.text.len()+p.thought_signature.as_ref().map(|x|x.len()).unwrap_or(0)).sum(),ProviderContinuationState::Anthropic(s)=>serde_json::to_string(&s.blocks).unwrap().len()};
            if replay_size>2*1024*1024{return Err(ProtocolError::InvalidRequest)}
            let kind_ok=matches!((r.provider,state),(ProviderKind::Openrouter,ProviderContinuationState::OpenRouter(_))|(ProviderKind::Google,ProviderContinuationState::Google(_))|(ProviderKind::Anthropic,ProviderContinuationState::Anthropic(_)));
            if m.role!="assistant"||!kind_ok||state.stamp()!=s{return Err(ProtocolError::RouteMismatch)}
            // Only the reference-supported replay shapes are accepted here.
            match state {
                ProviderContinuationState::Google(state)=>{
                    let visible=state.parts.iter().filter(|p|p.thought!=Some(true)).map(|p|p.text.as_str()).collect::<String>();
                    if visible!=m.content{return Err(ProtocolError::InvalidRequest)}
                }
                ProviderContinuationState::Anthropic(state)=>{
                    if state.blocks.iter().any(|b|!matches!(b["type"].as_str(),Some("thinking")|Some("redacted_thinking"))){return Err(ProtocolError::InvalidRequest)}
                }
                ProviderContinuationState::OpenRouter(state)=>{
                    if state.reasoning_details.iter().any(|v|!v.is_object()){return Err(ProtocolError::InvalidRequest)}
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]mod tests;
