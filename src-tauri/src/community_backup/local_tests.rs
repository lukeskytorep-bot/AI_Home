use super::*;
use crate::community_host::{
    local::{LocalAccess, LocalCapability},
    maintenance::{CommunityHost, HostError, HostStatus},
};
fn all(scope: &CommunityScope) -> LocalAccess {
    LocalAccess::new(
        scope.clone(),
        vec![
            LocalCapability::Status,
            LocalCapability::Prepare,
            LocalCapability::Provision,
            LocalCapability::Recover,
            LocalCapability::AccessStatus,
        ],
    )
}
fn host(f: &Fixture) -> CommunityHost {
    CommunityHost::new(
        &f.root,
        CommunityLifecycle::new(
            f.registry.clone(),
            f.keys.clone(),
            CommunityTokenStore::with_backend(f.secret.clone()),
        ),
        Vault::with_backend(Arc::new(NoModels)),
        None,
    )
    .unwrap()
}
async fn setup() -> (Fixture, CommunityHost, Value, CommunityScope) {
    let f = Fixture::new().await;
    let h = host(&f);
    h.open_application().await.unwrap();
    let response = h
        .dispatch(request_cmd(
            "profile.create",
            None,
            None,
            json!({"displayName":"Local profile"}),
        ))
        .await
        .unwrap()
        .response;
    assert_eq!(response["ok"], true);
    let p = response["result"].clone();
    let scope = CommunityScope::new(
        p["id"].as_str().unwrap(),
        "https://example.invalid/api/v1/",
        "https://example.invalid",
        "stable-community",
    )
    .unwrap();
    (f, h, p, scope)
}
fn request(p: &Value, action: &str, payload: Value) -> Value {
    request_cmd(
        &format!("community.registration.{action}"),
        p["id"].as_str(),
        p["revision"].as_i64(),
        payload,
    )
}
fn prepare(p: &Value) -> Value {
    request(
        p,
        "prepare",
        json!({"clientStartId":"local-start-0123456789","displayName":"Public agent","about":"Public description"}),
    )
}
async fn call(h: &CommunityHost, r: Value, s: &CommunityScope) -> Value {
    h.dispatch_local(r, all(s)).await.unwrap()
}
async fn raw(f: &Fixture, sql: &str) {
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query(sql).execute(&mut c).await.unwrap();
    c.close().await.unwrap();
}
async fn count(f: &Fixture, table: &str) -> i64 {
    let mut c = connection(&f.root.join("foundation.sqlite3"), true, false)
        .await
        .unwrap();
    let n = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    n
}

