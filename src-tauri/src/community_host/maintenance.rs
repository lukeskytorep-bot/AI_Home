//! Trusted native host ownership. Not an IPC surface or an authorization grant.
//! The host never lends Application/Store/pool handles to callers. All ordinary
//! dispatch and maintenance must use this one handle. Desktop activation and
//! direction/capability checks are separate integration gates.
use super::CommunityLifecycle;
use crate::{
    application::{Application, Dispatch},
    community_backup::{BackupError, OfflineCommunityBackup, RestorePolicy},
    persistence::Store,
    vault::Vault,
};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HostStatus {
    Offline,
    Ready,
    Maintenance,
    RecoveryRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    Busy,
    Unavailable,
    Storage,
    Backup(BackupError),
}

struct Inner {
    root: PathBuf,
    service: CommunityLifecycle,
    vault: Vault,
    picker: Option<Arc<dyn crate::imports::FilePicker>>,
    backup: OfflineCommunityBackup,
    application: Arc<Mutex<Option<Application>>>,
    status: AtomicU8,
}

/// Clones share the same admission gate, application and maintenance worker.
#[derive(Clone)]
pub struct CommunityHost(Arc<Inner>);

enum Action {
    Open,
    Close,
    Create(PathBuf, String),
    Restore(PathBuf, String, RestorePolicy),
    Cleanup(String),
}

impl CommunityHost {
    /// Existing installation only: construction opens neither Store nor vault.
    /// A missing Main is never manufactured, even during recovery after swap.
    /// The registry must be the exact local community.sqlite3 owner.
    pub fn new(
        root: &Path,
        service: CommunityLifecycle,
        vault: Vault,
        picker: Option<Arc<dyn crate::imports::FilePicker>>,
    ) -> Result<Self, HostError> {
        let backup = OfflineCommunityBackup::new(root, service.registry.clone())
            .map_err(HostError::Backup)?;
        let root = std::fs::canonicalize(root).map_err(|_| HostError::Storage)?;
        if service.registry.storage_path() != root.join("community.sqlite3") {
            return Err(HostError::Unavailable);
        }
        let this = Self(Arc::new(Inner {
            root,
            service,
            vault,
            picker,
            backup,
            application: Arc::new(Mutex::new(None)),
            status: AtomicU8::new(HostStatus::Offline as u8),
        }));
        this.set_offline_status();
        Ok(this)
    }

    pub fn status(&self) -> HostStatus {
        match self.0.status.load(Ordering::Acquire) {
            0 => HostStatus::Offline,
            1 => HostStatus::Ready,
            2 => HostStatus::Maintenance,
            _ => HostStatus::RecoveryRequired,
        }
    }
    fn blocked(&self) -> bool {
        crate::app_ownership::pending(&self.0.root)
            || std::fs::symlink_metadata(self.0.service.registry.restore_fence()).is_ok()
            || !self.0.root.join("foundation.sqlite3").is_file()
            || crate::backup::host_restore_pending(&self.0.root).unwrap_or(true)
    }
    fn set_offline_status(&self) {
        let status = if self.blocked() {
            HostStatus::RecoveryRequired
        } else {
            HostStatus::Offline
        };
        self.0.status.store(status as u8, Ordering::Release);
    }

    /// Explicit bootstrap/reopen. Pending native restore stays in recovery;
    /// no restore, token reissue, key generation or network call is automatic.
    pub async fn open_application(&self) -> Result<(), HostError> {
        self.run(Action::Open).await.map(|_| ())
    }
    pub async fn close_application(&self) -> Result<(), HostError> {
        self.run(Action::Close).await.map(|_| ())
    }
    /// Busy/unavailable is a native host result, not a new frozen IPC error.
    pub async fn dispatch(&self, request: Value) -> Result<Dispatch, HostError> {
        if self.status() != HostStatus::Ready {
            return Err(if self.status() == HostStatus::Maintenance {
                HostError::Busy
            } else {
                HostError::Unavailable
            });
        }
        let guard = self
            .0
            .application
            .clone()
            .try_lock_owned()
            .map_err(|_| HostError::Busy)?;
        if self.status() != HostStatus::Ready {
            return Err(HostError::Busy);
        }
        if guard.is_none() {
            return Err(HostError::Unavailable);
        }
        tokio::spawn(async move {
            Ok(guard
                .as_ref()
                .ok_or(HostError::Unavailable)?
                .dispatch(request)
                .await)
        })
        .await
        .map_err(|_| HostError::Storage)?
    }
    pub async fn create_backup(
        &self,
        destination: &Path,
        request_id: &str,
    ) -> Result<Value, HostError> {
        self.run(Action::Create(destination.to_path_buf(), request_id.into()))
            .await
    }
    pub async fn restore(
        &self,
        bundle: &Path,
        request_id: &str,
        policy: RestorePolicy,
    ) -> Result<Value, HostError> {
        self.run(Action::Restore(
            bundle.to_path_buf(),
            request_id.into(),
            policy,
        ))
        .await
    }
    pub async fn finish_cleanup(&self, request_id: &str) -> Result<Value, HostError> {
        self.run(Action::Cleanup(request_id.into())).await
    }
    pub fn receipt(&self, request_id: &str) -> Result<Option<Value>, HostError> {
        self.0.backup.receipt(request_id).map_err(HostError::Backup)
    }

