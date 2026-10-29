//! Test-only physical storage inspection without reading live Windows locks.
use super::registry::RegistrationRegistry;
use std::path::{Path, PathBuf};

pub(super) async fn bytes_after_close(
    registry: RegistrationRegistry,
    directory: &Path,
) -> Vec<(PathBuf, Vec<u8>)> {
    // Freeze registry writers. Preserve the exact live WAL before a clean close
    // can checkpoint/delete it, so transient plaintext remains covered too.
    let lease = registry.maintenance().await;
    let mut wal = registry.storage_path().as_os_str().to_os_string();
    wal.push("-wal");
    let wal = PathBuf::from(wal);
    let mut files = Vec::new();
    match std::fs::read(&wal) {
        Ok(bytes) => files.push((wal, bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("live WAL inspection failed: {e}"),
    }
    // Release actual SQLite and owner handles naturally. No forced unlock,
    // manual removal of WAL/SHM, skipped files or swallowed read errors.
    registry.close().await;
    drop(lease);
    for entry in std::fs::read_dir(directory).expect("storage directory") {
        let path = entry.expect("storage entry").path();
        let bytes = std::fs::read(&path).expect("closed storage read");
        files.push((path, bytes));
    }
    files
}

#[tokio::test]
async fn inspection_keeps_live_wal_bytes_and_reads_every_file_after_close() {
    use super::{CommunityScope, RegistrationProfile};
    let root = std::env::temp_dir().join(format!("ai-home-storage-scan-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("registry.sqlite");
    let registry = RegistrationRegistry::open(&path).await.unwrap();
    let scope = CommunityScope::new(
        "scan",
        "https://example.test/api/v1/",
        "https://example.test",
        "scan-test",
    )
    .unwrap();
    registry
        .prepare(
            &scope,
            &RegistrationProfile::new("WAL-SCAN-SENTINEL", "public").unwrap(),
            "scan-client-start-0123456789",
        )
        .await
        .unwrap();
    let wal = root.join("registry.sqlite-wal");
    let before = std::fs::read(&wal).unwrap();
    assert!(String::from_utf8_lossy(&before).contains("WAL-SCAN-SENTINEL"));
    std::fs::write(root.join("extra.txt"), b"EXTRA-SCAN-SENTINEL").unwrap();
    let files = bytes_after_close(registry, &root).await;
    assert!(files.iter().any(|(p, b)| p == &wal && b == &before));
    assert!(files
        .iter()
        .any(|(p, b)| p == &root.join("extra.txt") && b == b"EXTRA-SCAN-SENTINEL"));
    assert!(files.iter().any(|(p, _)| p == &path));
    assert!(files
        .iter()
        .any(|(p, _)| p == &root.join("registry.sqlite.owner-lock")));
    assert!(std::fs::read_dir(&root).unwrap().all(|e| {
        let path = e.unwrap().path();
        files.iter().any(|(p, _)| p == &path)
    }));
    std::fs::remove_dir_all(root).unwrap();
}
