use super::*;
use serde_json::json;
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("ai-home-desktop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn policy() -> Value {
    json!({"schemaVersion":1,"targets":[{"profileId":"profile-a","apiRoot":"https://example.test/sub/api/v1/","communityOrigin":"https://example.test","communityInstance":"test-community","commands":["community.registration.status","community.registration.progress"]}]})
}
fn write(t: &Temp, v: &Value) {
    std::fs::write(t.0.join(POLICY), v.to_string()).unwrap();
}
async fn main_db(t: &Temp) {
    let s = Store::open(&t.0.join("foundation.sqlite3")).await.unwrap();
    Application::new(s).close_for_maintenance().await;
}
async fn close(runtime: DesktopRuntime) {
    match runtime {
        DesktopRuntime::Standard(app) => app.close_for_maintenance().await,
        DesktopRuntime::Community { host, .. } => host.close_application().await.unwrap(),
    }
}
#[test]
fn policy_exact_shape_bounded_and_known_grants_only() {
    assert!(DesktopPolicy::parse(policy().to_string().as_bytes()).is_ok());
    let variants: Vec<Value> = vec![
        json!(null),
        json!({}),
        json!({"schemaVersion":2,"targets":[]}),
        json!({"schemaVersion":1,"targets":[]}),
    ];
    for v in variants {
        assert!(DesktopPolicy::parse(v.to_string().as_bytes()).is_err());
    }
    for (key, value) in [
        ("apiRoot", json!("http://example.test/api/v1/")),
        ("apiRoot", json!("https://foreign.test/api/v1/")),
        ("communityOrigin", json!("https://example.test/path")),
        (
            "commands",
            json!([
                "community.registration.progress",
                "community.registration.progress"
            ]),
        ),
        ("commands", json!(["profile.delete.permanent"])),
        ("commands", json!([])),
    ] {
        let mut v = policy();
        v["targets"][0][key] = value;
        assert!(
            DesktopPolicy::parse(v.to_string().as_bytes()).is_err(),
            "{v}"
        );
    }
    let mut duplicate = policy();
    duplicate["targets"]
        .as_array_mut()
        .unwrap()
        .push(policy()["targets"][0].clone());
    assert!(DesktopPolicy::parse(duplicate.to_string().as_bytes()).is_err());
    let mut extra = policy();
    extra["invite"] = json!("must-not-be-policy");
    assert!(DesktopPolicy::parse(extra.to_string().as_bytes()).is_err());
    let mut extra = policy();
    extra["targets"][0]["capabilities"] = json!(["all"]);
    assert!(DesktopPolicy::parse(extra.to_string().as_bytes()).is_err());
    assert!(DesktopPolicy::parse(&vec![b' '; 65537]).is_err());
}
#[test]
fn webview_caller_is_exact_and_dev_origin_is_not_allowed_in_release() {
    for u in ["tauri://localhost/", "http://tauri.localhost/index.html"] {
        assert!(caller("main", u, false));
    }
    for u in [
        "https://example.test/",
        "http://tauri.localhost.evil/",
        "tauri://foreign/",
        "http://127.0.0.1:1420/",
        "http://localhost:1420/",
        "http://tauri.localhost:9000/",
        "http://x@tauri.localhost/",
    ] {
        assert!(!caller("main", u, false), "{u}");
    }
    assert!(!caller("other", "tauri://localhost/", true));
    assert!(caller("main", "http://127.0.0.1:1420/", true));
    assert!(!caller("main", "http://127.0.0.1:1421/", true));
}
#[tokio::test]
async fn default_startup_keeps_standard_and_community_entry_disabled() {
    let t = Temp::new();
    let r = DesktopRuntime::open(&t.0, Vault::os(), None).await.unwrap();
    assert!(matches!(&r, DesktopRuntime::Standard(_)));
    let p = json!({"command":"community.registration.progress","profileId":"profile-a"});
    assert_eq!(
        r.registration("main", "tauri://localhost/", p, None).await,
        Err("COMMUNITY_NOT_ENABLED")
    );
    assert!(!t.0.join("community.sqlite3").exists());
    let response=r.dispatch(json!({"contractVersion":1,"requestId":"preferences","command":"app.preferences.get","payload":{}})).await.unwrap().response;
    assert_eq!(response["ok"], true, "{response}");
    close(r).await;
}
#[tokio::test]
async fn invalid_opt_in_never_falls_back_or_creates_main() {
    let t = Temp::new();
    std::fs::write(t.0.join(POLICY), b"{}").unwrap();
    assert_eq!(
        DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
        Some("COMMUNITY_POLICY_INVALID")
    );
    assert!(!t.0.join("foundation.sqlite3").exists());
    write(&t, &policy());
    assert_eq!(
        DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
        Some("COMMUNITY_RECOVERY_REQUIRED")
    );
    assert!(!t.0.join("foundation.sqlite3").exists());
    assert!(!t.0.join("community.sqlite3").exists());
}
#[tokio::test]
async fn opt_in_owns_one_host_without_keys_network_or_legacy_backup() {
    let t = Temp::new();
    main_db(&t).await;
    write(&t, &policy());
    let r = DesktopRuntime::open(&t.0, Vault::os(), None).await.unwrap();
    assert!(matches!(&r, DesktopRuntime::Community { .. }));
    let response=r.dispatch(json!({"contractVersion":1,"requestId":"ordinary","command":"app.preferences.get","payload":{}})).await.unwrap().response;
    assert_eq!(response["ok"], true);
    let DesktopRuntime::Community { host, .. } = &r else {
        unreachable!()
    };
    assert_eq!(host.status(), super::super::maintenance::HostStatus::Ready);
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(t.0.join("community.sqlite3")),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM community_preparations")
        .fetch_one(&mut c)
        .await
        .unwrap();
    assert_eq!(count, 0);
    c.close().await.unwrap();
    close(r).await;
    std::fs::remove_file(t.0.join(POLICY)).unwrap();
    assert_eq!(
        DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
        Some("COMMUNITY_POLICY_REQUIRED")
    );
}
#[tokio::test]
async fn restore_markers_and_missing_pinned_registry_refuse_before_creation() {
    let t = Temp::new();
    main_db(&t).await;
    write(&t, &policy());
    for marker in [
        "community-restore.pending.json",
        "community.sqlite3.restore-fence.json",
    ] {
        std::fs::write(t.0.join(marker), b"pending").unwrap();
        assert_eq!(
            DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
            Some("COMMUNITY_RECOVERY_REQUIRED")
        );
        assert!(!t.0.join("community.sqlite3").exists());
        std::fs::remove_file(t.0.join(marker)).unwrap();
    }
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(t.0.join("foundation.sqlite3")),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO community_host_binding VALUES(1,?)")
        .bind("a".repeat(64))
        .execute(&mut c)
        .await
        .unwrap();
    c.close().await.unwrap();
    assert_eq!(
        DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
        Some("COMMUNITY_RECOVERY_REQUIRED")
    );
    assert!(!t.0.join("community.sqlite3").exists());
    std::fs::remove_file(t.0.join(POLICY)).unwrap();
    assert_eq!(
        DesktopRuntime::open(&t.0, Vault::os(), None).await.err(),
        Some("COMMUNITY_POLICY_REQUIRED")
    );
}
#[cfg(unix)]
#[test]
fn policy_loader_refuses_symlink_and_other_writable_config() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let t = Temp::new();
    let target = t.0.join("other.json");
    std::fs::write(&target, policy().to_string()).unwrap();
    symlink(&target, t.0.join(POLICY)).unwrap();
    assert!(DesktopPolicy::load(&t.0).is_err());
    std::fs::remove_file(t.0.join(POLICY)).unwrap();
    write(&t, &policy());
    std::fs::set_permissions(t.0.join(POLICY), std::fs::Permissions::from_mode(0o666)).unwrap();
    assert!(DesktopPolicy::load(&t.0).is_err());
}
