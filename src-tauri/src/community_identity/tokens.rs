//! Immutable candidates in a namespace independent of keys and provider secrets.
use super::*;
use std::sync::Arc;
const SERVICE: &str = "org.aihome.app.community.tokens.v1";
const PREFIX: &str = "ai-home-community-token:v1:";
pub struct SecretToken(Zeroizing<String>);
impl SecretToken {
    pub(super) fn new(token: String) -> Result<Self, IdentityError> {
        let token = Zeroizing::new(token);
        let suffix = token.strip_prefix("eqai_").ok_or(IdentityError::Invalid)?;
        if suffix.len() != 43 || crypto::decode(suffix, 32)?.len() != 32 {
            return Err(IdentityError::Invalid);
        }
        Ok(Self(token))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
struct OsTokenBackend;
impl CommunitySecretStore for OsTokenBackend {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        let entry = keyring::Entry::new(SERVICE, r).map_err(|_| IdentityError::VaultUnavailable)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(Zeroizing::new(v))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(IdentityError::VaultUnavailable),
        }
    }
    fn create(&self, r: &str, s: &str) -> Result<(), IdentityError> {
        if self.read(r)?.is_some() {
            return Err(IdentityError::AlreadyExists);
        }
        keyring::Entry::new(SERVICE, r)
            .map_err(|_| IdentityError::VaultUnavailable)?
            .set_password(s)
            .map_err(|_| IdentityError::VaultUnavailable)
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        match keyring::Entry::new(SERVICE, r)
            .map_err(|_| IdentityError::VaultUnavailable)?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(IdentityError::VaultUnavailable),
        }
    }
}
#[derive(Clone)]
pub struct CommunityTokenStore {
    backend: Arc<dyn CommunitySecretStore>,
}
impl CommunityTokenStore {
    pub fn os() -> Self {
        Self {
            backend: Arc::new(OsTokenBackend),
        }
    }
    pub fn with_backend(backend: Arc<dyn CommunitySecretStore>) -> Self {
        Self { backend }
    }
    pub(super) fn reserve() -> String {
        format!("{PREFIX}{}", uuid::Uuid::new_v4().simple())
    }
    pub(super) async fn save(
        &self,
        scope: &CommunityScope,
        reference: &str,
        id: &str,
        agent: &str,
        generation: i64,
        token: SecretToken,
    ) -> Result<(), IdentityError> {
        validate_ref(reference)?;
        let backend = self.backend.clone();
        let scope = scope.clone();
        let reference = reference.to_string();
        let id = id.to_string();
        let agent = agent.to_string();
        tokio::task::spawn_blocking(move||{
            if !prefixed(&id,"reg_")||!prefixed(&agent,"agt_")||generation<1{return Err(IdentityError::Invalid);}
            let record=Zeroizing::new(format!("AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={}\nregistration_id={id}\nagent_id={agent}\ngeneration={generation}\ntoken={}\n",scope.binding(),token.as_str()));
            backend.create(&reference,&record)?;
            let read=backend.read(&reference)?.ok_or(IdentityError::VaultUnavailable)?;if read.as_str()!=record.as_str(){return Err(IdentityError::VaultUnavailable);}Ok(())
        }).await.map_err(|_|IdentityError::VaultUnavailable)?
    }
    pub async fn load(
        &self,
        scope: &CommunityScope,
        reference: &str,
        id: &str,
        agent: &str,
        generation: i64,
    ) -> Result<SecretToken, IdentityError> {
        if !prefixed(id, "reg_") || !prefixed(agent, "agt_") || generation < 1 {
            return Err(IdentityError::Invalid);
        }
        validate_ref(reference)?;
        let backend = self.backend.clone();
        let binding = scope.binding();
        let reference = reference.to_string();
        let id = id.to_string();
        let agent = agent.to_string();
        tokio::task::spawn_blocking(move||{
            let record=backend.read(&reference)?.ok_or(IdentityError::NotFound)?;
            let prefix=format!("AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={binding}\nregistration_id={id}\nagent_id={agent}\ngeneration={generation}\ntoken=");
            if record.len()>1024{return Err(IdentityError::Invalid);}
            let token=record.strip_prefix(&prefix).and_then(|s|s.strip_suffix('\n')).ok_or(IdentityError::ScopeMismatch)?;
            SecretToken::new(token.into())
        }).await.map_err(|_|IdentityError::VaultUnavailable)?
    }
    /// Low-level lifecycle primitive; host must exclude in-flight writers first.
    pub async fn remove(
        &self,
        scope: &CommunityScope,
        reference: &str,
        id: &str,
        agent: &str,
        generation: i64,
    ) -> Result<(), IdentityError> {
        match self.load(scope, reference, id, agent, generation).await {
            Ok(_) => {}
            Err(IdentityError::NotFound) => return Ok(()),
            Err(e) => return Err(e),
        }
        let backend = self.backend.clone();
        let r = reference.to_string();
        tokio::task::spawn_blocking(move || {
            backend.remove(&r)?;
            if backend.read(&r)?.is_some() {
                return Err(IdentityError::VaultUnavailable);
            }
            Ok(())
        })
        .await
        .map_err(|_| IdentityError::VaultUnavailable)?
    }
}
pub(super) fn validate_ref(r: &str) -> Result<(), IdentityError> {
    if r.strip_prefix(PREFIX).is_some_and(|s| {
        s.len() == 32
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        Ok(())
    } else {
        Err(IdentityError::Invalid)
    }
}
