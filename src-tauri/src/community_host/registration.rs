//! Explicit native registration network adapter. No production desktop routing.
use super::{
    local::{LocalAccess, LocalCapability},
    CommunityLifecycle,
};
use crate::community_identity::{
    registry::{
        finish::{FinishError, RegistrationFinish},
        start::{RegistrationStart, StartError, StartOutcome},
        PreparationState, RegistryError,
    },
    CommunityScope,
};
use crate::domain::schema;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// A fresh host grant plus an optional invite held outside the JSON envelope.
/// Not deserializable or Debug; never persist or project the invite.
pub struct RegistrationAccess {
    pub(super) local: LocalAccess,
    invite: Option<Zeroizing<String>>,
    start: RegistrationStart,
    timeout: std::time::Duration,
    #[cfg(test)]
    loopback: Option<reqwest::Url>,
}
impl RegistrationAccess {
    pub fn new(
        scope: CommunityScope,
        capabilities: Vec<LocalCapability>,
        invite: Option<Zeroizing<String>>,
    ) -> Self {
        let timeout = std::time::Duration::from_secs(10);
        Self {
            local: LocalAccess::new(scope, capabilities),
            invite,
            start: RegistrationStart::new(timeout).expect("fixed valid HTTP timeout"),
            timeout,
            #[cfg(test)]
            loopback: None,
        }
    }
    pub(super) fn valid_for(&self, cap: LocalCapability) -> bool {
        if cap != LocalCapability::Start {
            return self.invite.is_none();
        }
        self.invite
            .as_ref()
            .is_some_and(|s| !s.is_empty() && s.len() <= 200 && !s.contains(['\n', '\r', '\0']))
    }
    pub(super) fn invite_digest(&self) -> Option<String> {
        self.invite
            .as_ref()
            .map(|s| format!("{:x}", Sha256::digest(s.as_bytes())))
    }
    #[cfg(test)]
    pub(crate) fn set_loopback(&mut self, url: reqwest::Url) {
        self.start.set_loopback(url.clone());
        self.loopback = Some(url);
    }
    pub(super) async fn perform(
        &self,
        service: &CommunityLifecycle,
        scope: &CommunityScope,
        cap: LocalCapability,
        payload: &Value,
    ) -> Result<Value, &'static str> {
        let finish =
            RegistrationFinish::new(self.timeout, service.tokens.clone()).map_err(safe_finish)?;
        #[cfg(test)]
        let finish = {
            let mut finish = finish;
            if let Some(url) = self.loopback.clone() {
                finish.set_loopback(url);
            }
            finish
        };
        match cap {
            LocalCapability::Start | LocalCapability::Complete => {
                let prep = service.registry.get(scope).await.map_err(safe_registry)?;
                if prep.revision() != payload["registrationRevision"].as_i64().unwrap() {
                    return Err("REVISION_CONFLICT");
                }
                if prep.state() != PreparationState::Ready {
                    return Err("REGISTRATION_RECONCILIATION_REQUIRED");
                }
            }
            _ => {}
        }
        let (phase, revision) = match cap {
            LocalCapability::Start => {
                let result = self
                    .start
                    .start(
                        &service.registry,
                        &service.keys,
                        scope,
                        self.invite
                            .as_ref()
                            .ok_or("CREDENTIAL_UNAVAILABLE")?
                            .as_str(),
                    )
                    .await
                    .map_err(safe_start)?;
                let phase = match result {
                    StartOutcome::Challenge(_) => "awaiting_complete",
                    StartOutcome::RecoveryRequired { .. } => "recovery_required",
                };
                // Start never proves token custody, including a cached committed response.
                let revision = finish
                    .progress(&service.registry, scope)
                    .await
                    .map_err(safe_finish)?
                    .map_or(0, |p| p.revision);
                (phase, revision)
            }
            LocalCapability::Complete => {
                let p = finish
                    .complete(&service.registry, &service.keys, scope)
                    .await
                    .map_err(safe_finish)?;
                ("credential_available", p.revision)
            }
            LocalCapability::RetryComplete => {
                let p = finish
                    .retry_complete(
                        &service.registry,
                        &service.keys,
                        scope,
                        payload["finishRevision"].as_i64().unwrap(),
                    )
                    .await
                    .map_err(safe_finish)?;
                ("credential_available", p.revision)
            }
            LocalCapability::RecoverAccess => {
                let p = finish
                    .recover(
                        &service.registry,
                        &service.keys,
                        scope,
                        payload["finishRevision"].as_i64().unwrap(),
                    )
                    .await
                    .map_err(safe_finish)?;
                ("credential_available", p.revision)
            }
            _ => return Err("POLICY_DENIED"),
        };
        Ok(json!({"phase":phase,"finishRevision":revision,"onlineStatusKnown":false}))
    }
}
fn safe_registry(e: RegistryError) -> &'static str {
    match e {
        RegistryError::Identity(_) => "CREDENTIAL_UNAVAILABLE",
        RegistryError::Conflict => "REVISION_CONFLICT",
        RegistryError::NotFound => "NOT_FOUND",
        RegistryError::PolicyDenied => "POLICY_DENIED",
        RegistryError::Busy => "RESOURCE_UNAVAILABLE",
        RegistryError::ProvisioningUncertain => "REGISTRATION_RECONCILIATION_REQUIRED",
        _ => "STORAGE_ERROR",
    }
}
fn safe_start(e: StartError) -> &'static str {
    match e {
        StartError::Registry(e) => safe_registry(e),
        StartError::Identity(_) => "CREDENTIAL_UNAVAILABLE",
        StartError::InvalidResponse | StartError::ResponseTooLarge => {
            "REGISTRATION_RECONCILIATION_REQUIRED"
        }
        _ => "RESOURCE_UNAVAILABLE",
    }
}
fn safe_finish(e: FinishError) -> &'static str {
    match e {
        FinishError::Registry(e) => safe_registry(e),
        FinishError::Identity(_) => "CREDENTIAL_UNAVAILABLE",
        FinishError::Http(e) => safe_start(e),
        _ => "REGISTRATION_RECONCILIATION_REQUIRED",
    }
}
pub(super) fn terminal(r: &Value, v: &Value, contract: &Value) -> Result<Value, &'static str> {
    if v["ok"] == false {
        return super::local::terminal(r, v, contract);
    }
    if v.as_object().is_none_or(|o| o.len() != 4)
        || v["contractVersion"] != 1
        || v["requestId"] != r["requestId"]
        || v["ok"] != true
        || !schema::valid(&v["result"], &contract["result"])
    {
        return Err("STORAGE_ERROR");
    }
    if v["result"]["phase"] == "credential_available"
        && v["result"]["finishRevision"].as_i64().is_none_or(|n| n < 1)
    {
        return Err("STORAGE_ERROR");
    }
    Ok(v.clone())
}
