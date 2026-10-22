use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::{Connection,Row,SqliteConnection};
use std::{fs,io::{Read,Write},path::{Path,PathBuf}};
use crate::persistence::Store;

const CURRENT_SCHEMA:i64=12;
const MANIFEST_VERSION:i64=1;

fn io_error(message:&str)->sqlx::Error{sqlx::Error::Protocol(message.into())}
fn root(store:&Store)->Result<PathBuf,sqlx::Error>{store.database_path().and_then(Path::parent).map(Path::to_path_buf).ok_or_else(||io_error("backup unavailable for in-memory storage"))}
fn backup_dir(root:&Path)->PathBuf{root.join("backups")}
fn pending_path(root:&Path)->PathBuf{root.join("restore-pending.json")}
fn result_path(root:&Path)->PathBuf{root.join("restore-last.json")}
fn safe_id(s:&str)->bool{!s.is_empty()&&s.len()<=80&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
fn deterministic_id(kind:&str,request_id:&str)->String{let h=format!("{:x}",Sha256::digest(format!("ai-home:{kind}:{request_id}").as_bytes()));format!("{kind}-{}",&h[..32])}
fn quote_sqlite_path(path:&Path)->String{path.to_string_lossy().replace('\\',"/").replace('\'',"''")}
fn file_hash(path:&Path)->Result<String,sqlx::Error>{let mut f=fs::File::open(path).map_err(|_|io_error("backup file open failed"))?;let mut h=Sha256::new();let mut b=[0u8;65536];loop{let n=f.read(&mut b).map_err(|_|io_error("backup file read failed"))?;if n==0{break}h.update(&b[..n]);}Ok(format!("{:x}",h.finalize()))}
fn write_json_atomic(path:&Path,value:&Value)->Result<(),sqlx::Error>{let tmp=path.with_extension("json.tmp");let mut f=fs::File::create(&tmp).map_err(|_|io_error("backup metadata create failed"))?;f.write_all(value.to_string().as_bytes()).map_err(|_|io_error("backup metadata write failed"))?;f.sync_all().map_err(|_|io_error("backup metadata sync failed"))?;{if path.exists(){fs::remove_file(path).map_err(|_|io_error("backup metadata old remove failed"))?;}fs::rename(tmp,path).map_err(|_|io_error("backup metadata replace failed"))}}
fn read_json(path:&Path)->Result<Value,sqlx::Error>{serde_json::from_slice(&fs::read(path).map_err(|_|io_error("backup metadata read failed"))?).map_err(|_|io_error("backup metadata invalid"))}

async fn inspect(path:&Path)->Result<Value,sqlx::Error>{
    if !path.is_file(){return Err(io_error("backup missing"))}
    let options=sqlx::sqlite::SqliteConnectOptions::new().filename(path).read_only(true).create_if_missing(false);
    let mut c=SqliteConnection::connect_with(&options).await?;
    let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut c).await?;
    if !(1..=CURRENT_SCHEMA).contains(&version){return Err(io_error("unsupported backup schema"))}
    let integrity:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut c).await?;
    if integrity!="ok"{return Err(io_error("backup integrity check failed"))}
    let fk=sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut c).await?.len() as i64;
    if fk!=0{return Err(io_error("backup foreign key check failed"))}
    let profiles:i64=sqlx::query_scalar("SELECT count(*) FROM profiles").fetch_one(&mut c).await.unwrap_or(0);
    let conversations:i64=sqlx::query_scalar("SELECT count(*) FROM conversations").fetch_one(&mut c).await.unwrap_or(0);
    c.close().await?;
    Ok(json!({"schemaVersion":version,"integrity":"ok","foreignKeyViolations":0,"profileCount":profiles,"conversationCount":conversations}))
}

fn manifest_path(dir:&Path,id:&str)->PathBuf{dir.join(format!("{id}.json"))}
fn database_path(dir:&Path,id:&str)->PathBuf{dir.join(format!("{id}.sqlite3"))}

