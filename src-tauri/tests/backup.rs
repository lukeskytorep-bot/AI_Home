// Source-normalized backup/restore regression tests. Keep literal escape corruption out of Rust source.
use ai_home::{application::Application,backup,persistence::Store};
use serde_json::{json,Value};
use std::path::PathBuf;
fn req(id:&str,command:&str,payload:Value,expected:Option<i64>)->Value{let mut v=json!({"contractVersion":1,"requestId":id,"command":command,"payload":payload});if let Some(e)=expected{v["expectedRevision"]=json!(e);}v}
fn dir()->PathBuf{let p=std::env::temp_dir().join(format!("ai-home-backup-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&p).unwrap();p}
async fn app_test_pool(path:&std::path::Path)->sqlx::SqlitePool{use sqlx::sqlite::{SqliteConnectOptions,SqlitePoolOptions};SqlitePoolOptions::new().max_connections(1).connect_with(SqliteConnectOptions::new().filename(path).read_only(true)).await.unwrap()}

#[tokio::test]
async fn backup_change_restore_uses_safety_copy_and_restart(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);
 let b=app.dispatch(req("b1","backup.create",json!({}),None)).await.response;assert_eq!(b["ok"],true);let bid=b["result"]["id"].as_str().unwrap().to_string();
 let p=app.dispatch(req("p1","app.preferences.set",json!({"locale":"pl","themeId":"orbital","themeVariant":"system"}),Some(1))).await.response;assert_eq!(p["ok"],true);
 let pre=app.dispatch(req("pre","backup.restore.preflight",json!({"backupId":bid}),None)).await.response;assert_eq!(pre["ok"],true);assert_eq!(pre["result"]["safetyCopyRequired"],true);
 let rr=app.dispatch(req("r1","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(rr["ok"],true);let safety=rr["result"]["safetyBackupId"].as_str().unwrap().to_string();
 let blocked=app.dispatch(req("after","app.preferences.set",json!({"locale":"en","themeId":"orbital","themeVariant":"system"}),Some(2))).await.response;assert_eq!(blocked["ok"],false);assert_eq!(blocked["error"]["code"],"RESTORE_RESTART_REQUIRED");drop(app);
 backup::apply_pending_restore(&d).await.unwrap();let restored=Store::open(&db).await.unwrap();assert_eq!(restored.preferences().await.unwrap()["locale"],"en");drop(restored);
 let safety_db=d.join("backups").join(format!("{safety}.sqlite3"));let safety_store=Store::open(&safety_db).await.unwrap();assert_eq!(safety_store.preferences().await.unwrap()["locale"],"pl");drop(safety_store);let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn preflight_is_read_only_and_corrupt_backup_is_rejected(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let b=app.dispatch(req("b2","backup.create",json!({}),None)).await.response;let bid=b["result"]["id"].as_str().unwrap();let before:i64=sqlx::query_scalar("SELECT count(*) FROM app_events").fetch_one(&app_test_pool(&db).await).await.unwrap();let pre=app.dispatch(req("pre2","backup.restore.preflight",json!({"backupId":bid}),None)).await.response;assert_eq!(pre["ok"],true);let after:i64=sqlx::query_scalar("SELECT count(*) FROM app_events").fetch_one(&app_test_pool(&db).await).await.unwrap();assert_eq!(before,after);let path=d.join("backups").join(format!("{bid}.sqlite3"));std::fs::write(path,b"not sqlite").unwrap();let bad=app.dispatch(req("bad","backup.restore.preflight",json!({"backupId":bid}),None)).await.response;assert_eq!(bad["ok"],false);drop(app);let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn invalid_pending_restore_fails_closed_and_keeps_current_database(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let p=app.dispatch(req("p2","app.preferences.set",json!({"locale":"pl","themeId":"orbital","themeVariant":"system"}),Some(1))).await.response;assert_eq!(p["ok"],true);let b=app.dispatch(req("b3","backup.create",json!({}),None)).await.response;let bid=b["result"]["id"].as_str().unwrap().to_string();let rr=app.dispatch(req("r3","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(rr["ok"],true);drop(app);std::fs::write(d.join("backups").join(format!("{bid}.sqlite3")),b"corrupt").unwrap();assert!(backup::apply_pending_restore(&d).await.is_err());let current=Store::open(&db).await.unwrap();assert_eq!(current.preferences().await.unwrap()["locale"],"pl");drop(current);let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn completed_restore_request_is_idempotent_across_restart(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let b=app.dispatch(req("b4","backup.create",json!({}),None)).await.response;let bid=b["result"]["id"].as_str().unwrap().to_string();let first=app.dispatch(req("same-restore","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(first["ok"],true);let safety=first["result"]["safetyBackupId"].clone();drop(app);backup::apply_pending_restore(&d).await.unwrap();let store=Store::open(&db).await.unwrap();let app=Application::new(store);let replay=app.dispatch(req("same-restore","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(replay["ok"],true);assert_eq!(replay["result"]["safetyBackupId"],safety);let status=app.dispatch(req("st","backup.restore.status",json!({}),None)).await.response;assert!(status["result"]["pending"].is_null());drop(app);let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn missing_current_is_recovered_from_safety_before_restore(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let original=app.dispatch(req("b5","backup.create",json!({}),None)).await.response;let bid=original["result"]["id"].as_str().unwrap().to_string();let _=app.dispatch(req("p5","app.preferences.set",json!({"locale":"pl","themeId":"orbital","themeVariant":"system"}),Some(1))).await.response;let rr=app.dispatch(req("r5","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(rr["ok"],true);drop(app);std::fs::remove_file(&db).unwrap();let result=backup::apply_pending_restore(&d).await.unwrap().unwrap();assert_eq!(result["applied"],true);let restored=Store::open(&db).await.unwrap();assert_eq!(restored.preferences().await.unwrap()["locale"],"en");drop(restored);let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn crash_after_current_move_recovers_from_safety_and_finishes_restore(){
 use sha2::{Digest,Sha256};
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let b=app.dispatch(req("b6","backup.create",json!({}),None)).await.response;let bid=b["result"]["id"].as_str().unwrap().to_string();let _=app.dispatch(req("p6","app.preferences.set",json!({"locale":"pl","themeId":"orbital","themeVariant":"system"}),Some(1))).await.response;let rr=app.dispatch(req("crash-r","backup.restore.request",json!({"backupId":bid}),None)).await.response;assert_eq!(rr["ok"],true);drop(app);
 let key=format!("{:x}",Sha256::digest(b"ai-home:restore-request:crash-r"));let previous=d.join(format!("foundation.restore.previous.{key}.sqlite3"));std::fs::rename(&db,&previous).unwrap();assert!(!db.exists());
 let result=backup::apply_pending_restore(&d).await.unwrap().unwrap();assert_eq!(result["applied"],true);let restored=Store::open(&db).await.unwrap();assert_eq!(restored.preferences().await.unwrap()["locale"],"en");drop(restored);assert!(previous.exists());let _=std::fs::remove_dir_all(d);
}

#[tokio::test]
async fn replay_same_request_with_different_backup_is_conflict(){
 let d=dir();let db=d.join("foundation.sqlite3");let store=Store::open(&db).await.unwrap();let app=Application::new(store);let b1=app.dispatch(req("ba","backup.create",json!({}),None)).await.response;let b2=app.dispatch(req("bb","backup.create",json!({}),None)).await.response;let id1=b1["result"]["id"].as_str().unwrap();let id2=b2["result"]["id"].as_str().unwrap();let first=app.dispatch(req("restore-conflict","backup.restore.request",json!({"backupId":id1}),None)).await.response;assert_eq!(first["ok"],true);let second=app.dispatch(req("restore-conflict","backup.restore.request",json!({"backupId":id2}),None)).await.response;assert_eq!(second["ok"],false);assert_eq!(second["error"]["code"],"IDEMPOTENCY_CONFLICT");drop(app);let _=std::fs::remove_dir_all(d);
}
