// Adapted from AI RV Harness secrets.rs (MIT): versioned binding + fresh readback.
// AI Home uses its own namespace and immutable entries; no legacy secret fallback.
use crate::providers::{transport::BoundCredential, ProviderKind, ScopeStamp};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
const SERVICE: &str = "org.aihome.app.credentials.v1";
const PREFIX: &str = "ai-home-credential:v1:";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultError {
    Unavailable,
    Invalid,
    Exists,
}
pub trait SecretStore: Send + Sync {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError>;
    fn write_new(&self, id: &str, value: &str) -> Result<(), VaultError>;
}
pub struct OsSecretStore;
impl SecretStore for OsSecretStore {
    fn read(&self, id: &str) -> Result<Option<String>, VaultError> {
        let entry = keyring::Entry::new(SERVICE, id).map_err(|_| VaultError::Unavailable)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VaultError::Unavailable),
        }
    }
    fn write_new(&self, id: &str, value: &str) -> Result<(), VaultError> {
        if self.read(id)?.is_some() {
            return Err(VaultError::Exists);
        }
        keyring::Entry::new(SERVICE, id)
            .map_err(|_| VaultError::Unavailable)?
            .set_password(value)
            .map_err(|_| VaultError::Unavailable)
    }
}
#[derive(Clone)]
pub struct Vault {
    backend: Arc<dyn SecretStore>,
}
impl Vault {
    pub fn os() -> Self {
        Self {
            backend: Arc::new(OsSecretStore),
        }
    }
    pub fn with_backend(backend: Arc<dyn SecretStore>) -> Self {
        Self { backend }
    }
    fn record(scope: &ScopeStamp, id: &str, secret: &str) -> Result<String, VaultError> {
        BoundCredential::new(scope, secret.into()).map_err(|_| VaultError::Invalid)?;
        let v = json!({"version":1,"credentialRef":id,"profileId":scope.profile_id,"connectionId":scope.connection_id,
            "providerKind":provider_name(scope.provider),"normalizedEndpoint":scope.normalized_endpoint,
            "credentialRevision":scope.credential_revision,"secret":secret});
        Ok(format!("{PREFIX}{v}"))
    }
    // Blocking platform calls stay off the async runtime thread.
    pub(crate) async fn installation_fingerprint(
        &self,
        reference: String,
        message: String,
        create: bool,
    ) -> Result<String, VaultError> {
        let backend = self.backend.clone();
        tokio::task::spawn_blocking(move || {
            const PREFIX: &str = "ai-home-installation-hmac:v1:";
            let stored = match backend.read(&reference)? {
                Some(value) => value,
                None if create => {
                    let key = format!(
                        "{}{}",
                        uuid::Uuid::new_v4().simple(),
                        uuid::Uuid::new_v4().simple()
                    );
                    let encoded = format!("{PREFIX}{key}");
                    backend.write_new(&reference, &encoded)?;
                    if backend.read(&reference)?.as_deref() != Some(encoded.as_str()) {
                        return Err(VaultError::Unavailable);
                    }
                    encoded
                }
                None => return Err(VaultError::Unavailable),
            };
            let hex = stored
                .strip_prefix(PREFIX)
                .filter(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or(VaultError::Invalid)?;
            let key: Vec<u8> = (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| VaultError::Invalid))
                .collect::<Result<_, _>>()?;
            let digest = hmac_sha256(&key, message.as_bytes());
            Ok(format!(
                "hmac-v1:{}",
                digest
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            ))
        })
        .await
        .map_err(|_| VaultError::Unavailable)?
    }
    pub async fn store(
        &self,
        scope: ScopeStamp,
        id: String,
        secret: String,
    ) -> Result<(), VaultError> {
        let backend = self.backend.clone();
        tokio::task::spawn_blocking(move || {
            let encoded = Self::record(&scope, &id, &secret)?;
            backend.write_new(&id, &encoded)?;
            if backend.read(&id)?.as_deref() != Some(encoded.as_str()) {
                return Err(VaultError::Unavailable);
            }
            Ok(())
        })
        .await
        .map_err(|_| VaultError::Unavailable)?
    }
    pub async fn secret(&self, scope: ScopeStamp, id: String) -> Result<String, VaultError> {
        let backend = self.backend.clone();
        tokio::task::spawn_blocking(move || {
            let raw = backend.read(&id)?.ok_or(VaultError::Unavailable)?;
            let v: Value =
                serde_json::from_str(raw.strip_prefix(PREFIX).ok_or(VaultError::Invalid)?)
                    .map_err(|_| VaultError::Invalid)?;
            let expected = Self::record(
                &scope,
                &id,
                v["secret"].as_str().ok_or(VaultError::Invalid)?,
            )?;
            if raw != expected {
                return Err(VaultError::Invalid);
            }
            Ok(v["secret"].as_str().unwrap().to_string())
        })
        .await
        .map_err(|_| VaultError::Unavailable)?
    }
}

fn provider_name(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Openrouter => "openrouter",
        ProviderKind::Openai => "openai",
        ProviderKind::Google => "google",
        ProviderKind::Anthropic => "anthropic",
        ProviderKind::CustomOpenai => "custom_openai",
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    if key.len() > BLOCK {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

#[cfg(test)]
mod hmac_tests {
    use super::*;
    #[test]
    fn rfc4231_vector() {
        let digest = hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(
            digest
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect::<String>(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}