    async fn run(&self, action: Action) -> Result<Value, HostError> {
        let previous = self.0.status.load(Ordering::Acquire);
        if previous == HostStatus::Maintenance as u8 {
            return Err(HostError::Busy);
        }
        self.0
            .status
            .compare_exchange(
                previous,
                HostStatus::Maintenance as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| HostError::Busy)?;
        let this = self.clone();
        // No suspension between closing admission and launching the retained
        // worker. It waits for earlier dispatch before consuming Application.
        let worker = tokio::spawn(async move {
            let opening = matches!(action, Action::Open);
            let mut slot = this.0.application.clone().lock_owned().await;
            let result = this.run_inner(action, &mut slot).await;
            // Publish exactly once while still owning the gate. An older
            // worker must never reset the phase of a newly admitted action.
            if opening && result.is_ok() {
                this.0
                    .status
                    .store(HostStatus::Ready as u8, Ordering::Release);
            } else if opening {
                this.0
                    .status
                    .store(HostStatus::RecoveryRequired as u8, Ordering::Release);
            } else {
                this.set_offline_status();
            }
            result
        });
        match worker.await {
            Ok(result) => result,
            Err(_) => {
                self.0
                    .status
                    .store(HostStatus::RecoveryRequired as u8, Ordering::Release);
                Err(HostError::Storage)
            }
        }
    }
    async fn run_inner(
        &self,
        action: Action,
        slot: &mut Option<Application>,
    ) -> Result<Value, HostError> {
        if matches!(action, Action::Open) {
            if slot.is_some() {
                return Ok(Value::Null);
            }
            if self.blocked() {
                return Err(HostError::Unavailable);
            }
            // A previous close may have timed out while a detached worker
            // still retained a pool clone. Reopen must drain it too.
            let owner =
                crate::app_ownership::drained_exclusive(&self.0.root.join("foundation.sqlite3"))
                    .await
                    .map_err(|_| HostError::Busy)?;
            drop(owner);
            let store = Store::open(&self.0.root.join("foundation.sqlite3"))
                .await
                .map_err(|_| HostError::Storage)?;
            let pin: Option<String> = sqlx::query_scalar(
                "SELECT registry_binding FROM community_host_binding WHERE id=1",
            )
            .fetch_optional(&store.pool)
            .await
            .map_err(|_| HostError::Storage)?;
            if pin
                .as_deref()
                .is_some_and(|p| p != self.0.service.binding())
            {
                store.pool.close().await;
                return Err(HostError::Unavailable);
            }
            let app = match &self.0.picker {
                Some(picker) => {
                    Application::with_file_picker(store, self.0.vault.clone(), picker.clone())
                }
                None => Application::with_vault(store, self.0.vault.clone()),
            }
            .with_community_lifecycle(self.0.service.clone());
            *slot = Some(app);
            return Ok(Value::Null);
        }
        async {
            if let Some(app) = slot.take() {
                tokio::time::timeout(Duration::from_secs(5), app.close_for_maintenance())
                    .await
                    .map_err(|_| HostError::Busy)?;
            }
            // Detached local workers may still hold closed pool clones. Wait
            // for actual ownership drain, never unlink or expire their lock.
            let owner =
                crate::app_ownership::drained_exclusive(&self.0.root.join("foundation.sqlite3"))
                    .await
                    .map_err(|_| HostError::Busy)?;
            drop(owner); // Core reacquires and holds both owners for all I/O.
            if !matches!(action, Action::Close)
                && crate::backup::host_restore_pending(&self.0.root).unwrap_or(true)
            {
                return Err(HostError::Unavailable);
            }
            match action {
                Action::Close => Ok(Value::Null),
                Action::Create(path, id) => self
                    .0
                    .backup
                    .create(&path, &id)
                    .await
                    .map_err(HostError::Backup),
                Action::Restore(path, id, policy) => self
                    .0
                    .backup
                    .restore(&path, &id, policy)
                    .await
                    .map_err(HostError::Backup),
                Action::Cleanup(id) => self
                    .0
                    .backup
                    .finish_cleanup(&id)
                    .await
                    .map_err(HostError::Backup),
                Action::Open => unreachable!(),
            }
        }
        .await
    }
}