#[tokio::test]
async fn local_prepare_provision_recover_status_and_exact_replay_use_real_registry() {
    let (f, h, p, s) = setup().await;
    let absent = call(&h, request(&p, "status", json!({})), &s).await;
    assert_eq!(absent["result"]["preparationState"], "absent");
    assert_eq!(absent["result"]["onlineStatusKnown"], false);
    let req = prepare(&p);
    let reserved = call(&h, req.clone(), &s).await;
    assert_eq!(reserved["result"]["preparationState"], "reserved");
    assert_eq!(call(&h, req.clone(), &s).await, reserved);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    let provision = request(&p, "provision", json!({"registrationRevision":1}));
    let ready = call(&h, provision.clone(), &s).await;
    assert_eq!(ready["result"]["preparationState"], "ready");
    let created = f.secret.creates.load(Ordering::SeqCst);
    assert_eq!(created, 1);
    assert_eq!(call(&h, provision, &s).await, ready);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), created);
    let recovered = call(
        &h,
        request(&p, "recover", json!({"registrationRevision":3})),
        &s,
    )
    .await;
    assert_eq!(recovered["result"]["preparationState"], "ready");
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), created);
    let bytes = serde_json::to_string(&ready).unwrap();
    for forbidden in [
        "key_reference",
        "ai-home-community-keyset",
        "PRIVATE KEY",
        "identityPublicKey",
        "token",
    ] {
        assert!(!bytes.contains(forbidden));
    }
    assert!(count(&f, "receipts").await >= 4);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_capabilities_default_deny_and_json_cannot_grant_them() {
    let (f, h, p, s) = setup().await;
    let before = count(&f, "receipts").await;
    let denied = h
        .dispatch_local(prepare(&p), LocalAccess::new(s.clone(), vec![]))
        .await
        .unwrap();
    assert_eq!(denied["error"]["code"], "POLICY_DENIED");
    let read_only = LocalAccess::new(s.clone(), vec![LocalCapability::Status]);
    assert_eq!(
        h.dispatch_local(request(&p, "status", json!({})), read_only.clone())
            .await
            .unwrap()["ok"],
        true
    );
    assert_eq!(
        h.dispatch_local(prepare(&p), read_only).await.unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    let mut forged = prepare(&p);
    forged["capabilities"] = json!(["prepare"]);
    assert_eq!(
        call(&h, forged, &s).await["error"]["code"],
        "VALIDATION_ERROR"
    );
    assert_eq!(count(&f, "receipts").await, before);
    assert!(matches!(
        f.registry.get(&s).await,
        Err(RegistryError::NotFound)
    ));
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_grant_cannot_cross_profile_or_community_scope() {
    let (f, h, p, s) = setup().await;
    let req = prepare(&p);
    assert_eq!(call(&h, req.clone(), &s).await["ok"], true);
    let other = CommunityScope::new(
        "other-profile",
        "https://example.invalid/api/v1/",
        "https://example.invalid",
        "stable-community",
    )
    .unwrap();
    assert_eq!(
        call(&h, request(&p, "status", json!({})), &other).await["error"]["code"],
        "POLICY_DENIED"
    );
    let alternate = CommunityScope::new(
        s.profile_id(),
        "https://example.invalid/other/api/v1/",
        "https://example.invalid",
        "other-community",
    )
    .unwrap();
    assert_eq!(
        call(&h, req, &alternate).await["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    h.close_application().await.unwrap();
    drop(f);
}
#[tokio::test]
async fn local_stale_profile_revision_fails_before_registration_reservation() {
    let (f, h, p, s) = setup().await;
    let mut r = prepare(&p);
    r["expectedRevision"] = json!(2);
    let before = count(&f, "receipts").await;
    assert_eq!(call(&h, r, &s).await["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(count(&f, "receipts").await, before);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_archived_and_nonstandard_profiles_are_denied_even_with_all_grants() {
    for update in [
        "UPDATE profiles SET body=json_set(body,'$.status','archived')",
        "UPDATE profiles SET body=json_set(body,'$.mode','conscious')",
    ] {
        let (f, h, p, s) = setup().await;
        raw(&f, update).await;
        assert_eq!(
            call(&h, prepare(&p), &s).await["error"]["code"],
            "POLICY_DENIED"
        );
        assert!(matches!(
            f.registry.get(&s).await,
            Err(RegistryError::NotFound)
        ));
        assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
        h.close_application().await.unwrap();
    }
}
#[tokio::test]
async fn local_authoritative_experimental_binding_cannot_be_masked_by_standard_mode() {
    let (f, h, p, s) = setup().await;
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES(?, 'experimental-id',1,'{}')").bind(s.profile_id()).execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert_eq!(
        call(&h, prepare(&p), &s).await["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_pending_deletion_blocks_every_command_and_reserved_request_id() {
    let (f, h, p, s) = setup().await;
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query("INSERT INTO profile_deletion_jobs(profile_id,request_id,fingerprint,expected_revision) VALUES(?,'delete-id',?,1)").bind(s.profile_id()).bind("a".repeat(64)).execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert_eq!(
        call(&h, prepare(&p), &s).await["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(
        call(&h, request(&p, "status", json!({})), &s).await["error"]["code"],
        "POLICY_DENIED"
    );
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_invalid_payload_cannot_pin_or_claim_a_request() {
    let (f, h, p, s) = setup().await;
    let before = count(&f, "receipts").await;
    for payload in [
        json!({"clientStartId":"short","displayName":"x","about":""}),
        json!({"clientStartId":"local-start-0123456789","displayName":" ","about":""}),
        json!({"clientStartId":"local-start-0123456789","displayName":"x","about":"","apiRoot":"https://evil.invalid"}),
    ] {
        assert_eq!(
            call(&h, request(&p, "prepare", payload), &s).await["error"]["code"],
            "VALIDATION_ERROR"
        );
    }
    assert_eq!(count(&f, "receipts").await, before);
    assert!(matches!(
        f.registry.get(&s).await,
        Err(RegistryError::NotFound)
    ));
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_request_ids_conflict_with_ordinary_dispatch_and_other_local_commands() {
    let (_f, h, p, s) = setup().await;
    let r = prepare(&p);
    assert_eq!(call(&h, r.clone(), &s).await["ok"], true);
    let mut other = request(&p, "provision", json!({"registrationRevision":1}));
    other["requestId"] = r["requestId"].clone();
    assert_eq!(
        call(&h, other, &s).await["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let mut ordinary = request_cmd(
        "profile.create",
        None,
        None,
        json!({"displayName":"Cannot steal ID"}),
    );
    ordinary["requestId"] = r["requestId"].clone();
    assert_eq!(
        h.dispatch(ordinary).await.unwrap().response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_replay_rechecks_current_capability_and_profile_direction() {
    let (f, h, p, s) = setup().await;
    let r = prepare(&p);
    assert_eq!(call(&h, r.clone(), &s).await["ok"], true);
    assert_eq!(
        h.dispatch_local(r.clone(), LocalAccess::new(s.clone(), vec![]))
            .await
            .unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    raw(
        &f,
        "UPDATE profiles SET body=json_set(body,'$.mode','conscious')",
    )
    .await;
    assert_eq!(call(&h, r, &s).await["error"]["code"], "POLICY_DENIED");
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_corrupt_receipt_is_not_returned_as_a_valid_success() {
    let (f, h, p, s) = setup().await;
    let r = prepare(&p);
    call(&h, r.clone(), &s).await;
    for body in [
        "null",
        "{}",
        "{\"communityLocalPending\":true,\"foreign\":true}",
    ] {
        let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
            .await
            .unwrap();
        sqlx::query("UPDATE receipts SET response=? WHERE request_id=?")
            .bind(body)
            .bind(r["requestId"].as_str().unwrap())
            .execute(&mut c)
            .await
            .unwrap();
        c.close().await.unwrap();
        assert_eq!(
            call(&h, r.clone(), &s).await["error"]["code"],
            "STORAGE_ERROR"
        );
    }
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_unfinished_provision_claim_never_repeats_key_creation() {
    let (f, h, p, s) = setup().await;
    call(&h, prepare(&p), &s).await;
    let r = request(&p, "provision", json!({"registrationRevision":1}));
    assert_eq!(call(&h, r.clone(), &s).await["ok"], true);
    let creates = f.secret.creates.load(Ordering::SeqCst);
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query("UPDATE receipts SET response=? WHERE request_id=?")
        .bind("{\"communityLocalPending\":true}")
        .bind(r["requestId"].as_str().unwrap())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        call(&h, r, &s).await["error"]["code"],
        "REGISTRATION_RECONCILIATION_REQUIRED"
    );
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), creates);
    assert_eq!(
        call(
            &h,
            request(&p, "recover", json!({"registrationRevision":3})),
            &s
        )
        .await["ok"],
        true
    );
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_adapter_refuses_offline_or_restore_recovery_host() {
    let (f, h, p, s) = setup().await;
    h.close_application().await.unwrap();
    assert_eq!(h.status(), HostStatus::Offline);
    assert_eq!(
        h.dispatch_local(prepare(&p), all(&s)).await,
        Err(HostError::Unavailable)
    );
    std::fs::write(f.root.join(crate::app_ownership::PENDING), b"broken").unwrap();
    let next = host(&f);
    assert_eq!(next.status(), HostStatus::RecoveryRequired);
    assert_eq!(
        next.dispatch_local(prepare(&p), all(&s)).await,
        Err(HostError::Unavailable)
    );
}
#[tokio::test]
async fn local_failed_final_receipt_claim_is_durable_and_prepare_can_resume_after_restart() {
    let (f, h, p, s) = setup().await;
    let r = prepare(&p);
    let first = call(&h, r.clone(), &s).await;
    assert_eq!(first["ok"], true);
    let mut c = connection(&f.root.join("foundation.sqlite3"), false, false)
        .await
        .unwrap();
    sqlx::query("UPDATE receipts SET response=? WHERE request_id=?")
        .bind("{\"communityLocalPending\":true}")
        .bind(r["requestId"].as_str().unwrap())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    h.close_application().await.unwrap();
    drop(h);
    let next = host(&f);
    next.open_application().await.unwrap();
    assert_eq!(call(&next, r, &s).await["ok"], true);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    next.close_application().await.unwrap();
}

#[tokio::test]
async fn local_missing_keys_never_regenerate_during_recover() {
    let (f, h, p, s) = setup().await;
    call(&h, prepare(&p), &s).await;
    assert_eq!(
        call(
            &h,
            request(&p, "provision", json!({"registrationRevision":1})),
            &s
        )
        .await["ok"],
        true
    );
    f.secret.data.lock().unwrap().clear();
    let creates = f.secret.creates.load(Ordering::SeqCst);
    assert_eq!(
        call(
            &h,
            request(&p, "recover", json!({"registrationRevision":3})),
            &s
        )
        .await["error"]["code"],
        "CREDENTIAL_UNAVAILABLE"
    );
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), creates);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_missing_profile_never_creates_a_registration() {
    let (f, h, mut p, _) = setup().await;
    p["id"] = json!("missing-profile");
    let scope = CommunityScope::new(
        "missing-profile",
        "https://example.invalid/api/v1/",
        "https://example.invalid",
        "stable-community",
    )
    .unwrap();
    assert_eq!(
        call(&h, prepare(&p), &scope).await["error"]["code"],
        "NOT_FOUND"
    );
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn local_imported_reconcile_state_blocks_even_successful_old_receipt_replay() {
    let (f, h, p, s) = setup().await;
    let r = prepare(&p);
    assert_eq!(call(&h, r.clone(), &s).await["ok"], true);
    let mut c = f.reg_conn().await;
    sqlx::query("UPDATE community_profile_lifecycle SET state='reconcile',revision=revision+1")
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(call(&h, r, &s).await["error"]["code"], "POLICY_DENIED");
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn access_status_does_not_promote_local_keys_to_credentials_or_write_receipts() {
    let (f, h, p, s) = setup().await;
    let before = count(&f, "receipts").await;
    let req = request(&p, "access.status", json!({}));
    for stage in 0..3 {
        if stage == 1 {
            assert_eq!(call(&h, prepare(&p), &s).await["ok"], true);
        }
        if stage == 2 {
            assert_eq!(
                call(
                    &h,
                    request(&p, "provision", json!({"registrationRevision":1})),
                    &s
                )
                .await["ok"],
                true
            );
        }
        let reads = f.secret.reads.load(Ordering::SeqCst);
        let receipts = count(&f, "receipts").await;
        let result = call(&h, req.clone(), &s).await;
        assert_eq!(result["ok"], true, "{result}");
        assert_eq!(
            result["result"],
            json!({"localCredentialUsable":false,"onlineStatusKnown":false})
        );
        assert_eq!(count(&f, "receipts").await, receipts);
        assert_eq!(f.secret.reads.load(Ordering::SeqCst), reads);
    }
    assert_eq!(count(&f, "receipts").await, before + 2);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn access_status_requires_separate_capability_and_rechecks_policy() {
    let (f, h, p, s) = setup().await;
    let req = request(&p, "access.status", json!({}));
    assert_eq!(
        h.dispatch_local(
            req.clone(),
            LocalAccess::new(s.clone(), vec![LocalCapability::Status])
        )
        .await
        .unwrap()["error"]["code"],
        "POLICY_DENIED"
    );
    assert_eq!(call(&h, req.clone(), &s).await["ok"], true);
    raw(
        &f,
        "UPDATE profiles SET body=json_set(body,'$.status','archived')",
    )
    .await;
    assert_eq!(call(&h, req, &s).await["error"]["code"], "POLICY_DENIED");
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn access_status_rejects_corrupt_finish_instead_of_reporting_usable() {
    let (f, h, p, s) = setup().await;
    call(&h, prepare(&p), &s).await;
    let mut c = f.reg_conn().await;
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,'{}')")
        .bind(s.binding())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        call(&h, request(&p, "access.status", json!({})), &s).await["error"]["code"],
        "STORAGE_ERROR"
    );
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), 0);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn access_status_validates_actual_token_then_detects_loss_and_scope_tampering() {
    use crate::community_identity::{ALGORITHM, PUBLIC_FORMAT, SIGNATURE_FORMAT};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let (f, h, p, s) = setup().await;
    call(&h, prepare(&p), &s).await;
    call(
        &h,
        request(&p, "provision", json!({"registrationRevision":1})),
        &s,
    )
    .await;
    let prep = f.registry.get(&s).await.unwrap();
    let keys = prep.public_identity().unwrap();
    let id = format!("reg_{}", "1".repeat(32));
    let agent = format!("agt_{}", "2".repeat(32));
    let nonce = URL_SAFE_NO_PAD.encode([7u8; 32]);
    let expiry = (chrono::Utc::now() + chrono::Duration::minutes(10))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let hash = RegistrationProfile::new("Public agent", "Public description")
        .unwrap()
        .sha256();
    let statement=format!("AGENT-REGISTRATION/1\ncommunity_origin=https://example.invalid\ncommunity_instance=stable-community\nregistration_id={id}\nnonce={nonce}\nexpires_at={expiry}\nidentity_public_key={}\noperational_public_key={}\nprofile_format=COMMUNITY-PROFILE/1\nprofile_sha256={hash}\nsignature_algorithm={ALGORITHM}\npublic_key_format={PUBLIC_FORMAT}\nsignature_format={SIGNATURE_FORMAT}\nprotocol_version=1\n",keys.identity_public_key(),keys.operational_public_key());
    let context = json!({"ok":true,"request_id":"fixture","registration":{"registration_id":id,"state":"pending","nonce":nonce,"expires_at":expiry,"community_origin":"https://example.invalid","community_instance":"stable-community","profile_format":"COMMUNITY-PROFILE/1","profile_sha256":hash,"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}});
    let reference = format!("ai-home-community-token:v1:{}", "3".repeat(32));
    let token = format!("eqai_{}", URL_SAFE_NO_PAD.encode([9u8; 32]));
    let saved=format!("AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={}\nregistration_id={id}\nagent_id={agent}\ngeneration=1\ntoken={token}\n",s.binding());
    f.secret
        .data
        .lock()
        .unwrap()
        .insert(reference.clone(), Zeroizing::new(saved.clone()));
    let body = json!({"registrationId":id,"completeBody":null,"state":"ready","agentId":agent,"keyId":null,"generation":1,"tokenRef":reference});
    let mut c = f.reg_conn().await;
    sqlx::query("INSERT INTO community_registration_starts VALUES(?,?,1,?,NULL)")
        .bind(s.binding())
        .bind("0".repeat(64))
        .bind(context.to_string())
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,?)")
        .bind(s.binding())
        .bind(body.to_string())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let req = request(&p, "access.status", json!({}));
    let before = count(&f, "receipts").await;
    let creates = f.secret.creates.load(Ordering::SeqCst);
    let result = call(&h, req.clone(), &s).await;
    assert_eq!(
        result["result"],
        json!({"localCredentialUsable":true,"onlineStatusKnown":false}),
        "{result}"
    );
    assert!(!result.to_string().contains(&token));
    f.secret.data.lock().unwrap().remove(&reference);
    assert_eq!(
        call(&h, req.clone(), &s).await["result"]["localCredentialUsable"],
        false
    );
    f.secret.data.lock().unwrap().insert(
        reference,
        Zeroizing::new(saved.replace(&s.binding(), &"0".repeat(64))),
    );
    assert_eq!(
        call(&h, req, &s).await["result"]["localCredentialUsable"],
        false
    );
    assert_eq!(count(&f, "receipts").await, before);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), creates);
    h.close_application().await.unwrap();
}
struct FailedKeyRead {
    kind: u8,
    replacement: Option<Zeroizing<String>>,
}
impl CommunitySecretStore for FailedKeyRead {
    fn read(&self, _: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        match self.kind {
            0 => Ok(None),
            1 => Err(IdentityError::VaultUnavailable),
            _ => Ok(self.replacement.clone()),
        }
    }
    fn create(&self, _: &str, _: &str) -> Result<(), IdentityError> {
        panic!("status must not create secrets")
    }
    fn remove(&self, _: &str) -> Result<(), IdentityError> {
        panic!("status must not delete secrets")
    }
}
async fn audit_key_failure(kind: u8) {
    use crate::community_identity::{ALGORITHM, PUBLIC_FORMAT, SIGNATURE_FORMAT};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let (mut f, h, p, s) = setup().await;
    call(&h, prepare(&p), &s).await;
    call(
        &h,
        request(&p, "provision", json!({"registrationRevision":1})),
        &s,
    )
    .await;
    let prep = f.registry.get(&s).await.unwrap();
    let keys = prep.public_identity().unwrap();
    let id = format!("reg_{}", "1".repeat(32));
    let agent = format!("agt_{}", "2".repeat(32));
    let nonce = URL_SAFE_NO_PAD.encode([7u8; 32]);
    let expiry = (chrono::Utc::now() + chrono::Duration::minutes(10))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let hash = RegistrationProfile::new("Public agent", "Public description")
        .unwrap()
        .sha256();
    let statement=format!("AGENT-REGISTRATION/1\ncommunity_origin=https://example.invalid\ncommunity_instance=stable-community\nregistration_id={id}\nnonce={nonce}\nexpires_at={expiry}\nidentity_public_key={}\noperational_public_key={}\nprofile_format=COMMUNITY-PROFILE/1\nprofile_sha256={hash}\nsignature_algorithm={ALGORITHM}\npublic_key_format={PUBLIC_FORMAT}\nsignature_format={SIGNATURE_FORMAT}\nprotocol_version=1\n",keys.identity_public_key(),keys.operational_public_key());
    let context = json!({"ok":true,"request_id":"fixture","registration":{"registration_id":id,"state":"pending","nonce":nonce,"expires_at":expiry,"community_origin":"https://example.invalid","community_instance":"stable-community","profile_format":"COMMUNITY-PROFILE/1","profile_sha256":hash,"signature_algorithm":ALGORITHM,"public_key_format":PUBLIC_FORMAT,"signature_format":SIGNATURE_FORMAT,"protocol_version":"1","statement":statement,"retry":false}});
    let reference = format!("ai-home-community-token:v1:{}", "3".repeat(32));
    let token = format!("eqai_{}", URL_SAFE_NO_PAD.encode([9u8; 32]));
    let saved=format!("AI-HOME-COMMUNITY-TOKEN/1\nscope_sha256={}\nregistration_id={id}\nagent_id={agent}\ngeneration=1\ntoken={token}\n",s.binding());
    f.secret
        .data
        .lock()
        .unwrap()
        .insert(reference.clone(), Zeroizing::new(saved.clone()));
    let body = json!({"registrationId":id,"completeBody":null,"state":"ready","agentId":agent,"keyId":null,"generation":1,"tokenRef":reference});
    let mut c = f.reg_conn().await;
    sqlx::query("INSERT INTO community_registration_starts VALUES(?,?,1,?,NULL)")
        .bind(s.binding())
        .bind("0".repeat(64))
        .bind(context.to_string())
        .execute(&mut c)
        .await
        .unwrap();
    sqlx::query("INSERT INTO community_registration_finishes VALUES(?,1,?)")
        .bind(s.binding())
        .bind(body.to_string())
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    let req = request(&p, "access.status", json!({}));
    assert_eq!(
        call(&h, req.clone(), &s).await["result"]["localCredentialUsable"],
        true
    );
    h.close_application().await.unwrap();
    drop(h);
    let other = CommunityKeyStore::reserve_reference().unwrap();
    f.keys.provision(&s, &other).await.unwrap();
    let replacement = f.secret.data.lock().unwrap().get(&other).cloned();
    f.keys = CommunityKeyStore::with_backend(Arc::new(FailedKeyRead { kind, replacement }));
    let before = f.secret.data.lock().unwrap().clone();
    let creates = f.secret.creates.load(Ordering::SeqCst);
    let removes = f.secret.removes.load(Ordering::SeqCst);
    let receipts = count(&f, "receipts").await;
    let h = host(&f);
    h.open_application().await.unwrap();
    let result = call(&h, req.clone(), &s).await;
    assert_eq!(
        result,
        json!({"contractVersion":1,"requestId":req["requestId"],"ok":true,"result":{"localCredentialUsable":false,"onlineStatusKnown":false}})
    );
    assert_eq!(*f.secret.data.lock().unwrap(), before);
    assert_eq!(f.secret.creates.load(Ordering::SeqCst), creates);
    assert_eq!(f.secret.removes.load(Ordering::SeqCst), removes);
    assert_eq!(count(&f, "receipts").await, receipts);
    h.close_application().await.unwrap();
}
#[tokio::test]
async fn audit_access_status_missing_original_keys() {
    audit_key_failure(0).await
}
#[tokio::test]
async fn audit_access_status_unavailable_original_keys() {
    audit_key_failure(1).await
}
#[tokio::test]
async fn audit_access_status_mismatched_original_keys() {
    audit_key_failure(2).await
}
struct PausedSecrets {
    inner: Secrets,
    entered: tokio::sync::Notify,
    release: (Mutex<bool>, std::sync::Condvar),
}
impl CommunitySecretStore for PausedSecrets {
    fn read(&self, r: &str) -> Result<Option<Zeroizing<String>>, IdentityError> {
        self.inner.read(r)
    }
    fn create(&self, r: &str, v: &str) -> Result<(), IdentityError> {
        self.entered.notify_one();
        let (lock, cv) = &self.release;
        let (guard, timer) = cv
            .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(15), |released| {
                !*released
            })
            .unwrap();
        if timer.timed_out() && !*guard {
            return Err(IdentityError::VaultUnavailable);
        }
        self.inner.create(r, v)
    }
    fn remove(&self, r: &str) -> Result<(), IdentityError> {
        self.inner.remove(r)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_cancelled_waiter_retains_main_policy_lock_and_host_maintenance_gate() {
    let (mut f, h, p, s) = setup().await;
    assert_eq!(call(&h, prepare(&p), &s).await["ok"], true);
    h.close_application().await.unwrap();
    drop(h);
    let secrets = Arc::new(PausedSecrets {
        inner: Secrets::default(),
        entered: tokio::sync::Notify::new(),
        release: (Mutex::new(false), std::sync::Condvar::new()),
    });
    f.keys = CommunityKeyStore::with_backend(secrets.clone());
    let h = host(&f);
    h.open_application().await.unwrap();
    let worker = tokio::spawn({
        let h = h.clone();
        let s = s.clone();
        let r = request(&p, "provision", json!({"registrationRevision":1}));
        async move { call(&h, r, &s).await }
    });
    tokio::time::timeout(Duration::from_secs(10), secrets.entered.notified())
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(f.root.join("foundation.sqlite3"))
        .create_if_missing(false)
        .busy_timeout(Duration::from_millis(50));
    let mut c = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE profiles SET body=json_set(body,'$.status','archived')")
            .execute(&mut c)
            .await
            .is_err()
    );
    c.close().await.unwrap();
    let capture = tokio::spawn({
        let h = h.clone();
        let path = f.bundle();
        async move { h.create_backup(&path, "after-local-write").await }
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while h.status() != HostStatus::Maintenance {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        h.dispatch_local(request(&p, "status", json!({})), all(&s))
            .await,
        Err(HostError::Busy)
    );
    assert!(!capture.is_finished());
    *secrets.release.0.lock().unwrap() = true;
    secrets.release.1.notify_all();
    capture.await.unwrap().unwrap();
    assert_eq!(secrets.inner.creates.load(Ordering::SeqCst), 1);
    h.open_application().await.unwrap();
    assert_eq!(
        call(&h, request(&p, "status", json!({})), &s).await["result"]["preparationState"],
        "ready"
    );
    h.close_application().await.unwrap();
}
