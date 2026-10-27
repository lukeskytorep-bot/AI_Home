use super::IdentityError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::{
    ecdsa::{
        signature::{Signer, Verifier},
        Signature, SigningKey, VerifyingKey,
    },
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey},
};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;
pub const ALGORITHM: &str = "openssl-ecdsa-p256-sha256-der/1";
pub const PUBLIC_FORMAT: &str = "spki-der-base64url/1";
pub const SIGNATURE_FORMAT: &str = "asn1-der-base64url/1";
pub(crate) fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
pub(crate) fn b64(b: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(b)
}
pub(crate) fn decode(s: &str, max: usize) -> Result<Vec<u8>, IdentityError> {
    if s.is_empty() || s.len() > max * 2 {
        return Err(IdentityError::Invalid);
    }
    let b = URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|_| IdentityError::Invalid)?;
    if b.len() > max || b64(&b) != s {
        return Err(IdentityError::Invalid);
    }
    Ok(b)
}
// Exact SPKI wrapper for the single frozen named P-256 suite. ECC and DER
// decoding/signing use RustCrypto. Both OpenSSL point encodings are canonical.
fn spki(point: &[u8]) -> Vec<u8> {
    let mut v = vec![
        0x30,
        (24 + point.len()) as u8,
        0x30,
        0x13,
        0x06,
        0x07,
        0x2a,
        0x86,
        0x48,
        0xce,
        0x3d,
        0x02,
        0x01,
        0x06,
        0x08,
        0x2a,
        0x86,
        0x48,
        0xce,
        0x3d,
        0x03,
        0x01,
        0x07,
        0x03,
        (point.len() + 1) as u8,
        0x00,
    ];
    v.extend_from_slice(point);
    v
}
fn public(s: &str) -> Result<VerifyingKey, IdentityError> {
    let b = decode(s, 512)?;
    let k = VerifyingKey::from_public_key_der(&b).map_err(|_| IdentityError::Invalid)?;
    let compressed = match b.len() {
        59 => true,
        91 => false,
        _ => return Err(IdentityError::Invalid),
    };
    if spki(k.to_encoded_point(compressed).as_bytes()) != b {
        return Err(IdentityError::Invalid);
    }
    Ok(k)
}
pub fn public_key_fingerprint(s: &str) -> Result<String, IdentityError> {
    let k = public(s)?;
    let point = k.to_encoded_point(false);
    let mut bytes = b"P256-POINT/1\0".to_vec();
    bytes.extend_from_slice(&point.as_bytes()[1..]);
    Ok(digest(&bytes))
}
pub fn verify_signature(message: &[u8], key: &str, signature: &str) -> Result<bool, IdentityError> {
    if message.len() > 8192 {
        return Err(IdentityError::Invalid);
    }
    let k = public(key)?;
    let b = decode(signature, 256)?;
    let s = Signature::from_der(&b).map_err(|_| IdentityError::Invalid)?;
    if s.to_der().as_bytes() != b {
        return Err(IdentityError::Invalid);
    }
    Ok(k.verify(message, &s).is_ok())
}
pub(crate) struct PrivateKey(SigningKey);
impl PrivateKey {
    pub fn generate() -> Result<Self, IdentityError> {
        for _ in 0..16 {
            let mut bytes = Zeroizing::new([0u8; 32]);
            OsRng
                .try_fill_bytes(bytes.as_mut())
                .map_err(|_| IdentityError::Entropy)?;
            if let Ok(k) = SigningKey::from_slice(bytes.as_ref()) {
                return Ok(Self(k));
            }
        }
        Err(IdentityError::Entropy)
    }
    pub fn public(&self) -> String {
        b64(&spki(
            self.0.verifying_key().to_encoded_point(false).as_bytes(),
        ))
    }
    pub fn encode(&self) -> Result<Zeroizing<String>, IdentityError> {
        let d = self.0.to_pkcs8_der().map_err(|_| IdentityError::Invalid)?;
        Ok(Zeroizing::new(b64(d.as_bytes())))
    }
    pub fn decode(s: &str) -> Result<Self, IdentityError> {
        let bytes = Zeroizing::new(decode(s, 512)?);
        let k = SigningKey::from_pkcs8_der(&bytes).map_err(|_| IdentityError::Invalid)?;
        Ok(Self(k))
    }
    pub fn sign(&self, message: &[u8]) -> String {
        let signature: Signature = self.0.sign(message);
        b64(signature.to_der().as_bytes())
    }
}
