//! One owner process per stable registry path; blocking writers retain leases.
use super::*;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    sync::{Arc, Mutex, OnceLock, Weak},
};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
static OWNERS: OnceLock<Mutex<HashMap<std::path::PathBuf, Weak<Gate>>>> = OnceLock::new();
pub(crate) struct Gate {
    _file: File,
    path: std::path::PathBuf,
    lock: Arc<RwLock<()>>,
}
enum Hold {
    Read { _guard: OwnedRwLockReadGuard<()> },
    Write { _guard: OwnedRwLockWriteGuard<()> },
}
struct Inner {
    owner: Arc<Gate>,
    _hold: Hold,
    profile: Option<String>,
}
#[derive(Clone)]
pub(crate) struct Lease(Arc<Inner>);
impl Lease {
    fn permits(&self, owner: &Arc<Gate>, profile: &str) -> bool {
        Arc::ptr_eq(&self.0.owner, owner) && self.0.profile.as_deref().is_none_or(|p| p == profile)
    }
}
impl Gate {
    pub(super) async fn open(path: &Path) -> Result<Arc<Self>, RegistryError> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            if !path.is_absolute() || path.to_str().is_none() {
                return Err(RegistryError::Corrupt);
            }
            if path.exists() && !path.is_file() {
                return Err(RegistryError::Corrupt);
            }
            if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(RegistryError::Corrupt);
            }
            let path = if path.exists() {
                std::fs::canonicalize(&path).map_err(|_| RegistryError::Storage)?
            } else {
                std::fs::canonicalize(path.parent().ok_or(RegistryError::Storage)?)
                    .map_err(|_| RegistryError::Storage)?
                    .join(path.file_name().ok_or(RegistryError::Storage)?)
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if path.metadata().is_ok_and(|m| m.nlink() != 1) {
                    return Err(RegistryError::Corrupt);
                }
            }
            let mut owners = OWNERS
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| RegistryError::Storage)?;
            if let Some(owner) = owners.get(&path).and_then(Weak::upgrade) {
                return Ok(owner);
            }
            let lockpath = path.with_file_name(format!(
                "{}.owner-lock",
                path.file_name().unwrap().to_string_lossy()
            ));
            if std::fs::symlink_metadata(&lockpath).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(RegistryError::Corrupt);
            }
            let mut options = OpenOptions::new();
            options.create(true).read(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options.open(lockpath).map_err(|_| RegistryError::Storage)?;
            fs2::FileExt::try_lock_exclusive(&file).map_err(|_| RegistryError::Busy)?;
            if !path.exists() {
                let mut create = OpenOptions::new();
                create.create_new(true).read(true).write(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    create.mode(0o600);
                }
                create.open(&path).map_err(|_| RegistryError::Storage)?;
            }
            owners.retain(|_, w| w.strong_count() > 0);
            let owner = Arc::new(Self {
                _file: file,
                path: path.clone(),
                lock: Arc::new(RwLock::new(())),
            });
            owners.insert(path, Arc::downgrade(&owner));
            Ok(owner)
        })
        .await
        .map_err(|_| RegistryError::Storage)?
    }
    pub(super) async fn exclusive(self: &Arc<Self>) -> Lease {
        Lease(Arc::new(Inner {
            owner: self.clone(),
            _hold: Hold::Write {
                _guard: self.lock.clone().write_owned().await,
            },
            profile: None,
        }))
    }
}
impl RegistrationRegistry {
    /// Native host pin, not an authentication credential or a portable identity.
    pub(crate) fn host_binding(&self) -> String {
        let mut bytes=b"ai-home-community-registry-path/1\n".to_vec();
        bytes.extend_from_slice(self.gate.path.as_os_str().as_encoded_bytes());
        super::super::crypto::digest(&bytes)
    }
    pub(crate) async fn lease(
        &self,
        scope: &CommunityScope,
        existing: Option<&Lease>,
    ) -> Result<Lease, RegistryError> {
        self.lease_checked(scope, existing, false).await
    }
    pub(super) async fn prepare_lease(
        &self,
        scope: &CommunityScope,
    ) -> Result<Lease, RegistryError> {
        self.lease_checked(scope, None, true).await
    }
    async fn lease_checked(
        &self,
        scope: &CommunityScope,
        existing: Option<&Lease>,
        create: bool,
    ) -> Result<Lease, RegistryError> {
        if let Some(lease) = existing {
            if lease.permits(&self.gate, &scope.profile_id) {
                return Ok(lease.clone());
            }
            return Err(RegistryError::PolicyDenied);
        }
        let hold = self.gate.lock.clone().read_owned().await;
        if create {
            sqlx::query("INSERT INTO community_profile_lifecycle(profile_id,state,revision) VALUES(?,'active',1) ON CONFLICT DO NOTHING").bind(&scope.profile_id).execute(&self.pool).await?;
        }
        let state: String =
            sqlx::query_scalar("SELECT state FROM community_profile_lifecycle WHERE profile_id=?")
                .bind(&scope.profile_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(RegistryError::NotFound)?;
        if state != "active" {
            return Err(RegistryError::PolicyDenied);
        }
        Ok(Lease(Arc::new(Inner {
            owner: self.gate.clone(),
            _hold: Hold::Read { _guard: hold },
            profile: Some(scope.profile_id.clone()),
        })))
    }
}
