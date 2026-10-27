use super::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/community-identity")
            .join(name),
    )
    .unwrap()
}
fn manifest() -> Value {
    serde_json::from_str(&fixture("registration/manifest.json")).unwrap()
}
fn scope() -> CommunityScope {
    CommunityScope::new(
        "profile-a",
        "https://icrvg.etqis.com/api/v1/",
        "https://icrvg.etqis.com",
        "icrvg-etqis-ai-community-v3.1",
    )
    .unwrap()
}
fn profile() -> RegistrationProfile {
    let m = manifest();
    RegistrationProfile::new(
        m["profile"]["display_name"].as_str().unwrap(),
        m["profile"]["about"].as_str().unwrap(),
    )
    .unwrap()
}
fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-02T14:14:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
fn public_fixture() -> PublicIdentity {
    let m = manifest();
    PublicIdentity {
        key_ref: "fixture".into(),
        scope_binding: scope().binding(),
        identity_public_key: m["identity_public_key"].as_str().unwrap().into(),
        operational_public_key: m["operational_public_key"].as_str().unwrap().into(),
    }
}
fn response(intent: &RegistrationIntent) -> Value {
    let m = manifest();
    let statement = registration_statement(
        &intent.scope,
        &intent.identity,
        &intent.profile,
        m["registration_id"].as_str().unwrap(),
        m["nonce"].as_str().unwrap(),
        m["expires_at"].as_str().unwrap(),
    );
    json!({"ok":true,"request_id":"request","registration":{"registration_id":m["registration_id"],"state":"pending","nonce":m["nonce"],"expires_at":m["expires_at"],"community_origin":intent.scope.origin,"community_instance":intent.scope.instance,"profile_format":"COMMUNITY-PROFILE/1","profile_sha256":intent.profile.sha256(),"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}})
}
#[derive(Default)]
struct Memory {
    data: Mutex<HashMap<String, zeroize::Zeroizing<String>>>,
}
impl CommunitySecretStore for Memory {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        Ok(self.data.lock().unwrap().get(r).cloned())
    }
    fn create(&self, r: &str, s: &str) -> Result<(), IdentityError> {
        let mut m = self.data.lock().unwrap();
        if m.contains_key(r) {
            return Err(IdentityError::AlreadyExists);
        }
        m.insert(r.into(), Zeroizing::new(s.into()));
        Ok(())
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.data.lock().unwrap().remove(r);
        Ok(())
    }
}
async fn setup() -> (CommunityKeyStore, Arc<Memory>, RegistrationIntent) {
    let b = Arc::new(Memory::default());
    let store = CommunityKeyStore::with_backend(b.clone());
    let p = store
        .provision(&scope(), &CommunityKeyStore::reserve_reference().unwrap())
        .await
        .unwrap();
    let intent =
        RegistrationIntent::new(scope(), p, profile(), "stable-client-start-0123456789").unwrap();
    (store, b, intent)
}
#[test]
fn profile_and_statement_match_independent_php_registration_fixtures_byte_for_byte() {
    let m = manifest();
    assert_eq!(
        profile().canonical_bytes(),
        fixture("registration/profile.txt").as_bytes()
    );
    assert_eq!(profile().sha256(), m["profile_sha256"]);
    let i = RegistrationIntent::new(
        scope(),
        public_fixture(),
        profile(),
        "fixture-client-start-123",
    )
    .unwrap();
    let r = i.validate_challenge(&response(&i), now()).unwrap();
    assert_eq!(
        r.statement_bytes(),
        fixture("registration/statement.txt").as_bytes()
    );
    assert_eq!(crypto::digest(r.statement_bytes()), m["statement_sha256"]);
}
#[test]
fn independent_signature_vectors_verify_for_both_registration_roles() {
    let m = manifest();
    let text = fixture("registration/statement.txt");
    for role in ["identity", "operational"] {
        let key = m[format!("{role}_public_key")].as_str().unwrap();
        let sig = fixture(&format!("registration/{role}_signature.b64u"));
        assert!(verify_signature(text.as_bytes(), key, sig.trim()).unwrap());
        assert!(!verify_signature(b"modified", key, sig.trim()).unwrap());
    }
}
#[test]
fn p256_generic_fixture_verifies_and_p384_is_rejected() {
    let text = fixture("p256-sha256-v1/statement.txt");
    assert!(verify_signature(
        text.as_bytes(),
        fixture("p256-sha256-v1/public_key.b64u").trim(),
        fixture("p256-sha256-v1/signature.b64u").trim()
    )
    .unwrap());
    assert!(public_key_fingerprint(fixture("p384-negative/public_key.b64u").trim()).is_err());
}
#[test]
fn canonical_base64_and_trailing_der_aliases_are_rejected() {
    let p = public_fixture().identity_public_key;
    for bad in [
        format!("{p}="),
        format!(" {p}"),
        format!("{p}\n"),
        fixture("registration/identity_public_key_trailing_alias.b64u")
            .trim()
            .into(),
    ] {
        assert!(public_key_fingerprint(&bad).is_err());
    }
}
#[test]
fn compressed_key_alias_has_same_point_fingerprint_and_cannot_be_second_authority() {
    let p = public_fixture();
    let alias = fixture("registration/identity_public_key_compressed_alias.b64u");
    assert_eq!(
        public_key_fingerprint(&p.identity_public_key).unwrap(),
        public_key_fingerprint(alias.trim()).unwrap()
    );
    let mut same = p.clone();
    same.operational_public_key = alias.trim().into();
    assert!(RegistrationIntent::new(scope(), same, profile(), "fixture-client-start-123").is_err());
}
#[test]
fn display_name_uses_php_ascii_trim_utf8_bytes_and_preserves_source_text() {
    let p = RegistrationProfile::new("\0\t Żółw \r\n", "about\nline").unwrap();
    assert_eq!(p.display_name, "Żółw");
    assert!(RegistrationProfile::new(&"ą".repeat(61), "").is_err());
    assert!(RegistrationProfile::new("middle\nline", "").is_err());
    assert!(RegistrationProfile::new("x", "a\0b").is_err());
    assert_eq!(
        RegistrationProfile::new("\u{a0}Name\u{a0}", "")
            .unwrap()
            .display_name,
        "\u{a0}Name\u{a0}"
    );
}
#[test]
fn scope_requires_explicit_pins_and_no_provider_or_model_dependency() {
    let a = scope();
    for b in [
        CommunityScope::new("profile-b", &a.api_root, &a.origin, &a.instance).unwrap(),
        CommunityScope::new(
            "profile-a",
            "https://icrvg.etqis.com/other/api/v1/",
            &a.origin,
            &a.instance,
        )
        .unwrap(),
        CommunityScope::new("profile-a", &a.api_root, "https://other.test", &a.instance).unwrap(),
        CommunityScope::new("profile-a", &a.api_root, &a.origin, "other-instance").unwrap(),
    ] {
        assert_ne!(a.binding(), b.binding());
    }
    assert!(CommunityScope::new(
        "p",
        "http://example.test/api/v1",
        "https://example.test",
        "instance"
    )
    .is_err());
}
#[test]
fn tampered_server_statement_profile_algorithms_and_nonce_are_never_signed() {
    let i = RegistrationIntent::new(
        scope(),
        public_fixture(),
        profile(),
        "fixture-client-start-123",
    )
    .unwrap();
    for (k, v) in [
        ("statement", json!("sign arbitrary text")),
        ("profile_sha256", json!("a".repeat(64))),
        ("signature_algorithm", json!("ed25519")),
        ("protocol_version", json!("2")),
        ("nonce", json!("bad")),
        ("registration_id", json!("reg_bad")),
        ("retry", json!("false")),
    ] {
        let mut r = response(&i);
        r["registration"][k] = v;
        assert!(i.validate_challenge(&r, now()).is_err(), "{k}");
    }
}
#[test]
fn wrong_origin_instance_or_profile_fails_closed() {
    let i = RegistrationIntent::new(
        scope(),
        public_fixture(),
        profile(),
        "fixture-client-start-123",
    )
    .unwrap();
    for field in ["community_origin", "community_instance"] {
        let mut r = response(&i);
        r["registration"][field] = json!("other");
        assert_eq!(
            i.validate_challenge(&r, now()).unwrap_err(),
            IdentityError::ScopeMismatch
        );
    }
    let mut wrong = public_fixture();
    wrong.scope_binding = "other".into();
    assert!(
        RegistrationIntent::new(scope(), wrong, profile(), "fixture-client-start-123").is_err()
    );
}
#[test]
fn expiration_and_committed_retry_require_explicit_recovery() {
    let i = RegistrationIntent::new(
        scope(),
        public_fixture(),
        profile(),
        "fixture-client-start-123",
    )
    .unwrap();
    assert_eq!(
        i.validate_challenge(&response(&i), now() + chrono::Duration::minutes(2))
            .unwrap_err(),
        IdentityError::Expired
    );
    let mut r = response(&i);
    r["registration"]["state"] = json!("committed");
    assert_eq!(
        i.validate_challenge(&r, now()).unwrap_err(),
        IdentityError::RecoveryRequired
    );
}
#[tokio::test]
async fn create_load_restart_and_sign_use_distinct_stable_authorities() {
    let (store, b, i) = setup().await;
    let reference = i.identity.key_reference();
    let restarted = CommunityKeyStore::with_backend(b);
    let loaded = restarted.load(&scope(), reference).await.unwrap();
    assert_eq!(loaded, i.identity);
    assert_ne!(loaded.identity_public_key, loaded.operational_public_key);
    let r = i.validate_challenge(&response(&i), now()).unwrap();
    let packet = store.complete_packet(&scope(), &r, now()).await.unwrap();
    assert!(verify_signature(
        r.statement_bytes(),
        loaded.identity_public_key(),
        packet["identity_signature"].as_str().unwrap()
    )
    .unwrap());
    assert!(verify_signature(
        r.statement_bytes(),
        loaded.operational_public_key(),
        packet["operational_signature"].as_str().unwrap()
    )
    .unwrap());
    assert!(!verify_signature(
        r.statement_bytes(),
        loaded.identity_public_key(),
        packet["operational_signature"].as_str().unwrap()
    )
    .unwrap());
}
#[tokio::test]
async fn start_retry_keeps_keys_start_id_and_body_including_invite_escaping() {
    let (_, _, i) = setup().await;
    let a = i.start_request("invite-\"back\\slash").unwrap();
    let b = i.start_request("invite-\"back\\slash").unwrap();
    assert_eq!(a.as_bytes(), b.as_bytes());
    let body: Value = serde_json::from_slice(a.as_bytes()).unwrap();
    assert_eq!(body["invite_token"], "invite-\"back\\slash");
    assert_eq!(body["client_start_id"], i.client_start_id);
    assert_eq!(body["identity_public_key"], i.identity.identity_public_key);
    assert_eq!(body.as_object().unwrap().len(), 5);
}
#[tokio::test]
async fn vault_records_and_private_keys_do_not_appear_in_public_handles_or_packets() {
    let (store, b, i) = setup().await;
    let reg = i.validate_challenge(&response(&i), now()).unwrap();
    let packet = store.complete_packet(&scope(), &reg, now()).await.unwrap();
    let visible = format!("{:?}{packet}", i);
    for secret in b.data.lock().unwrap().values() {
        assert!(!visible.contains(secret.as_str()));
        for line in secret.lines().filter(|l| l.contains("pkcs8=")) {
            assert!(!visible.contains(line));
        }
    }
    assert!(!visible.contains("identity_pkcs8"));
}
#[tokio::test]
async fn cross_profile_scope_and_provider_secret_fallback_are_blocked() {
    let (store, _, i) = setup().await;
    let a = scope();
    let b = CommunityScope::new("foreign", &a.api_root, &a.origin, &a.instance).unwrap();
    assert_eq!(
        store
            .load(&b, i.identity.key_reference())
            .await
            .unwrap_err(),
        IdentityError::ScopeMismatch
    );
    assert_eq!(
        store.load(&a, "provider-credential-123").await.unwrap_err(),
        IdentityError::Invalid
    );
    let reg = i.validate_challenge(&response(&i), now()).unwrap();
    assert_eq!(
        store.complete_packet(&b, &reg, now()).await.unwrap_err(),
        IdentityError::ScopeMismatch
    );
}
#[tokio::test]
async fn deletion_is_explicit_scoped_idempotent_and_missing_keys_never_regenerate() {
    let (store, b, i) = setup().await;
    let reference = i.identity.key_reference();
    let a = scope();
    let foreign = CommunityScope::new("foreign", &a.api_root, &a.origin, &a.instance).unwrap();
    assert_eq!(
        store.remove(&foreign, reference).await,
        Err(IdentityError::ScopeMismatch)
    );
    store.remove(&a, reference).await.unwrap();
    store.remove(&a, reference).await.unwrap();
    assert_eq!(
        store.load(&a, reference).await.unwrap_err(),
        IdentityError::NotFound
    );
    assert!(b.data.lock().unwrap().is_empty());
}
#[tokio::test]
async fn late_signature_after_expiry_is_blocked() {
    let (store, _, i) = setup().await;
    let reg = i.validate_challenge(&response(&i), now()).unwrap();
    assert_eq!(
        store
            .complete_packet(&scope(), &reg, now() + chrono::Duration::minutes(2))
            .await
            .unwrap_err(),
        IdentityError::Expired
    );
}
#[tokio::test]
async fn replaced_private_material_cannot_sign_a_previously_validated_registration() {
    let (store, b, i) = setup().await;
    let reg = i.validate_challenge(&response(&i), now()).unwrap();
    let other = store
        .provision(&scope(), &CommunityKeyStore::reserve_reference().unwrap())
        .await
        .unwrap();
    let second = b.data.lock().unwrap()[other.key_reference()].clone();
    b.data
        .lock()
        .unwrap()
        .insert(i.identity.key_ref.clone(), second);
    assert_eq!(
        store
            .complete_packet(&scope(), &reg, now())
            .await
            .unwrap_err(),
        IdentityError::ScopeMismatch
    );
}
struct Unavailable;
impl CommunitySecretStore for Unavailable {
    fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        Err(IdentityError::VaultUnavailable)
    }
    fn create(&self, _: &str, _: &str) -> Result<(), IdentityError> {
        Err(IdentityError::VaultUnavailable)
    }
    fn remove(&self, _: &str) -> Result<(), IdentityError> {
        Err(IdentityError::VaultUnavailable)
    }
}
#[tokio::test]
async fn unavailable_vault_never_creates_plaintext_fallback() {
    let s = CommunityKeyStore::with_backend(Arc::new(Unavailable));
    assert_eq!(
        s.provision(&scope(), &CommunityKeyStore::reserve_reference().unwrap())
            .await
            .unwrap_err(),
        IdentityError::VaultUnavailable
    );
}

#[tokio::test]
async fn reserved_reference_is_reusable_for_load_but_never_overwrites_existing_keys() {
    let b = Arc::new(Memory::default());
    let store = CommunityKeyStore::with_backend(b);
    let reference = CommunityKeyStore::reserve_reference().unwrap();
    let first = store.provision(&scope(), &reference).await.unwrap();
    assert_eq!(
        store.provision(&scope(), &reference).await.unwrap_err(),
        IdentityError::AlreadyExists
    );
    assert_eq!(store.load(&scope(), &reference).await.unwrap(), first);
}
#[tokio::test]
async fn complete_retry_is_byte_identical_and_start_wire_size_is_bounded() {
    let (store, _, i) = setup().await;
    let r = i.validate_challenge(&response(&i), now()).unwrap();
    assert_eq!(
        store.complete_packet(&scope(), &r, now()).await.unwrap(),
        store.complete_packet(&scope(), &r, now()).await.unwrap()
    );
    let mut big = i;
    big.profile = RegistrationProfile::new("x", &"\u{0001}".repeat(2048)).unwrap();
    assert!(matches!(
        big.start_request("invite"),
        Err(IdentityError::RequestTooLarge)
    ));
}
#[tokio::test]
async fn public_registration_packet_is_available_for_independent_openssl_verification() {
    let (store, _, i) = setup().await;
    let r = i.validate_challenge(&response(&i), now()).unwrap();
    let packet = store.complete_packet(&scope(), &r, now()).await.unwrap();
    if let Ok(path) = std::env::var("AI_HOME_PUBLIC_INTEROP_OUT") {
        let public = json!({"statement":String::from_utf8(r.statement_bytes().to_vec()).unwrap(),"identity_public_key":i.identity.identity_public_key,"operational_public_key":i.identity.operational_public_key,"complete":packet});
        std::fs::write(path, serde_json::to_vec_pretty(&public).unwrap()).unwrap();
    }
}
