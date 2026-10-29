//! Opt-in desktop ownership and policy. Policy is loaded from the local app data
//! directory at startup, never from IPC. No registration or recovery is automatic.
use super::{
    local::{LocalAccess, LocalCapability},
    maintenance::CommunityHost,
    registration::RegistrationAccess,
    CommunityLifecycle,
};
use crate::{
    application::{Application, Dispatch},
    community_identity::{
        registry::RegistrationRegistry, tokens::CommunityTokenStore, CommunityKeyStore,
        CommunityScope,
    },
    persistence::Store,
    vault::Vault,
};
use serde_json::Value;
use sqlx::Connection;
use std::{collections::HashSet, path::Path, sync::Arc};
use zeroize::Zeroizing;

const POLICY: &str = "community.desktop.json";
struct Target {
    scope: CommunityScope,
    commands: Vec<String>,
}
pub(crate) struct DesktopPolicy {
    targets: Vec<Target>,
    #[cfg(test)]
    loopback: Option<reqwest::Url>,
}
fn exact(v: &Value, keys: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|o| o.len() == keys.len() && keys.iter().all(|k| o.contains_key(*k)))
}
fn capability(command: &str) -> Option<(LocalCapability, bool)> {
    use LocalCapability::*;
    Some(match command {
        "community.registration.status" => (Status, false),
        "community.registration.progress" => (Progress, false),
        "community.registration.access.status" => (AccessStatus, false),
        "community.registration.prepare" => (Prepare, false),
        "community.registration.provision" => (Provision, false),
        "community.registration.recover" => (Recover, false),
        "community.registration.start" => (Start, true),
        "community.registration.complete" => (Complete, true),
        "community.registration.complete.retry" => (RetryComplete, true),
        "community.registration.access.recover" => (RecoverAccess, true),
        _ => return None,
    })
}
impl DesktopPolicy {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > 65536 {
            return Err("COMMUNITY_POLICY_INVALID");
        }
        let v: Value = serde_json::from_slice(bytes).map_err(|_| "COMMUNITY_POLICY_INVALID")?;
        if !exact(&v, &["schemaVersion", "targets"]) || v["schemaVersion"] != 1 {
            return Err("COMMUNITY_POLICY_INVALID");
        }
        let items = v["targets"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 32)
            .ok_or("COMMUNITY_POLICY_INVALID")?;
        let mut profiles = HashSet::new();
        let mut targets = Vec::new();
        for item in items {
            if !exact(
                item,
                &[
                    "profileId",
                    "apiRoot",
                    "communityOrigin",
                    "communityInstance",
                    "commands",
                ],
            ) {
                return Err("COMMUNITY_POLICY_INVALID");
            }
            let string = |k: &str| item[k].as_str().ok_or("COMMUNITY_POLICY_INVALID");
            let profile = string("profileId")?;
            if !profiles.insert(profile.to_owned()) {
                return Err("COMMUNITY_POLICY_INVALID");
            }
            let api = string("apiRoot")?;
            let origin = string("communityOrigin")?;
            let scope = CommunityScope::new(profile, api, origin, string("communityInstance")?)
                .map_err(|_| "COMMUNITY_POLICY_INVALID")?;
            // This opt-in integration deliberately permits one same-origin forum
            // per profile. Cross-origin registration needs a separate reviewed policy.
            let api_url = reqwest::Url::parse(api).map_err(|_| "COMMUNITY_POLICY_INVALID")?;
            let origin_url = reqwest::Url::parse(origin).map_err(|_| "COMMUNITY_POLICY_INVALID")?;
            if api_url.origin() != origin_url.origin() || origin_url.path() != "/" {
                return Err("COMMUNITY_POLICY_INVALID");
            }
            let commands = item["commands"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 10)
                .ok_or("COMMUNITY_POLICY_INVALID")?;
            let mut seen = HashSet::new();
            let mut allowed = Vec::new();
            for cmd in commands {
                let cmd = cmd.as_str().ok_or("COMMUNITY_POLICY_INVALID")?;
                if capability(cmd).is_none() || !seen.insert(cmd.to_owned()) {
                    return Err("COMMUNITY_POLICY_INVALID");
                }
                allowed.push(cmd.to_owned());
            }
            targets.push(Target {
                scope,
                commands: allowed,
            });
        }
        Ok(Self {
            targets,
            #[cfg(test)]
            loopback: None,
        })
    }
    fn load(root: &Path) -> Result<Option<Self>, &'static str> {
        let path = root.join(POLICY);
        let m = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("COMMUNITY_POLICY_UNAVAILABLE"),
        };
        if !m.is_file() || m.file_type().is_symlink() || m.len() > 65536 {
            return Err("COMMUNITY_POLICY_INVALID");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if m.nlink() != 1 || m.mode() & 0o022 != 0 {
                return Err("COMMUNITY_POLICY_INVALID");
            }
        }
        // Bound the read even if the file grows between metadata and open.
        use std::io::Read;
        let f = std::fs::File::open(path).map_err(|_| "COMMUNITY_POLICY_UNAVAILABLE")?;
        let mut bytes = Vec::new();
        f.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| "COMMUNITY_POLICY_UNAVAILABLE")?;
        Self::parse(&bytes).map(Some)
    }
    fn grant(
        &self,
        request: &Value,
    ) -> Result<(CommunityScope, LocalCapability, bool), &'static str> {
        let command = request["command"].as_str().ok_or("VALIDATION_ERROR")?;
        let profile = request["profileId"].as_str().ok_or("POLICY_DENIED")?;
        let target = self
            .targets
            .iter()
            .find(|t| t.scope.profile_id() == profile)
            .ok_or("POLICY_DENIED")?;
        if !target.commands.iter().any(|c| c == command) {
            return Err("POLICY_DENIED");
        }
        let (cap, network) = capability(command).ok_or("POLICY_DENIED")?;
        Ok((target.scope.clone(), cap, network))
    }
    #[cfg(test)]
    pub(crate) fn set_loopback(&mut self, url: reqwest::Url) {
        self.loopback = Some(url);
    }
}

