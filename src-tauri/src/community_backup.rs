//! Offline, two-store maintenance. No IPC, provider call, key generation or vault
//! export. Both Store pools and Community writers must be drained before work.
mod io;
use crate::community_identity::registry::{gate::Lease, lifecycle, RegistrationRegistry};
use io::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Connection, Row};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    Busy,
    PolicyDenied,
    InvalidBundle,
    Io,
    Storage,
    IdempotencyConflict,
    RestorePending,
}
impl From<std::io::Error> for BackupError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}
impl From<sqlx::Error> for BackupError {
    fn from(_: sqlx::Error) -> Self {
        Self::Storage
    }
}
impl From<lifecycle::LifecycleError> for BackupError {
    fn from(_: lifecycle::LifecycleError) -> Self {
        Self::InvalidBundle
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestorePolicy {
    SameInstallation,
    FreshInstallationRelocation,
}
impl RestorePolicy {
    fn label(self) -> &'static str {
        match self {
            Self::SameInstallation => "same_installation",
            Self::FreshInstallationRelocation => "fresh_installation_relocation",
        }
    }
}
#[derive(Clone)]
pub struct OfflineCommunityBackup {
    root: PathBuf,
    registry: RegistrationRegistry,
    #[cfg(test)]
    fault: std::sync::Arc<std::sync::atomic::AtomicU8>,
    #[cfg(test)]
    owner_entered: std::sync::Arc<tokio::sync::Notify>,
}
struct Bundle {
    dir: PathBuf,
    manifest: Value,
    manifest_hash: String,
    main: MainInfo,
}
impl OfflineCommunityBackup {
    pub fn new(app_data: &Path, registry: RegistrationRegistry) -> Result<Self, BackupError> {
        Ok(Self {
            root: canonical_dir(app_data)?,
            registry,
            #[cfg(test)]
            fault: Default::default(),
            #[cfg(test)]
            owner_entered: Default::default(),
        })
    }
    /// Read the durable terminal receipt without needing the original bundle.
    pub fn receipt(&self, request_id: &str) -> Result<Option<Value>, BackupError> {
        if !request(request_id) {
            return Err(BackupError::PolicyDenied);
        }
        let path = self
            .root
            .join("community-restore-results")
            .join(format!("{}.json", key(request_id)));
        if !path.exists() {
            return Ok(None);
        }
        let v = read_json(&path)?;
        if !self.valid_receipt(&v, request_id) {
            return Err(BackupError::InvalidBundle);
        }
        Ok(Some(v))
    }
    /// Requires all Store-derived pools closed/dropped. A clone of a pool keeps
    /// the shared owner even after the Store wrapper was dropped.
    pub async fn create(&self, destination: &Path, request_id: &str) -> Result<Value, BackupError> {
        if !request(request_id) {
            return Err(BackupError::PolicyDenied);
        }
        let this = self.clone();
        let dest = destination.to_path_buf();
        let id = request_id.to_string();
        tokio::spawn(async move {
            let _app = crate::app_ownership::acquire(&this.root.join("foundation.sqlite3"), true)
                .map_err(|_| BackupError::Busy)?;
            if this.registry.restore_fence().exists() || crate::app_ownership::pending(&this.root) {
                return Err(BackupError::RestorePending);
            }
            #[cfg(test)]
            this.owner_entered.notify_one();
            let lease = this.registry.maintenance().await;
            this.create_inner(&dest, &id, &lease).await
        })
        .await
        .map_err(|_| BackupError::Storage)?
    }
    async fn create_inner(
        &self,
        dest: &Path,
        id: &str,
        lease: &Lease,
    ) -> Result<Value, BackupError> {
        if !dest.is_absolute() || dest.file_name().is_none() {
            return Err(BackupError::PolicyDenied);
        }
        let parent = canonical_dir(dest.parent().ok_or(BackupError::Io)?)?;
        let dest = parent.join(dest.file_name().unwrap());
        if dest.exists() {
            let old = self.validate_bundle(&dest, lease).await?;
            if old.manifest["requestId"] != id
                || old.manifest["sourceRootBinding"] != path_binding(&self.root)
                || old.manifest["registryBinding"] != self.registry.host_binding()
            {
                return Err(BackupError::IdempotencyConflict);
            }
            return Ok(old.manifest);
        }
        let info = main_info(&self.root.join("foundation.sqlite3"), false).await?;
        let states = self
            .registry
            .maintenance_states(lease)
            .await
            .map_err(|_| BackupError::Storage)?;
        self.check_live(&info, &states)?;
        let final_dest = dest;
        let dest = parent.join(format!(".aihome-backup-{}", uuid::Uuid::new_v4()));
        private_dir(&dest)?;
        vacuum(
            &self.root.join("foundation.sqlite3"),
            &dest.join("application.sqlite3"),
        )
        .await?;
        let registry =
            lifecycle::snapshot_with_lease(&self.registry, &dest.join("community.sqlite3"), lease)
                .await?;
        main_info(&dest.join("application.sqlite3"), true).await?;
        lifecycle::inspect_snapshot(
            &self.registry,
            &dest.join("community.sqlite3"),
            &registry.sha256,
            lease,
        )
        .await?;
        let manifest = json!({"format":"ai-home-two-store-backup/1","requestId":id,"createdAt":chrono::Utc::now().to_rfc3339(),"sourceRootBinding":path_binding(&self.root),"registryBinding":self.registry.host_binding(),"appSchema":crate::persistence::CURRENT_SCHEMA,"registrySchema":4,"appSha256":hash(&dest.join("application.sqlite3"))?,"communitySha256":registry.sha256,"profileCount":info.ids.len(),"communityProfileCount":states.len(),"vaultSecretsIncluded":false,"importedMetadataRequiresReconciliation":true});
        // Publish only after both complete files have been synchronized.
        sync_parent(&dest.join("community.sqlite3"))?;
        write_once(&dest.join("manifest.json"), &manifest)?;
        self.validate_bundle(&dest, lease).await?;
        if std::fs::symlink_metadata(&final_dest).is_ok() {
            return Err(BackupError::IdempotencyConflict);
        }
        std::fs::rename(&dest, &final_dest)?;
        sync_parent(&final_dest)?;
        Ok(manifest)
    }
    fn check_live(&self, info: &MainInfo, states: &[(String, String)]) -> Result<(), BackupError> {
        if info.pin.as_deref() != Some(self.registry.host_binding().as_str())
            && !(info.pin.is_none() && info.ids.is_empty() && states.is_empty())
        {
            return Err(BackupError::PolicyDenied);
        }
        if states.iter().any(|(id, state)| {
            state == "deleting" || (state != "deleted" && !info.ids.contains(id))
        }) {
            return Err(BackupError::PolicyDenied);
        }
        Ok(())
    }
    async fn validate_bundle(&self, path: &Path, lease: &Lease) -> Result<Bundle, BackupError> {
        let dir = canonical_dir(path)?;
        let names: BTreeSet<_> = std::fs::read_dir(&dir)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<_, _>>()?;
        let expected: BTreeSet<_> = ["application.sqlite3", "community.sqlite3", "manifest.json"]
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect();
        if names != expected
            || dir.join("application.sqlite3").metadata()?.len() > 1024 * 1024 * 1024
            || dir.join("community.sqlite3").metadata()?.len() > 64 * 1024 * 1024
        {
            return Err(BackupError::InvalidBundle);
        }
        let manifest = read_json(&dir.join("manifest.json"))?;
        let keys = [
            "format",
            "requestId",
            "createdAt",
            "sourceRootBinding",
            "registryBinding",
            "appSchema",
            "registrySchema",
            "appSha256",
            "communitySha256",
            "profileCount",
            "communityProfileCount",
            "vaultSecretsIncluded",
            "importedMetadataRequiresReconciliation",
        ];
        if manifest
            .as_object()
            .is_none_or(|m| m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)))
            || manifest["format"] != "ai-home-two-store-backup/1"
            || manifest["appSchema"] != crate::persistence::CURRENT_SCHEMA
            || manifest["registrySchema"] != 4
            || manifest["vaultSecretsIncluded"] != false
            || manifest["importedMetadataRequiresReconciliation"] != true
            || !request(manifest["requestId"].as_str().unwrap_or(""))
            || chrono::DateTime::parse_from_rfc3339(manifest["createdAt"].as_str().unwrap_or(""))
                .is_err()
            || [
                "sourceRootBinding",
                "registryBinding",
                "appSha256",
                "communitySha256",
            ]
            .iter()
            .any(|k| !is_hash(&manifest[k]))
        {
            return Err(BackupError::InvalidBundle);
        }
        if hash(&dir.join("application.sqlite3"))? != manifest["appSha256"]
            || hash(&dir.join("community.sqlite3"))? != manifest["communitySha256"]
        {
            return Err(BackupError::InvalidBundle);
        }
        let main = main_info(&dir.join("application.sqlite3"), true).await?;
        let states = lifecycle::inspect_snapshot(
            &self.registry,
            &dir.join("community.sqlite3"),
            manifest["communitySha256"].as_str().unwrap(),
            lease,
        )
        .await?;
        if manifest["profileCount"].as_u64() != Some(main.ids.len() as u64)
            || manifest["communityProfileCount"].as_u64() != Some(states.len() as u64)
            || states
                .iter()
                .any(|(id, s)| s != "deleted" && !main.ids.contains(id))
            || !(main.pin.as_deref() == manifest["registryBinding"].as_str()
                || (main.pin.is_none() && main.ids.is_empty() && states.is_empty()))
        {
            return Err(BackupError::InvalidBundle);
        }
        let manifest_hash = hash(&dir.join("manifest.json"))?;
        Ok(Bundle {
            dir,
            manifest,
            manifest_hash,
            main,
        })
    }
    /// Explicit offline restore. Existing live Community state is never rolled
    /// back; imported entries require reconciliation. No vault/API calls occur.
    pub async fn restore(
        &self,
        bundle: &Path,
        request_id: &str,
        policy: RestorePolicy,
    ) -> Result<Value, BackupError> {
        if !request(request_id) {
            return Err(BackupError::PolicyDenied);
        }
        let this = self.clone();
        let path = bundle.to_path_buf();
        let id = request_id.to_string();
        tokio::spawn(async move { this.restore_inner(&path, &id, policy).await })
            .await
            .map_err(|_| BackupError::Storage)?
    }
    /// Finish only the housekeeping of an already committed restore receipt.
    /// Does not reapply data and does not require the original input bundle.
    pub async fn finish_cleanup(&self, request_id: &str) -> Result<Value, BackupError> {
        if !request(request_id) {
            return Err(BackupError::PolicyDenied);
        }
        let this = self.clone();
        let id = request_id.to_string();
        tokio::spawn(async move {
            let main = this.root.join("foundation.sqlite3");
            let _owner =
                crate::app_ownership::acquire(&main, true).map_err(|_| BackupError::Busy)?;
            let _lease = this.registry.maintenance().await;
            let result = this.receipt(&id)?.ok_or(BackupError::InvalidBundle)?;
            let pending = this.root.join(crate::app_ownership::PENDING);
            if pending.exists() && hash(&main)? != result["restoredAppSha256"] {
                return Err(BackupError::InvalidBundle);
            }
            let mut c = connection(&main, true, false).await?;
            let pin: Option<String> = sqlx::query_scalar(
                "SELECT registry_binding FROM community_host_binding WHERE id=1",
            )
            .fetch_optional(&mut c)
            .await?;
            c.close().await?;
            if pin.as_deref() != Some(this.registry.host_binding().as_str()) {
                return Err(BackupError::PolicyDenied);
            }
            this.finish_markers(&pending, result["fingerprint"].as_str().unwrap(), &id)?;
            Ok(result)
        })
        .await
        .map_err(|_| BackupError::Storage)?
    }
    async fn restore_inner(
        &self,
        path: &Path,
        id: &str,
        policy: RestorePolicy,
    ) -> Result<Value, BackupError> {
        let main_path = self.root.join("foundation.sqlite3");
        let _app =
            crate::app_ownership::acquire(&main_path, true).map_err(|_| BackupError::Busy)?;
        let lease = self.registry.maintenance().await;
        let bundle = self.validate_bundle(path, &lease).await?;
        let fingerprint = key(&json!([
            "offline-community-restore/1",
            id,
            bundle.dir.to_str(),
            bundle.manifest_hash,
            policy.label(),
            self.registry.host_binding(),
            path_binding(&self.root)
        ])
        .to_string());
        let pending = self.root.join(crate::app_ownership::PENDING);
        let work_parent = self.root.join("community-restore-work");
        let result_parent = self.root.join("community-restore-results");
        ensure_dir(&work_parent)?;
        ensure_dir(&result_parent)?;
        let result_path = result_parent.join(format!("{}.json", key(id)));
        let work = work_parent.join(key(id));
        if result_path.exists() {
            let result = read_json(&result_path)?;
            if !self.valid_receipt(&result, id) {
                return Err(BackupError::InvalidBundle);
            }
            if result["fingerprint"] != fingerprint {
                return Err(BackupError::IdempotencyConflict);
            }
            if pending.exists() && hash(&main_path)? != result["restoredAppSha256"] {
                return Err(BackupError::InvalidBundle);
            }
            self.finish_markers(&pending, &fingerprint, id)?;
            return Ok(result);
        }
        if self.registry.restore_fence().exists()
            && (!pending.exists()
                || read_json(&self.registry.restore_fence())?["fingerprint"] != fingerprint)
        {
            return Err(BackupError::RestorePending);
        }
        let current_states = self
            .registry
            .maintenance_states(&lease)
            .await
            .map_err(|_| BackupError::Storage)?;
        if pending.exists() {
            let pointer = read_json(&pending)?;
            if pointer["format"] != "ai-home-community-restore-pending/1"
                || pointer["requestId"] != id
            {
                return Err(BackupError::InvalidBundle);
            }
            if pointer["fingerprint"] != fingerprint {
                return Err(BackupError::RestorePending);
            }
            let journal = read_json(&work.join("request.json"))?;
            if journal["fingerprint"] != fingerprint
                || journal["registryBinding"] != self.registry.host_binding()
                || journal["format"] != "ai-home-community-restore/1"
                || journal["requestId"] != id
                || journal["bundlePath"].as_str() != bundle.dir.to_str()
                || journal["bundleManifestSha256"] != bundle.manifest_hash
                || journal["policy"] != policy.label()
                || journal["safetyPath"].as_str()
                    != self
                        .root
                        .join("community-safety-backups")
                        .join(key(id))
                        .to_str()
            {
                return Err(BackupError::InvalidBundle);
            }
        } else {
            let current = main_info(&main_path, false).await?;
            self.check_live(&current, &current_states)?;
            if current.active_experimental || (current.pin.is_some() && current_states.is_empty()) {
                return Err(BackupError::PolicyDenied);
            }
            if policy == RestorePolicy::FreshInstallationRelocation {
                if !current.ids.is_empty() || current.pin.is_some() || !current_states.is_empty() {
                    return Err(BackupError::PolicyDenied);
                }
            } else if bundle.manifest["registryBinding"] != self.registry.host_binding() {
                return Err(BackupError::PolicyDenied);
            }
            self.check_coverage(&bundle, &current_states)?;
            let safety_parent = self.root.join("community-safety-backups");
            ensure_dir(&safety_parent)?;
            let safety = safety_parent.join(key(id));
            ensure_dir(&work)?;
            let journal = json!({"format":"ai-home-community-restore/1","requestId":id,"fingerprint":fingerprint,"registryBinding":self.registry.host_binding(),"bundlePath":bundle.dir.to_str(),"bundleManifestSha256":bundle.manifest_hash,"policy":policy.label(),"safetyPath":safety.to_str()});
            write_once(&work.join("request.json"), &journal)?;
            write_once(
                &pending,
                &json!({"format":"ai-home-community-restore-pending/1","requestId":id,"fingerprint":fingerprint}),
            )?;
        }
        self.check_coverage(&bundle, &current_states)?;
        let fence = self.registry.restore_fence();
        write_once(
            &fence,
            &json!({"format":"ai-home-community-restore-fence/1","fingerprint":fingerprint,"rootBinding":path_binding(&self.root)}),
        )?;
        self.check_coverage(&bundle, &current_states)?;
        let journal = read_json(&work.join("request.json"))?;
        let safety = PathBuf::from(
            journal["safetyPath"]
                .as_str()
                .ok_or(BackupError::InvalidBundle)?,
        );
        let safety_receipt = work.join("safety.json");
        if safety_receipt.exists() {
            let receipt = read_json(&safety_receipt)?;
            let saved = self.validate_bundle(&safety, &lease).await?;
            if receipt["fingerprint"] != fingerprint
                || receipt["manifestSha256"] != saved.manifest_hash
            {
                return Err(BackupError::InvalidBundle);
            }
        } else {
            self.create_inner(&safety, &format!("safety-{}", key(id)), &lease)
                .await?;
            write_once(
                &safety_receipt,
                &json!({"fingerprint":fingerprint,"manifestSha256":hash(&safety.join("manifest.json"))?}),
            )?;
        }
        let ready_path = work.join("ready.json");
        let staged = work.join("application.ready.sqlite3");
        if !ready_path.exists() {
            let private_registry = work.join("input-community.sqlite3");
            copy_checked(
                &bundle.dir.join("community.sqlite3"),
                &private_registry,
                bundle.manifest["communitySha256"].as_str().unwrap(),
            )?;
            lifecycle::restore_with_lease(
                &self.registry,
                &private_registry,
                bundle.manifest["communitySha256"].as_str().unwrap(),
                &lease,
            )
            .await?;
            #[cfg(test)]
            self.fail_at(1)?;
            let states = self
                .registry
                .maintenance_states(&lease)
                .await
                .map_err(|_| BackupError::Storage)?;
            let deleted: Vec<String> = states
                .iter()
                .filter(|(_, s)| s == "deleted")
                .map(|(id, _)| id.clone())
                .collect();
            if staged.exists() {
                regular(&staged)?;
                std::fs::remove_file(&staged)?;
            }
            let mut input = std::fs::File::open(bundle.dir.join("application.sqlite3"))?;
            let mut output = private_file(&staged)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
            drop(output);
            if hash(&staged)? != bundle.manifest["appSha256"] {
                return Err(BackupError::InvalidBundle);
            }
            main_info(&staged, true).await?;
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(&staged)
                        .create_if_missing(false)
                        .foreign_keys(true),
                )
                .await?;
            let mut tx = pool.begin().await?;
            for profile in &deleted {
                crate::connections::profile_deletion::delete_rows(&mut tx, profile)
                    .await
                    .map_err(|_| BackupError::Storage)?;
            }
            sqlx::query("DELETE FROM community_host_binding")
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO community_host_binding VALUES(1,?)")
                .bind(self.registry.host_binding())
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            pool.close().await;
            drop(pool);
            main_info(&staged, true).await?;
            sync_file(&staged)?;
            let pruned: Vec<_> = deleted
                .iter()
                .filter(|id| bundle.main.ids.contains(*id))
                .collect();
            write_once(
                &ready_path,
                &json!({"fingerprint":fingerprint,"appSha256":hash(&staged)?,"prunedProfileCount":pruned.len(),"prunedProfilesSha256":key(&json!(pruned).to_string())}),
            )?;
        }
        let ready = read_json(&ready_path)?;
        if ready["fingerprint"] != fingerprint
            || !is_hash(&ready["appSha256"])
            || !is_hash(&ready["prunedProfilesSha256"])
            || ready["prunedProfileCount"]
                .as_u64()
                .is_none_or(|n| n > bundle.main.ids.len() as u64)
        {
            return Err(BackupError::InvalidBundle);
        }
        // Recheck source bytes after import/staging, before swapping the app DB.
        let fresh = self.validate_bundle(&bundle.dir, &lease).await?;
        if fresh.manifest_hash != bundle.manifest_hash {
            return Err(BackupError::InvalidBundle);
        }
        let previous = work.join("application.previous.sqlite3");
        let already_applied = main_path.exists() && hash(&main_path)? == ready["appSha256"];
        if !already_applied {
            if hash(&staged)? != ready["appSha256"] {
                return Err(BackupError::InvalidBundle);
            }
            if main_path.exists() {
                if previous.exists() {
                    return Err(BackupError::PolicyDenied);
                }
                checkpoint(&main_path).await?;
                std::fs::rename(&main_path, &previous)?;
                sync_parent(&main_path)?;
                sync_parent(&previous)?;
            } else if !previous.exists() {
                return Err(BackupError::InvalidBundle);
            }
            #[cfg(test)]
            self.fail_at(2)?;
            std::fs::rename(&staged, &main_path)?;
            sync_parent(&main_path)?;
        }
        main_info(&main_path, false).await?;
        if hash(&main_path)? != ready["appSha256"] {
            return Err(BackupError::InvalidBundle);
        }
        #[cfg(test)]
        self.fail_at(3)?;
        let journal = read_json(&work.join("request.json"))?;
        let result = json!({"format":"ai-home-community-restore-result/1","applied":true,"requestId":id,"fingerprint":fingerprint,"sourceManifestSha256":bundle.manifest_hash,"restoredAppSha256":ready["appSha256"],"registryBinding":self.registry.host_binding(),"prunedProfileCount":ready["prunedProfileCount"],"prunedProfilesSha256":ready["prunedProfilesSha256"],"importedMetadataRequiresReconciliation":true,"safetyPath":journal["safetyPath"]});
        write_once(&result_path, &result)?;
        #[cfg(test)]
        self.fail_at(4)?;
        self.finish_markers(&pending, &fingerprint, id)?;
        Ok(result)
    }
    fn valid_receipt(&self, v: &Value, id: &str) -> bool {
        let keys = [
            "format",
            "applied",
            "requestId",
            "fingerprint",
            "sourceManifestSha256",
            "restoredAppSha256",
            "registryBinding",
            "prunedProfileCount",
            "prunedProfilesSha256",
            "importedMetadataRequiresReconciliation",
            "safetyPath",
        ];
        v.as_object()
            .is_some_and(|m| m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k)))
            && v["format"] == "ai-home-community-restore-result/1"
            && v["applied"] == true
            && v["requestId"] == id
            && v["registryBinding"] == self.registry.host_binding()
            && v["importedMetadataRequiresReconciliation"] == true
            && [
                "fingerprint",
                "sourceManifestSha256",
                "restoredAppSha256",
                "registryBinding",
                "prunedProfilesSha256",
            ]
            .iter()
            .all(|k| is_hash(&v[k]))
            && v["prunedProfileCount"].as_u64().is_some()
            && v["safetyPath"].as_str()
                == self
                    .root
                    .join("community-safety-backups")
                    .join(key(id))
                    .to_str()
    }
    fn finish_markers(
        &self,
        pending: &Path,
        fingerprint: &str,
        id: &str,
    ) -> Result<(), BackupError> {
        let fence = self.registry.restore_fence();
        for p in [pending, &fence] {
            if p.exists() && read_json(p)?["fingerprint"] != fingerprint {
                return Err(BackupError::RestorePending);
            }
        }
        if pending.exists() {
            let v = read_json(pending)?;
            if v["format"] != "ai-home-community-restore-pending/1" || v["requestId"] != id {
                return Err(BackupError::InvalidBundle);
            }
        }
        if fence.exists() {
            let v = read_json(&fence)?;
            if v["format"] != "ai-home-community-restore-fence/1"
                || v["rootBinding"] != path_binding(&self.root)
            {
                return Err(BackupError::InvalidBundle);
            }
        }
        // Main may open after the first removal; the registry remains fenced
        // until cleanup finishes. A crash here affects availability, not truth.
        for p in [pending, &fence] {
            if p.exists() {
                std::fs::remove_file(p)?;
                sync_parent(p)?;
            }
        }
        Ok(())
    }
    fn check_coverage(
        &self,
        bundle: &Bundle,
        states: &[(String, String)],
    ) -> Result<(), BackupError> {
        if states
            .iter()
            .any(|(id, s)| s == "deleting" || (s != "deleted" && !bundle.main.ids.contains(id)))
        {
            return Err(BackupError::PolicyDenied);
        }
        Ok(())
    }
    #[cfg(test)]
    fn fail_at(&self, point: u8) -> Result<(), BackupError> {
        if self.fault.load(std::sync::atomic::Ordering::SeqCst) == point {
            Err(BackupError::Io)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
