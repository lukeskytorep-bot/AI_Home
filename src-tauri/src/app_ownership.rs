//! Cooperating Store pools hold shared ownership; offline maintenance is exclusive.
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
};
pub(crate) const PENDING: &str = "community-restore.pending.json";
pub(crate) struct Owner {
    _file: File,
}
pub(crate) fn pending(root: &Path) -> bool {
    std::fs::symlink_metadata(root.join(PENDING)).is_ok()
}
pub(crate) fn acquire(path: &Path, exclusive: bool) -> Result<Arc<Owner>, sqlx::Error> {
    let error = || sqlx::Error::Protocol("application storage is owned or unavailable".into());
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = std::fs::canonicalize(parent).map_err(|_| error())?;
    let name = path.file_name().ok_or_else(error)?;
    let db = parent.join(name);
    for p in [&db, &lock_path(&parent, name)] {
        if std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(error());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if p.metadata().is_ok_and(|m| m.nlink() != 1) {
                return Err(error());
            }
        }
    }
    let mut opts = OpenOptions::new();
    opts.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let file = opts.open(lock_path(&parent, name)).map_err(|_| error())?;
    if exclusive {
        fs2::FileExt::try_lock_exclusive(&file)
    } else {
        fs2::FileExt::try_lock_shared(&file)
    }
    .map_err(sqlx::Error::Io)?;
    Ok(Arc::new(Owner { _file: file }))
}
/// Pool disposal can finish on a scheduled SQLx task after the last wrapper
/// drops. Startup may wait for that drain, but never expire a live owner.
pub(crate) async fn drained_exclusive(path: &Path) -> Result<Arc<Owner>, sqlx::Error> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match acquire(path, true) {
                Ok(owner) => return Ok(owner),
                Err(sqlx::Error::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
                {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await
                }
                Err(e) => return Err(e),
            }
        }
    })
    .await
    .map_err(|_| sqlx::Error::Protocol("application storage is still owned".into()))?
}
fn lock_path(parent: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let mut n = name.to_os_string();
    n.push(".app-owner-lock");
    parent.join(n)
}