async fn existing_pin(root: &Path) -> Result<Option<String>, &'static str> {
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.join("foundation.sqlite3"))
            .read_only(true),
    )
    .await
    .map_err(|_| "COMMUNITY_STORAGE_UNAVAILABLE")?;
    let table: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='community_host_binding'",
    )
    .fetch_one(&mut c)
    .await
    .map_err(|_| "COMMUNITY_STORAGE_UNAVAILABLE")?;
    let pin = if table == 0 {
        None
    } else {
        sqlx::query_scalar("SELECT registry_binding FROM community_host_binding WHERE id=1")
            .fetch_optional(&mut c)
            .await
            .map_err(|_| "COMMUNITY_STORAGE_UNAVAILABLE")?
    };
    c.close()
        .await
        .map_err(|_| "COMMUNITY_STORAGE_UNAVAILABLE")?;
    Ok(pin)
}

/// Only the main bundled webview (or the exact debug dev server) may invoke.
/// The native Tauri-injected WebviewWindow supplies both values, never JSON.
pub(crate) fn local_caller(label: &str, url: &str) -> bool {
    caller(label, url, cfg!(debug_assertions))
}
fn caller(label: &str, url: &str, dev: bool) -> bool {
    if label != "main" {
        return false;
    }
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    if !u.username().is_empty() || u.password().is_some() {
        return false;
    }
    let bundled = (u.scheme() == "tauri"
        && u.host_str() == Some("localhost")
        && u.port().is_none())
        || (u.scheme() == "http" && u.host_str() == Some("tauri.localhost") && u.port().is_none());
    bundled
        || (dev
            && u.scheme() == "http"
            && u.host_str() == Some("127.0.0.1")
            && u.port() == Some(1420))
}