async fn create(store:&Store,request_id:&str,kind:&str)->Result<Value,sqlx::Error>{
    let root=root(store)?;let dir=backup_dir(&root);fs::create_dir_all(&dir).map_err(|_|io_error("backup directory create failed"))?;
    let id=deterministic_id(kind,request_id);let db=database_path(&dir,&id);let meta=manifest_path(&dir,&id);
    if meta.is_file()&&db.is_file(){let m=read_json(&meta)?;if m["requestId"]==request_id&&m["kind"]==kind&&file_hash(&db)?==m["sha256"]{return Ok(m)}return Err(io_error("backup idempotency conflict"))}
    if db.exists(){fs::remove_file(&db).map_err(|_|io_error("stale backup remove failed"))?}
    let sql=format!("VACUUM INTO '{}'",quote_sqlite_path(&db));sqlx::query(&sql).execute(&store.pool).await?;
    let check=inspect(&db).await?;let bytes=fs::metadata(&db).map_err(|_|io_error("backup metadata failed"))?.len();let sha=file_hash(&db)?;
    let m=json!({"manifestVersion":MANIFEST_VERSION,"id":id,"kind":kind,"requestId":request_id,"createdAt":chrono::Utc::now().to_rfc3339(),"schemaVersion":check["schemaVersion"],"sizeBytes":bytes,"sha256":sha,"integrity":"ok","foreignKeyViolations":0});
    write_json_atomic(&meta,&m)?;Ok(m)
}

fn list_manifests(root:&Path)->Result<Vec<Value>,sqlx::Error>{let dir=backup_dir(root);if !dir.exists(){return Ok(vec![])}let mut out=vec![];for e in fs::read_dir(&dir).map_err(|_|io_error("backup directory read failed"))?{let p=e.map_err(|_|io_error("backup directory entry failed"))?.path();if p.extension().and_then(|x|x.to_str())!=Some("json"){continue}if let Ok(v)=read_json(&p){if v["manifestVersion"]==MANIFEST_VERSION&&v["id"].as_str().is_some_and(safe_id){out.push(v)}}}out.sort_by(|a,b|b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));Ok(out)}
fn find_manifest(root:&Path,id:&str)->Result<Value,sqlx::Error>{if !safe_id(id){return Err(io_error("invalid backup id"))}let m=read_json(&manifest_path(&backup_dir(root),id))?;if m["id"]!=id{return Err(io_error("backup manifest mismatch"))}Ok(m)}

pub async fn dispatch(store:&Store,r:&Value)->Result<Value,sqlx::Error>{
    let root=root(store)?;let command=r["command"].as_str().unwrap_or("");
    match command{
        "backup.create"=>create(store,r["requestId"].as_str().unwrap(),"manual").await,
        "backup.list"=>Ok(json!({"items":list_manifests(&root)?})),
        "backup.restore.status"=>{let pending=if pending_path(&root).is_file(){Some(read_json(&pending_path(&root))?)}else{None};let last=if result_path(&root).is_file(){Some(read_json(&result_path(&root))?)}else{None};Ok(json!({"pending":pending,"last":last}))},
        "backup.restore.preflight"=>{let id=r["payload"]["backupId"].as_str().unwrap();let m=find_manifest(&root,id)?;let db=database_path(&backup_dir(&root),id);let sha=file_hash(&db)?;if m["sha256"]!=sha{return Err(io_error("backup checksum mismatch"))}let check=inspect(&db).await?;let current:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&store.pool).await?;Ok(json!({"backup":m,"source":check,"currentSchemaVersion":current,"safetyCopyRequired":true,"restartRequired":true}))},
        "backup.restore.request"=>{
            let id=r["payload"]["backupId"].as_str().unwrap();let request_id=r["requestId"].as_str().unwrap();let m=find_manifest(&root,id)?;let db=database_path(&backup_dir(&root),id);if file_hash(&db)?!=m["sha256"]{return Err(io_error("backup checksum mismatch"))}let _=inspect(&db).await?;
            let pending=pending_path(&root);if pending.is_file(){let old=read_json(&pending)?;if old["requestId"]==request_id&&old["backupId"]==id{return Ok(json!({"accepted":true,"backupId":id,"safetyBackupId":old["safetyBackupId"],"restartRequired":true}))}return Err(io_error("restore already pending"))}
            let safety=create(store,request_id,"safety").await?;let p=json!({"manifestVersion":1,"requestId":request_id,"backupId":id,"backupSha256":m["sha256"],"safetyBackupId":safety["id"],"requestedAt":chrono::Utc::now().to_rfc3339()});write_json_atomic(&pending,&p)?;
            Ok(json!({"accepted":true,"backupId":id,"safetyBackupId":safety["id"],"restartRequired":true}))
        },
        _=>Err(io_error("unknown backup command"))
    }
}

