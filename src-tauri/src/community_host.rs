//! Native Community deletion and managed maintenance/recovery host.
//! Production desktop and registration IPC remain gated on direction/capability
//! policy and command/result integration in later 2B-5.3 substeps.
use crate::community_identity::{
    registry::{lifecycle, RegistrationRegistry},
    tokens::CommunityTokenStore,
    CommunityKeyStore,
};
pub mod maintenance;
pub mod local;

#[derive(Clone)]
pub struct CommunityLifecycle {
    registry: RegistrationRegistry,
    keys: CommunityKeyStore,
    tokens: CommunityTokenStore,
}

impl CommunityLifecycle {
    pub(crate) fn binding(&self) -> String {
        self.registry.host_binding()
    }
    /// All coordinators for this app must share the same registry owner. No
    /// vault access, key generation or network request occurs at construction.
    pub fn new(
        registry: RegistrationRegistry,
        keys: CommunityKeyStore,
        tokens: CommunityTokenStore,
    ) -> Self {
        Self {
            registry,
            keys,
            tokens,
        }
    }

    pub(crate) async fn delete_profile(&self, profile: &str) -> Result<(), &'static str> {
        lifecycle::delete_profile(&self.registry, &self.keys, &self.tokens, profile)
            .await
            .map(|_| ())
            .map_err(|e| match e {
                lifecycle::LifecycleError::Identity(_) => "CREDENTIAL_UNAVAILABLE",
                _ => "STORAGE_ERROR",
            })
    }
}
