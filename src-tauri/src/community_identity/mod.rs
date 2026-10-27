//! 4B-12 step 2A: native-only key custody and exact registration packets.
//! The registry submodule adds durable local preparation (2B-1).
//! No HTTP, automatic registration, token acceptance or IPC.
mod crypto;
mod vault;
pub mod registry;
use chrono::{DateTime, SecondsFormat, Utc};
pub use crypto::{
    public_key_fingerprint, verify_signature, ALGORITHM, PUBLIC_FORMAT, SIGNATURE_FORMAT,
};
use serde_json::{json, Value};
pub use vault::{CommunityKeyStore, CommunitySecretStore, OsCommunitySecretStore};
use zeroize::Zeroizing;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    Invalid,
    ScopeMismatch,
    Expired,
    RecoveryRequired,
    VaultUnavailable,
    AlreadyExists,
    NotFound,
    Entropy,
    RequestTooLarge,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityScope {
    profile_id: String,
    api_root: String,
    origin: String,
    instance: String,
}
impl CommunityScope {
    pub fn new(
        profile_id: &str,
        api_root: &str,
        origin: &str,
        instance: &str,
    ) -> Result<Self, IdentityError> {
        if !scalar(profile_id, 256)
            || !scalar(origin, 512)
            || !scalar(instance, 128)
            || instance.len() < 3
            || !instance
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        {
            return Err(IdentityError::Invalid);
        }
        let api = crate::community_connector::normalize_root(api_root)
            .map_err(|_| IdentityError::Invalid)?;
        let origin = origin.trim_end_matches('/');
        let parsed = reqwest::Url::parse(origin).map_err(|_| IdentityError::Invalid)?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || origin.chars().any(char::is_whitespace)
        {
            return Err(IdentityError::Invalid);
        }
        Ok(Self {
            profile_id: profile_id.into(),
            api_root: api.to_string(),
            origin: origin.into(),
            instance: instance.into(),
        })
    }
    pub fn binding(&self) -> String {
        crypto::digest(json!({"format":"community-key-scope/1","profileId":self.profile_id,"apiRoot":self.api_root,"communityOrigin":self.origin,"communityInstance":self.instance}).to_string().as_bytes())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationProfile {
    display_name: String,
    about: String,
}
impl RegistrationProfile {
    pub fn new(display: &str, about: &str) -> Result<Self, IdentityError> {
        let display =
            display.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\0' | '\u{000b}'));
        if !scalar(display, 120) || about.len() > 2048 || about.contains('\0') {
            return Err(IdentityError::Invalid);
        }
        Ok(Self {
            display_name: display.into(),
            about: about.into(),
        })
    }
    pub fn canonical_bytes(&self) -> Vec<u8> {
        format!(
            "COMMUNITY-PROFILE/1\ndisplay_name_b64={}\nabout_b64={}\n",
            crypto::b64(self.display_name.as_bytes()),
            crypto::b64(self.about.as_bytes())
        )
        .into_bytes()
    }
    pub fn sha256(&self) -> String {
        crypto::digest(&self.canonical_bytes())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIdentity {
    key_ref: String,
    scope_binding: String,
    identity_public_key: String,
    operational_public_key: String,
}
impl PublicIdentity {
    pub fn key_reference(&self) -> &str {
        &self.key_ref
    }
    pub fn identity_public_key(&self) -> &str {
        &self.identity_public_key
    }
    pub fn operational_public_key(&self) -> &str {
        &self.operational_public_key
    }
}
#[derive(Debug, Clone)]
pub struct RegistrationIntent {
    scope: CommunityScope,
    identity: PublicIdentity,
    profile: RegistrationProfile,
    client_start_id: String,
}
/// Secret request bytes: contains the invite. No Debug/Display/Serialize.
pub struct StartRequest(Zeroizing<String>);
impl StartRequest {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
/// Constructed only by exact local challenge validation, never from raw statement text.
#[derive(Debug, Clone)]
pub struct ValidatedRegistration {
    scope_binding: String,
    key_ref: String,
    expected_public: PublicIdentity,
    registration_id: String,
    statement: String,
    expires_at: DateTime<Utc>,
}
impl ValidatedRegistration {
    pub fn statement_bytes(&self) -> &[u8] {
        self.statement.as_bytes()
    }
    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }
}
impl RegistrationIntent {
    pub fn new(
        scope: CommunityScope,
        identity: PublicIdentity,
        profile: RegistrationProfile,
        client_start_id: &str,
    ) -> Result<Self, IdentityError> {
        if identity.scope_binding != scope.binding() {
            return Err(IdentityError::ScopeMismatch);
        }
        distinct(
            &identity.identity_public_key,
            &identity.operational_public_key,
        )?;
        if !(16..=128).contains(&client_start_id.len())
            || !client_start_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
        {
            return Err(IdentityError::Invalid);
        }
        Ok(Self {
            scope,
            identity,
            profile,
            client_start_id: client_start_id.into(),
        })
    }
    /// Caller must keep this intent and the key reference durable before any HTTP
    /// in step 2B. Retrying never creates a new start ID or key pair here.
    pub fn start_request(&self, invite: &str) -> Result<StartRequest, IdentityError> {
        if !scalar(invite, 200) {
            return Err(IdentityError::Invalid);
        }
        let invite =
            Zeroizing::new(serde_json::to_string(invite).map_err(|_| IdentityError::Invalid)?);
        let public = json!({"client_start_id":self.client_start_id,"profile":{"display_name":self.profile.display_name,"about":self.profile.about},"identity_public_key":self.identity.identity_public_key,"operational_public_key":self.identity.operational_public_key});
        let text = public.to_string();
        let wire = Zeroizing::new(format!(
            "{{\"invite_token\":{},{}",
            invite.as_str(),
            &text[1..]
        ));
        if wire.len() > 8192 {
            return Err(IdentityError::RequestTooLarge);
        }
        Ok(StartRequest(wire))
    }
    pub fn validate_challenge(
        &self,
        response: &Value,
        now: DateTime<Utc>,
    ) -> Result<ValidatedRegistration, IdentityError> {
        if response.to_string().len() > 16384
            || response["ok"] != true
            || !response["request_id"]
                .as_str()
                .is_some_and(|s| scalar(s, 256))
        {
            return Err(IdentityError::Invalid);
        }
        let r = &response["registration"];
        if r["state"] == "committed" {
            return Err(IdentityError::RecoveryRequired);
        }
        if r["state"] != "pending" || !r["retry"].is_boolean() {
            return Err(IdentityError::Invalid);
        }
        if r["community_origin"] != self.scope.origin
            || r["community_instance"] != self.scope.instance
        {
            return Err(IdentityError::ScopeMismatch);
        }
        for (k, v) in [
            ("profile_format", "COMMUNITY-PROFILE/1"),
            ("signature_algorithm", ALGORITHM),
            ("public_key_format", PUBLIC_FORMAT),
            ("signature_format", SIGNATURE_FORMAT),
            ("protocol_version", "1"),
        ] {
            if r[k] != v {
                return Err(IdentityError::Invalid);
            }
        }
        if r["profile_sha256"] != self.profile.sha256() {
            return Err(IdentityError::Invalid);
        }
        let id = field(r, "registration_id", 128)?;
        if !prefixed(&id, "reg_") {
            return Err(IdentityError::Invalid);
        }
        let nonce = field(r, "nonce", 128)?;
        if crypto::decode(&nonce, 32)?.len() != 32 {
            return Err(IdentityError::Invalid);
        }
        let expires = field(r, "expires_at", 64)?;
        let deadline = DateTime::parse_from_rfc3339(&expires)
            .map_err(|_| IdentityError::Invalid)?
            .with_timezone(&Utc);
        if deadline.to_rfc3339_opts(SecondsFormat::Secs, true) != expires {
            return Err(IdentityError::Invalid);
        }
        if deadline < now {
            return Err(IdentityError::Expired);
        }
        if deadline > now + chrono::Duration::hours(24) {
            return Err(IdentityError::Invalid);
        }
        let statement = registration_statement(
            &self.scope,
            &self.identity,
            &self.profile,
            &id,
            &nonce,
            &expires,
        );
        if r["statement"] != statement {
            return Err(IdentityError::Invalid);
        }
        Ok(ValidatedRegistration {
            scope_binding: self.scope.binding(),
            key_ref: self.identity.key_ref.clone(),
            expected_public: self.identity.clone(),
            registration_id: id,
            statement,
            expires_at: deadline,
        })
    }
}
fn registration_statement(
    scope: &CommunityScope,
    keys: &PublicIdentity,
    p: &RegistrationProfile,
    id: &str,
    nonce: &str,
    expires: &str,
) -> String {
    format!("AGENT-REGISTRATION/1\ncommunity_origin={}\ncommunity_instance={}\nregistration_id={}\nnonce={}\nexpires_at={}\nidentity_public_key={}\noperational_public_key={}\nprofile_format=COMMUNITY-PROFILE/1\nprofile_sha256={}\nsignature_algorithm={}\npublic_key_format={}\nsignature_format={}\nprotocol_version=1\n",scope.origin,scope.instance,id,nonce,expires,keys.identity_public_key,keys.operational_public_key,p.sha256(),ALGORITHM,PUBLIC_FORMAT,SIGNATURE_FORMAT)
}
fn scalar(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.contains(['\n', '\r', '\0'])
}
fn field(v: &Value, k: &str, max: usize) -> Result<String, IdentityError> {
    v[k].as_str()
        .filter(|s| scalar(s, max))
        .map(str::to_string)
        .ok_or(IdentityError::Invalid)
}
fn prefixed(s: &str, p: &str) -> bool {
    s.strip_prefix(p).is_some_and(|x| {
        x.len() == 32
            && x.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn distinct(a: &str, b: &str) -> Result<(), IdentityError> {
    if public_key_fingerprint(a)? == public_key_fingerprint(b)? {
        Err(IdentityError::Invalid)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests;
