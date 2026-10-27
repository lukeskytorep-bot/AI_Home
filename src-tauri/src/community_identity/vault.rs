use super::*;
use std::sync::Arc;
const SERVICE: &str = "org.aihome.app.community.identity.v1";
const PREFIX: &str = "ai-home-community-keyset:v1:";
/// Native secret backend, isolated from provider credentials. Must provide
/// immutable, create-once storage; no plaintext disk fallback is permitted.
pub trait CommunitySecretStore: Send + Sync {
    fn read(&self, reference: &str) -> Result<Option<Zeroizing<String>>, IdentityError>;
    fn create(&self, reference: &str, secret: &str) -> Result<(), IdentityError>;
    fn remove(&self, reference: &str) -> Result<(), IdentityError>;
}
pub struct OsCommunitySecretStore;
impl CommunitySecretStore for OsCommunitySecretStore {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        let e = keyring::Entry::new(SERVICE, r).map_err(|_| IdentityError::VaultUnavailable)?;
        match e.get_password() {
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
pub struct CommunityKeyStore {
    backend: Arc<dyn CommunitySecretStore>,
}
impl CommunityKeyStore {
    pub(super) async fn proof_signature(
        &self, scope: &CommunityScope, expected: &PublicIdentity, proof: &proof::Proof,
    ) -> Result<String, IdentityError> {
        if proof.scope_binding != scope.binding() { return Err(IdentityError::ScopeMismatch); }
        if proof.expires < Utc::now() { return Err(IdentityError::Expired); }
        let backend = self.backend.clone();
        let scope = scope.clone();
        let expected = expected.clone();
        let bytes = proof.bytes().to_vec();
        let expires = proof.expires;
        tokio::task::spawn_blocking(move || {
            let (_, operational, public) = load(backend.as_ref(), &scope, &expected.key_ref)?;
            if public != expected { return Err(IdentityError::ScopeMismatch); }
            if expires < Utc::now() { return Err(IdentityError::Expired); }
            Ok(operational.sign(&bytes))
        }).await.map_err(|_| IdentityError::VaultUnavailable)?
    }
    pub fn os() -> Self {
        Self {
            backend: Arc::new(OsCommunitySecretStore),
        }
    }
    pub fn with_backend(backend: Arc<dyn CommunitySecretStore>) -> Self {
        Self { backend }
    }
    /// Reserve locally, then persist this reference before provisioning/HTTP in 2B.
    pub fn reserve_reference() -> Result<String, IdentityError> {
        use rand_core::RngCore;
        let mut bytes = [0u8; 16];
        rand_core::OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| IdentityError::Entropy)?;
        Ok(format!(
            "{PREFIX}{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ))
    }
    /// Caller must hold exclusive provisioning ownership for the reserved reference.
    /// OS keyring is not a cross-process CAS. Existing entries are never regenerated.
    pub async fn provision(
        &self,
        scope: &CommunityScope,
        reference: &str,
    ) -> Result<PublicIdentity, IdentityError> {
        reference_valid(reference)?;
        let backend = self.backend.clone();
        let scope = scope.clone();
        let reference = reference.to_string();
        tokio::task::spawn_blocking(move||{
   if backend.read(&reference)?.is_some(){return Err(IdentityError::AlreadyExists);}let identity=crypto::PrivateKey::generate()?;let operational=crypto::PrivateKey::generate()?;
   distinct(&identity.public(),&operational.public())?;let a=identity.encode()?;let b=operational.encode()?;
   let record=Zeroizing::new(format!("AI-HOME-COMMUNITY-KEYSET/1\nscope_sha256={}\nidentity_pkcs8={}\noperational_pkcs8={}\n",scope.binding(),a.as_str(),b.as_str()));backend.create(&reference,&record)?;
   let read=backend.read(&reference)?.ok_or(IdentityError::VaultUnavailable)?;if read.as_str()!=record.as_str(){return Err(IdentityError::VaultUnavailable);}let(_,_,public)=load(backend.as_ref(),&scope,&reference)?;Ok(public)
  }).await.map_err(|_|IdentityError::VaultUnavailable)?
    }
    pub async fn load(
        &self,
        scope: &CommunityScope,
        reference: &str,
    ) -> Result<PublicIdentity, IdentityError> {
        let backend = self.backend.clone();
        let scope = scope.clone();
        let reference = reference.to_string();
        tokio::task::spawn_blocking(move || {
            load(backend.as_ref(), &scope, &reference).map(|(_, _, p)| p)
        })
        .await
        .map_err(|_| IdentityError::VaultUnavailable)?
    }
    pub async fn complete_packet(
        &self,
        scope: &CommunityScope,
        registration: &ValidatedRegistration,
        now: DateTime<Utc>,
    ) -> Result<Value, IdentityError> {
        if scope.binding() != registration.scope_binding {
            return Err(IdentityError::ScopeMismatch);
        }
        if now > registration.expires_at {
            return Err(IdentityError::Expired);
        }
        let backend = self.backend.clone();
        let scope = scope.clone();
        let reg = registration.clone();
        tokio::task::spawn_blocking(move||{
   let(a,b,actual)=load(backend.as_ref(),&scope,&reg.key_ref)?;if actual!=reg.expected_public{return Err(IdentityError::ScopeMismatch);}
   Ok(json!({"registration_id":reg.registration_id,"identity_signature":a.sign(reg.statement.as_bytes()),"operational_signature":b.sign(reg.statement.as_bytes())}))
  }).await.map_err(|_|IdentityError::VaultUnavailable)?
    }
    pub async fn remove(
        &self,
        scope: &CommunityScope,
        reference: &str,
    ) -> Result<(), IdentityError> {
        let backend = self.backend.clone();
        let scope = scope.clone();
        let reference = reference.to_string();
        tokio::task::spawn_blocking(move || {
            match load(backend.as_ref(), &scope, &reference) {
                Ok(_) => {}
                Err(IdentityError::NotFound) => return Ok(()),
                Err(e) => return Err(e),
            };
            backend.remove(&reference)?;
            if backend.read(&reference)?.is_some() {
                return Err(IdentityError::VaultUnavailable);
            }
            Ok(())
        })
        .await
        .map_err(|_| IdentityError::VaultUnavailable)?
    }
}
fn load(
    backend: &dyn CommunitySecretStore,
    scope: &CommunityScope,
    reference: &str,
) -> Result<(crypto::PrivateKey, crypto::PrivateKey, PublicIdentity), IdentityError> {
    reference_valid(reference)?;
    let record = backend.read(reference)?.ok_or(IdentityError::NotFound)?;
    if record.len() > 2048 || !record.ends_with('\n') {
        return Err(IdentityError::Invalid);
    }
    let lines: Vec<&str> = record.lines().collect();
    if lines.len() != 4 || lines[0] != "AI-HOME-COMMUNITY-KEYSET/1" {
        return Err(IdentityError::Invalid);
    }
    if lines[1] != format!("scope_sha256={}", scope.binding()) {
        return Err(IdentityError::ScopeMismatch);
    }
    let a = crypto::PrivateKey::decode(
        lines[2]
            .strip_prefix("identity_pkcs8=")
            .ok_or(IdentityError::Invalid)?,
    )?;
    let b = crypto::PrivateKey::decode(
        lines[3]
            .strip_prefix("operational_pkcs8=")
            .ok_or(IdentityError::Invalid)?,
    )?;
    distinct(&a.public(), &b.public())?;
    let public = PublicIdentity {
        key_ref: reference.into(),
        scope_binding: scope.binding(),
        identity_public_key: a.public(),
        operational_public_key: b.public(),
    };
    Ok((a, b, public))
}

pub(super) fn reference_valid(reference: &str) -> Result<(), IdentityError> {
    let suffix = reference
        .strip_prefix(PREFIX)
        .ok_or(IdentityError::Invalid)?;
    if suffix.len() != 32
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(IdentityError::Invalid);
    }
    Ok(())
}
