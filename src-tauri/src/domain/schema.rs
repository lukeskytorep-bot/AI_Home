use serde_json::Value;
use std::sync::OnceLock;
static DEFINITIONS: OnceLock<Value> = OnceLock::new();
fn definitions() -> &'static Value {
    DEFINITIONS.get_or_init(|| {
        let mut base: Value = serde_json::from_str(include_str!("../../../contracts/domain.schema.json")).expect("embedded P2 schema");
        let extra: Value = serde_json::from_str(include_str!("../../../contracts/foundation.schema.json")).expect("embedded foundation schema");
        base["$defs"].as_object_mut().unwrap().extend(extra["$defs"].as_object().unwrap().clone());
        base["$defs"].clone()
    })
}
pub fn dto(name: &str, value: &Value) -> bool {
    definitions().get(name).is_some_and(|s| valid(value, s))
}
// Deliberately implements only the schema keywords used by the frozen contracts.
pub fn valid(v: &Value, s: &Value) -> bool {
    if let Some(reference) = s["$ref"].as_str() {
        return reference.rsplit('/').next().and_then(|k| definitions().get(k)).is_some_and(|d| valid(v,d));
    }
    if let Some(options) = s["oneOf"].as_array() { return options.iter().filter(|x| valid(v,x)).count()==1; }
    if let Some(c) = s.get("const") { if v != c { return false; } }
    if let Some(options) = s["enum"].as_array() { if !options.contains(v) { return false; } }
    match s["type"].as_str() {
        Some("object") => {
            let Some(o) = v.as_object() else {return false};
            if o.len() < s["minProperties"].as_u64().unwrap_or(0) as usize {return false}
            if s["required"].as_array().is_some_and(|keys| keys.iter().any(|k| !o.contains_key(k.as_str().unwrap()))) {return false}
            o.iter().all(|(k,v)| {
                if let Some(p)=s["properties"].get(k) {valid(v,p)}
                else if s["additionalProperties"]==false {false}
                else if s["additionalProperties"].is_object() {valid(v,&s["additionalProperties"])}
                else {true}
            })
        }
        Some("array") => {
            let Some(a)=v.as_array() else {return false};
            a.len()>=s["minItems"].as_u64().unwrap_or(0) as usize &&
            (!s["uniqueItems"].as_bool().unwrap_or(false)||a.iter().enumerate().all(|(i,x)| !a[..i].contains(x))) &&
            a.iter().all(|x| valid(x,&s["items"]))
        }
        Some("string") => {
            let Some(text)=v.as_str() else{return false};let n=text.chars().count() as u64;
            if n<s["minLength"].as_u64().unwrap_or(0)||n>s["maxLength"].as_u64().unwrap_or(u64::MAX){return false}
            if s["pattern"].as_str().is_some_and(|p| !regex::Regex::new(p).expect("embedded regex").is_match(text)){return false}
            match s["format"].as_str(){
                Some("date-time")=>chrono::DateTime::parse_from_rfc3339(text).is_ok(),
                Some("uri")=>regex::Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:[^\s]+$").unwrap().is_match(text),
                _=>true
            }
        }
        Some("integer") | Some("number") => {
            let Some(n)=v.as_f64() else{return false};
            if s["type"]=="integer" && (n.fract()!=0.0||n.abs()>9_007_199_254_740_991.0){return false}
            n>=s["minimum"].as_f64().unwrap_or(f64::NEG_INFINITY)&&n<=s["maximum"].as_f64().unwrap_or(f64::INFINITY)&&n>s["exclusiveMinimum"].as_f64().unwrap_or(f64::NEG_INFINITY)
        }
        Some("boolean")=>v.is_boolean(),_=>true
    }
}