pub(crate) enum DesktopRuntime {
    Standard(Application),
    Community {
        host: CommunityHost,
        policy: DesktopPolicy,
    },
}
impl DesktopRuntime {
    pub(crate) async fn open(
        root: &Path,
        vault: Vault,
        picker: Option<Arc<dyn crate::imports::FilePicker>>,
    ) -> Result<Self, &'static str> {
        let policy = DesktopPolicy::load(root)?;
        if let Some(policy) = policy {
            // Never create a blank Main or auto-apply a legacy/native restore.
            if !root.join("foundation.sqlite3").is_file()
                || crate::app_ownership::pending(root)
                || std::fs::symlink_metadata(root.join("community.sqlite3.restore-fence.json"))
                    .is_ok()
                || crate::backup::host_restore_pending(root)
                    .map_err(|_| "COMMUNITY_RECOVERY_REQUIRED")?
            {
                return Err("COMMUNITY_RECOVERY_REQUIRED");
            }
            let pin = existing_pin(root).await?;
            if pin.is_some() && !root.join("community.sqlite3").is_file() {
                return Err("COMMUNITY_RECOVERY_REQUIRED");
            }
            let registry = RegistrationRegistry::open(&root.join("community.sqlite3"))
                .await
                .map_err(|_| "COMMUNITY_STORAGE_UNAVAILABLE")?;
            let service = CommunityLifecycle::new(
                registry,
                CommunityKeyStore::os(),
                CommunityTokenStore::os(),
            );
            let host = CommunityHost::new(root, service, vault, picker)
                .map_err(|_| "COMMUNITY_RECOVERY_REQUIRED")?;
            host.open_application()
                .await
                .map_err(|_| "COMMUNITY_RECOVERY_REQUIRED")?;
            Ok(Self::Community { host, policy })
        } else {
            // Removing the opt-in file cannot silently change an existing
            // Community installation back to an uncoordinated Store owner.
            if std::fs::symlink_metadata(root.join("community.sqlite3")).is_ok()
                || crate::app_ownership::pending(root)
            {
                return Err("COMMUNITY_POLICY_REQUIRED");
            }
            if std::fs::symlink_metadata(root.join("community.sqlite3.restore-fence.json")).is_ok()
                || (root.join("foundation.sqlite3").is_file()
                    && existing_pin(root).await?.is_some())
            {
                return Err("COMMUNITY_POLICY_REQUIRED");
            }
            crate::backup::apply_pending_restore(root)
                .await
                .map_err(|_| "APPLICATION_RESTORE_REQUIRED")?;
            let store = Store::open(&root.join("foundation.sqlite3"))
                .await
                .map_err(|_| "APPLICATION_STORAGE_UNAVAILABLE")?;
            let pinned: bool =
                sqlx::query_scalar::<_, i64>("SELECT count(*) FROM community_host_binding")
                    .fetch_one(&store.pool)
                    .await
                    .map_err(|_| "APPLICATION_STORAGE_UNAVAILABLE")?
                    != 0;
            if pinned {
                Application::new(store).close_for_maintenance().await;
                return Err("COMMUNITY_POLICY_REQUIRED");
            }
            let app = match picker {
                Some(p) => Application::with_file_picker(store, vault, p),
                None => Application::with_vault(store, vault),
            };
            Ok(Self::Standard(app))
        }
    }
    pub(crate) async fn dispatch(&self, request: Value) -> Result<Dispatch, &'static str> {
        match self {
            Self::Standard(app) => Ok(app.dispatch(request).await),
            Self::Community { host, .. } => host
                .dispatch_queued(request)
                .await
                .map_err(|_| "COMMUNITY_HOST_UNAVAILABLE"),
        }
    }
    pub(crate) async fn registration(
        &self,
        label: &str,
        url: &str,
        request: Value,
        invite: Option<Zeroizing<String>>,
    ) -> Result<Value, &'static str> {
        if !local_caller(label, url) {
            return Err("POLICY_DENIED");
        }
        let Self::Community { host, policy } = self else {
            return Err("COMMUNITY_NOT_ENABLED");
        };
        let (scope, cap, network) = policy.grant(&request)?;
        if network {
            let access = RegistrationAccess::new(scope, vec![cap], invite);
            #[cfg(test)]
            let access = {
                let mut access = access;
                if let Some(url) = policy.loopback.clone() {
                    access.set_loopback(url);
                }
                access
            };
            host.dispatch_registration(request, access).await
        } else {
            if invite.is_some() {
                return Err("VALIDATION_ERROR");
            }
            host.dispatch_local(request, LocalAccess::new(scope, vec![cap]))
                .await
        }
        .map_err(|_| "COMMUNITY_HOST_UNAVAILABLE")
    }
}

#[cfg(test)]
mod tests;