async fn apply_pending_restore_inner(app_data:&Path)->Result<Option<Value>,sqlx::Error>{
    let pending=pending_path(app_data);if !pending.is_file(){return Ok(None)}let p=read_json(&pending)?;let id=p["backupId"].as_str().filter(|x|safe_id(x)).ok_or_else(||io_error("pending restore invalid"))?;let dir=backup_dir(app_data);let source=database_path(&dir,id);if file_hash(&source)?!=p["backupSha256"]{return Err(io_error("pending restore checksum mismatch"))}let source_check=inspect(&source).await?;
    let target=app_data.join("foundation.sqlite3");let tmp=app_data.join("foundation.restore.tmp.sqlite3");let old=app_data.join("foundation.restore.previous.sqlite3");for p in [&tmp,&old]{let _=fs::remove_file(p);}fs::copy(&source,&tmp).map_err(|_|io_error("restore staging copy failed"))?;let _=inspect(&tmp).await?;
    let _=fs::remove_file(app_data.join("foundation.sqlite3-wal"));let _=fs::remove_file(app_data.join("foundation.sqlite3-shm"));
    if target.exists(){fs::rename(&target,&old).map_err(|_|io_error("restore current database move failed"))?}
    if let Err(_)=fs::rename(&tmp,&target){if old.exists(){let _=fs::rename(&old,&target);}return Err(io_error("restore replacement failed"))}
    let restored=inspect(&target).await?;let result=json!({"applied":true,"backupId":id,"safetyBackupId":p["safetyBackupId"],"appliedAt":chrono::Utc::now().to_rfc3339(),"sourceSchemaVersion":source_check["schemaVersion"],"restoredSchemaVersion":restored["schemaVersion"]});write_json_atomic(&result_path(app_data),&result)?;fs::remove_file(&pending).map_err(|_|io_error("pending restore cleanup failed"))?;let _=fs::remove_file(old);Ok(Some(result))
}
pub async fn apply_pending_restore(app_data:&Path)->Result<Option<Value>,sqlx::Error>{match apply_pending_restore_inner(app_data).await{Ok(v)=>Ok(v),Err(e)=>{let pending=pending_path(app_data);let request=if pending.is_file(){read_json(&pending).ok()}else{None};let result=json!({"applied":false,"backupId":request.as_ref().and_then(|v|v["backupId"].as_str()).unwrap_or("unknown"),"safetyBackupId":request.as_ref().and_then(|v|v["safetyBackupId"].as_str()).unwrap_or("unknown"),"appliedAt":chrono::Utc::now().to_rfc3339(),"error":"RESTORE_FAILED"});let _=write_json_atomic(&result_path(app_data),&result);let _=fs::remove_file(&pending);let _=fs::remove_file(app_data.join("foundation.restore.tmp.sqlite3"));Ok(Some(result))}}}
