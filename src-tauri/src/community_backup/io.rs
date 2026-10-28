use super::*;
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
};
pub(super) fn hash(path: &Path) -> Result<String, BackupError> {
    regular(path)?;
    let mut f = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub(super) fn regular(path: &Path) -> Result<(), BackupError> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.file_type().is_symlink() {
        return Err(BackupError::InvalidBundle);
    }
    Ok(())
}
pub(super) fn canonical_dir(path: &Path) -> Result<PathBuf, BackupError> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || fs::symlink_metadata(path)?.file_type().is_symlink()
    {
        return Err(BackupError::InvalidBundle);
    }
    let p = fs::canonicalize(path)?;
    if !p.is_dir() || p.to_str().is_none() {
        return Err(BackupError::InvalidBundle);
    }
    Ok(p)
}
pub(super) fn private_dir(path: &Path) -> Result<(), BackupError> {
    let mut d = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        d.mode(0o700);
    }
    d.create(path)?;
    sync_parent(path)
}
pub(super) fn ensure_dir(path: &Path) -> Result<(), BackupError> {
    if path.exists() {
        canonical_dir(path).map(|_| ())
    } else {
        private_dir(path)
    }
}
pub(super) fn private_file(path: &Path) -> Result<fs::File, BackupError> {
    let mut o = OpenOptions::new();
    o.create_new(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    Ok(o.open(path)?)
}
pub(super) fn read_json(path: &Path) -> Result<Value, BackupError> {
    regular(path)?;
    if path.metadata()?.len() > 65536 {
        return Err(BackupError::InvalidBundle);
    }
    serde_json::from_slice(&fs::read(path)?).map_err(|_| BackupError::InvalidBundle)
}
pub(super) fn write_once(path: &Path, v: &Value) -> Result<(), BackupError> {
    if path.exists() {
        if read_json(path)? == *v {
            return Ok(());
        }
        return Err(BackupError::IdempotencyConflict);
    }
    if v.to_string().len() > 65536 {
        return Err(BackupError::InvalidBundle);
    }
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let mut f = private_file(&tmp)?;
    f.write_all(v.to_string().as_bytes())?;
    f.sync_all()?;
    drop(f);
    // All publishers for this installation hold the exclusive app owner.
    if path.exists() {
        let _ = fs::remove_file(&tmp);
        return Err(BackupError::IdempotencyConflict);
    }
    fs::rename(&tmp, path)?;
    sync_parent(path)
}
pub(super) fn sync_file(path: &Path) -> Result<(), BackupError> {
    regular(path)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}
pub(super) fn sync_parent(path: &Path) -> Result<(), BackupError> {
    #[cfg(unix)]
    {
        fs::File::open(path.parent().ok_or(BackupError::Io)?)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
pub(super) fn quoted(path: &Path) -> Result<String, BackupError> {
    Ok(path.to_str().ok_or(BackupError::Io)?.replace('\'', "''"))
}
pub(super) fn key(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
pub(super) fn request(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
}
pub(super) fn is_hash(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub(super) fn path_binding(p: &Path) -> String {
    let mut bytes = b"ai-home-offline-root/1\n".to_vec();
    bytes.extend_from_slice(p.as_os_str().as_encoded_bytes());
    format!("{:x}", Sha256::digest(&bytes))
}
pub(super) async fn connection(
    path: &Path,
    readonly: bool,
    immutable: bool,
) -> Result<sqlx::SqliteConnection, BackupError> {
    Ok(sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .read_only(readonly)
            .immutable(immutable)
            .foreign_keys(true),
    )
    .await?)
}
type Shape = Vec<(String, String, String, Option<String>)>;
async fn shape(c: &mut sqlx::SqliteConnection) -> Result<Shape, BackupError> {
    Ok(
        sqlx::query_as("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name")
            .fetch_all(c)
            .await?,
    )
}
pub(super) struct MainInfo {
    pub ids: BTreeSet<String>,
    pub pin: Option<String>,
    pub active_experimental: bool,
}
pub(super) async fn main_info(path: &Path, immutable: bool) -> Result<MainInfo, BackupError> {
    regular(path)?;
    if path.metadata()?.len() > 1024 * 1024 * 1024 {
        return Err(BackupError::InvalidBundle);
    }
    let mut c = connection(path, true, immutable).await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut c)
        .await?;
    if version != crate::persistence::CURRENT_SCHEMA {
        return Err(BackupError::InvalidBundle);
    }
    // Only the canonical schema for this native bundle version is accepted.
    // Compare before evaluating source CHECKs/triggers or opening it for writes.
    let trusted = crate::persistence::Store::memory().await?;
    let expected: Shape =
        sqlx::query_as("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name")
            .fetch_all(&trusted.pool)
            .await?;
    trusted.pool.close().await;
    if shape(&mut c).await? != expected {
        return Err(BackupError::InvalidBundle);
    }
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut c)
        .await?;
    if integrity != "ok"
        || !sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut c)
            .await?
            .is_empty()
    {
        return Err(BackupError::InvalidBundle);
    }
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM profile_deletion_jobs")
        .fetch_one(&mut c)
        .await?;
    if pending != 0 {
        return Err(BackupError::PolicyDenied);
    }
    let pin: Option<String> =
        sqlx::query_scalar("SELECT registry_binding FROM community_host_binding WHERE id=1")
            .fetch_optional(&mut c)
            .await?;
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM profiles ORDER BY id")
        .fetch_all(&mut c)
        .await?;
    let active:i64=sqlx::query_scalar("SELECT count(*) FROM experimental_instances WHERE json_extract(body,'$.status') IS NOT 'inactive'").fetch_one(&mut c).await?;
    c.close().await?;
    Ok(MainInfo {
        ids: ids.into_iter().collect(),
        pin,
        active_experimental: active != 0,
    })
}
pub(super) async fn vacuum(source: &Path, dest: &Path) -> Result<(), BackupError> {
    let f = private_file(dest)?;
    drop(f);
    let mut c = connection(source, true, false).await?;
    sqlx::query(&format!("VACUUM INTO '{}'", quoted(dest)?))
        .execute(&mut c)
        .await?;
    c.close().await?;
    sync_file(dest)?;
    sync_parent(dest)
}
pub(super) async fn checkpoint(path: &Path) -> Result<(), BackupError> {
    let mut c = connection(path, false, false).await?;
    let rows = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_all(&mut c)
        .await?;
    if rows.iter().any(|r| r.get::<i64, _>(0) != 0) {
        return Err(BackupError::Busy);
    }
    c.close().await?;
    sync_file(path)
}
pub(super) fn copy_checked(
    source: &Path,
    target: &Path,
    expected: &str,
) -> Result<(), BackupError> {
    if target.exists() {
        if hash(target)? == expected {
            return Ok(());
        }
        return Err(BackupError::InvalidBundle);
    }
    let tmp = target.with_file_name(format!(".input-{}.tmp", uuid::Uuid::new_v4()));
    let mut input = std::fs::File::open(source)?;
    let mut output = private_file(&tmp)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    drop(output);
    if hash(&tmp)? != expected {
        let _ = std::fs::remove_file(&tmp);
        return Err(BackupError::InvalidBundle);
    }
    std::fs::rename(&tmp, target)?;
    sync_parent(target)
}
