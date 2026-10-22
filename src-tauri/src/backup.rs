use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::{Connection,SqliteConnection};
use std::{fs,fs::OpenOptions,io::{Read,Write},path::{Path,PathBuf}};
use crate::persistence::Store;

const CURRENT_SCHEMA:i64=12;
const MANIFEST_VERSION:i64=1;

fn io_error(message:&str)->sqlx::Error{sqlx::Error::Protocol(message.into())}
fn root(store:&Store)->Result<PathBuf,sqlx::Error>{store.database_path().and_then(Path::parent).map(Path::to_path_buf).ok_or_else(||io_error("backup unavailable for in-memory storage"))}
fn backup_dir(root:&Path)->PathBuf{root.join("backups")}
fn ledger_dir(root:&Path)->PathBuf{root.join("restore-ledger")}
fn request_dir(root:&Path)->PathBuf{ledger_dir(root).join("requests")}
fn result_dir(root:&Path)->PathBuf{ledger_dir(root).join("results")}
fn safe_id(s:&str)->bool{!s.is_empty()&&s.len()<=80&&s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_')}
fn deterministic_id(kind:&str,request_id:&str)->String{let h=format!("{:x}",Sha256::digest(format!("ai-home:{kind}:{request_id}").as_bytes()));format!("{kind}-{}",&h[..32])}
fn request_key(request_id:&str)->String{format!("{:x}",Sha256::digest(format!("ai-home:restore-request:{request_id}").as_bytes()))}
fn quote_sqlite_path(path:&Path)->String{path.to_string_lossy().replace('\\',"/").replace('\'',"''")}
fn manifest_path(dir:&Path,id:&str)->PathBuf{dir.join(format!("{id}.json"))}
fn database_path(dir:&Path,id:&str)->PathBuf{dir.join(format!("{id}.sqlite3"))}
fn request_path(root:&Path,request_id:&str)->PathBuf{request_dir(root).join(format!("{}.json",request_key(request_id)))}
fn result_for(root:&Path,request_id:&str)->PathBuf{result_dir(root).join(format!("{}.json",request_key(request_id)))}
fn file_hash(path:&Path)->Result<String,sqlx::Error>{let mut f=fs::File::open(path).map_err(|_|io_error("backup file open failed"))?;let mut h=Sha256::new();let mut b=[0u8;65536];loop{let n=f.read(&mut b).map_err(|_|io_error("backup file read failed"))?;if n==0{break}h.update(&b[..n]);}Ok(format!("{:x}",h.finalize()))}
fn read_json(path:&Path)->Result<Value,sqlx::Error>{serde_json::from_slice(&fs::read(path).map_err(|_|io_error("backup metadata read failed"))?).map_err(|_|io_error("backup metadata invalid"))}

#[cfg(unix)]
fn sync_parent(path:&Path)->Result<(),sqlx::Error>{let parent=path.parent().ok_or_else(||io_error("backup metadata parent missing"))?;fs::File::open(parent).and_then(|f|f.sync_all()).map_err(|_|io_error("backup metadata directory sync failed"))}
#[cfg(not(unix))]
fn sync_parent(_path:&Path)->Result<(),sqlx::Error>{Ok(())}

/// Durable create-once sidecar. We never remove/replace an existing request/result file.
/// This avoids the old Windows-visible gap where metadata disappeared before rename.
fn write_json_once(path:&Path,value:&Value)->Result<(),sqlx::Error>{
    if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(|_|io_error("backup metadata directory create failed"))?;}
    if path.exists(){let old=read_json(path)?;if old==*value{return Ok(())}return Err(io_error("backup metadata idempotency conflict"))}
    let tmp=path.with_file_name(format!(".{}.{}.tmp",path.file_name().and_then(|x|x.to_str()).unwrap_or("metadata"),uuid::Uuid::new_v4()));
    let mut f=OpenOptions::new().create_new(true).write(true).open(&tmp).map_err(|_|io_error("backup metadata create failed"))?;
    f.write_all(value.to_string().as_bytes()).map_err(|_|io_error("backup metadata write failed"))?;f.sync_all().map_err(|_|io_error("backup metadata sync failed"))?;drop(f);
    match fs::rename(&tmp,path){Ok(())=>{sync_parent(path)?;Ok(())},Err(_)=>{let _=fs::remove_file(&tmp);if path.exists()&&read_json(path)?==*value{Ok(())}else{Err(io_error("backup metadata publish failed"))}}}
}

async fn inspect(path:&Path)->Result<Value,sqlx::Error>{
    if !path.is_file(){return Err(io_error("backup missing"))}
    let options=sqlx::sqlite::SqliteConnectOptions::new().filename(path).read_only(true).create_if_missing(false);
    let mut c=SqliteConnection::connect_with(&options).await?;
    let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut c).await?;
    if !(1..=CURRENT_SCHEMA).contains(&version){return Err(io_error("unsupported backup schema"))}
    let integrity:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut c).await?;if integrity!="ok"{return Err(io_error("backup integrity check failed"))}
    let fk=sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut c).await?.len() as i64;if fk!=0{return Err(io_error("backup foreign key check failed"))}
    let profiles:i64=sqlx::query_scalar("SELECT count(*) FROM profiles").fetch_one(&mut c).await.unwrap_or(0);
    let conversations:i64=sqlx::query_scalar("SELECT count(*) FROM conversations").fetch_one(&mut c).await.unwrap_or(0);c.close().await?;
    Ok(json!({"schemaVersion":version,"integrity":"ok","foreignKeyViolations":0,"profileCount":profiles,"conversationCount":conversations}))
}

async fn create(store:&Store,request_id:&str,kind:&str)->Result<Value,sqlx::Error>{
    let root=root(store)?;let dir=backup_dir(&root);fs::create_dir_all(&dir).map_err(|_|io_error("backup directory create failed"))?;
    let id=deterministic_id(kind,request_id);let db=database_path(&dir,&id);let meta=manifest_path(&dir,&id);
    if meta.is_file()&&db.is_file(){let m=read_json(&meta)?;if m["requestId"]==request_id&&m["kind"]==kind&&file_hash(&db)?==m["sha256"]{return Ok(m)}return Err(io_error("backup idempotency conflict"))}
    if db.exists(){fs::remove_file(&db).map_err(|_|io_error("stale backup remove failed"))?}
    let sql=format!("VACUUM INTO '{}'",quote_sqlite_path(&db));sqlx::query(&sql).execute(&store.pool).await?;
    let check=inspect(&db).await?;let bytes=fs::metadata(&db).map_err(|_|io_error("backup metadata failed"))?.len();let sha=file_hash(&db)?;
    let m=json!({"manifestVersion":MANIFEST_VERSION,"id":id,"kind":kind,"requestId":request_id,"createdAt":chrono::Utc::now().to_rfc3339(),"schemaVersion":check["schemaVersion"],"sizeBytes":bytes,"sha256":sha,"integrity":"ok","foreignKeyViolations":0});
    write_json_once(&meta,&m)?;Ok(m)
}

fn list_manifests(root:&Path)->Result<Vec<Value>,sqlx::Error>{let dir=backup_dir(root);if !dir.exists(){return Ok(vec![])}let mut out=vec![];for e in fs::read_dir(&dir).map_err(|_|io_error("backup directory read failed"))?{let p=e.map_err(|_|io_error("backup directory entry failed"))?.path();if p.extension().and_then(|x|x.to_str())!=Some("json"){continue}if let Ok(v)=read_json(&p){if v["manifestVersion"]==MANIFEST_VERSION&&v["id"].as_str().is_some_and(safe_id){out.push(v)}}}out.sort_by(|a,b|b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));Ok(out)}
fn find_manifest(root:&Path,id:&str)->Result<Value,sqlx::Error>{if !safe_id(id){return Err(io_error("invalid backup id"))}let m=read_json(&manifest_path(&backup_dir(root),id))?;if m["id"]!=id{return Err(io_error("backup manifest mismatch"))}Ok(m)}

fn request_records(root:&Path)->Result<Vec<Value>,sqlx::Error>{let dir=request_dir(root);if !dir.exists(){return Ok(vec![])}let mut out=vec![];for e in fs::read_dir(dir).map_err(|_|io_error("restore ledger read failed"))?{let p=e.map_err(|_|io_error("restore ledger entry failed"))?.path();if p.extension().and_then(|x|x.to_str())==Some("json"){if let Ok(v)=read_json(&p){out.push(v)}}}Ok(out)}
fn result_records(root:&Path)->Result<Vec<Value>,sqlx::Error>{let dir=result_dir(root);if !dir.exists(){return Ok(vec![])}let mut out=vec![];for e in fs::read_dir(dir).map_err(|_|io_error("restore result read failed"))?{let p=e.map_err(|_|io_error("restore result entry failed"))?.path();if p.extension().and_then(|x|x.to_str())==Some("json"){if let Ok(v)=read_json(&p){out.push(v)}}}out.sort_by(|a,b|b["appliedAt"].as_str().cmp(&a["appliedAt"].as_str()));Ok(out)}
fn result_by_request(root:&Path,request_id:&str)->Result<Option<Value>,sqlx::Error>{let p=result_for(root,request_id);if p.is_file(){Ok(Some(read_json(&p)?))}else{Ok(None)}}
fn request_by_id(root:&Path,request_id:&str)->Result<Option<Value>,sqlx::Error>{let p=request_path(root,request_id);if p.is_file(){Ok(Some(read_json(&p)?))}else{Ok(None)}}
fn pending_request(root:&Path)->Result<Option<Value>,sqlx::Error>{let mut pending=vec![];for r in request_records(root)?{let req=r["requestId"].as_str().ok_or_else(||io_error("restore request invalid"))?;if result_by_request(root,req)?.is_none(){pending.push(r)}}if pending.len()>1{return Err(io_error("multiple pending restores"))}Ok(pending.pop())}
pub fn restore_pending_for_store(store:&Store)->bool{root(store).ok().and_then(|r|pending_request(&r).ok().flatten()).is_some()}

async fn freeze_operations(store:&Store)->Result<(),sqlx::Error>{
    let mut tx=store.pool.begin().await?;let rows=sqlx::query("SELECT profile_id,id,revision,body FROM operations WHERE json_extract(body,'$.status') IN ('queued','running')").fetch_all(&mut *tx).await?;
    for row in rows{let mut op:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|io_error("invalid operation during restore freeze"))?;let rev=row.get::<i64,_>("revision")+1;op["revision"]=json!(rev);op["status"]=json!("cancelled");op["finishedAt"]=json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true));op["error"]=json!({"code":"OPERATION_CANCELLED","messageKey":"error.operation_cancelled","retryable":false,"operationId":op["id"]});sqlx::query("UPDATE operations SET revision=?,body=? WHERE profile_id=? AND id=? AND revision=?").bind(rev).bind(op.to_string()).bind(row.get::<&str,_>("profile_id")).bind(row.get::<&str,_>("id")).bind(row.get::<i64,_>("revision")).execute(&mut *tx).await?;}
    tx.commit().await?;Ok(())
}

async fn validate_restore_target(store:&Store,id:&str)->Result<Value,sqlx::Error>{let root=root(store)?;let m=find_manifest(&root,id)?;let db=database_path(&backup_dir(&root),id);if file_hash(&db)?!=m["sha256"]{return Err(io_error("backup checksum mismatch"))}let check=inspect(&db).await?;Ok(json!({"manifest":m,"check":check}))}

pub async fn dispatch(store:&Store,r:&Value)->Result<Value,sqlx::Error>{
    let root=root(store)?;let command=r["command"].as_str().unwrap_or("");
    match command{
        "backup.create"=>create(store,r["requestId"].as_str().unwrap(),"manual").await,
        "backup.list"=>Ok(json!({"items":list_manifests(&root)?})),
        "backup.restore.status"=>{let pending=pending_request(&root)?;let last=result_records(&root)?.into_iter().next();Ok(json!({"pending":pending,"last":last}))},
        "backup.restore.preflight"=>{let id=r["payload"]["backupId"].as_str().unwrap();let v=validate_restore_target(store,id).await?;let current:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&store.pool).await?;Ok(json!({"backup":v["manifest"],"source":v["check"],"currentSchemaVersion":current,"safetyCopyRequired":true,"restartRequired":true}))},
        "backup.restore.request"=>{
            let id=r["payload"]["backupId"].as_str().unwrap();let request_id=r["requestId"].as_str().unwrap();
            if let Some(done)=result_by_request(&root,request_id)?{if done["backupId"]!=id{return Err(io_error("restore idempotency conflict"))}return Ok(json!({"accepted":true,"backupId":id,"safetyBackupId":done["safetyBackupId"],"restartRequired":true}))}
            if let Some(old)=request_by_id(&root,request_id)?{if old["backupId"]!=id{return Err(io_error("restore idempotency conflict"))}return Ok(json!({"accepted":true,"backupId":id,"safetyBackupId":old["safetyBackupId"],"restartRequired":true}))}
            let _=validate_restore_target(store,id).await?;
            if pending_request(&root)?.is_some(){return Err(io_error("restore already pending"))}
            freeze_operations(store).await?;
            // This is the final logical state: after this request the application rejects every mutation until restart.
            let safety=create(store,request_id,"safety").await?;let source=find_manifest(&root,id)?;
            let rec=json!({"manifestVersion":2,"requestId":request_id,"backupId":id,"backupSha256":source["sha256"],"safetyBackupId":safety["id"],"requestedAt":chrono::Utc::now().to_rfc3339()});write_json_once(&request_path(&root,request_id),&rec)?;
            Ok(json!({"accepted":true,"backupId":id,"safetyBackupId":safety["id"],"restartRequired":true}))
        },
        _=>Err(io_error("unknown backup command"))
    }
}

async fn checkpoint_current(path:&Path)->Result<(),sqlx::Error>{
    if !path.is_file(){return Err(io_error("current database missing during restore"))}
    let options=sqlx::sqlite::SqliteConnectOptions::new().filename(path).create_if_missing(false);
    let mut c=SqliteConnection::connect_with(&options).await?;let _:Vec<sqlx::sqlite::SqliteRow>=sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").fetch_all(&mut c).await?;c.close().await?;Ok(())
}
fn copy_verified_source(source:&Path,dest:&Path)->Result<(),sqlx::Error>{if dest.exists(){fs::remove_file(dest).map_err(|_|io_error("restore stale staging remove failed"))?}fs::copy(source,dest).map_err(|_|io_error("restore staging copy failed"))?;fs::File::open(dest).and_then(|f|f.sync_all()).map_err(|_|io_error("restore staging sync failed"))?;Ok(())}

async fn recover_current_from_safety(app_data:&Path,req:&Value)->Result<(),sqlx::Error>{
    let target=app_data.join("foundation.sqlite3");if target.is_file()&&inspect(&target).await.is_ok(){return Ok(())}
    let safety=req["safetyBackupId"].as_str().filter(|x|safe_id(x)).ok_or_else(||io_error("restore safety id invalid"))?;let source=database_path(&backup_dir(app_data),safety);let m=find_manifest(app_data,safety)?;if file_hash(&source)?!=m["sha256"]{return Err(io_error("restore safety checksum mismatch"))}let _=inspect(&source).await?;
    let tmp=app_data.join(format!("foundation.recovery.{}.tmp.sqlite3",request_key(req["requestId"].as_str().unwrap())));copy_verified_source(&source,&tmp)?;let _=inspect(&tmp).await?;
    if target.exists(){let quarantine=app_data.join(format!("foundation.restore.corrupt.{}.sqlite3",request_key(req["requestId"].as_str().unwrap())));if !quarantine.exists(){fs::rename(&target,&quarantine).map_err(|_|io_error("restore corrupt database quarantine failed"))?}else{fs::remove_file(&target).map_err(|_|io_error("restore corrupt database remove failed"))?}}
    fs::rename(&tmp,&target).map_err(|_|io_error("restore safety recovery publish failed"))?;sync_parent(&target)?;let _=inspect(&target).await?;Ok(())
}

async fn apply_one(app_data:&Path,req:&Value)->Result<Value,sqlx::Error>{
    let request_id=req["requestId"].as_str().ok_or_else(||io_error("pending restore request id invalid"))?;let id=req["backupId"].as_str().filter(|x|safe_id(x)).ok_or_else(||io_error("pending restore invalid"))?;
    if let Some(done)=result_by_request(app_data,request_id)?{return Ok(done)}
    let source=database_path(&backup_dir(app_data),id);if file_hash(&source)?!=req["backupSha256"]{return Err(io_error("pending restore checksum mismatch"))}let source_check=inspect(&source).await?;
    recover_current_from_safety(app_data,req).await?;let target=app_data.join("foundation.sqlite3");
    if file_hash(&target).ok().as_deref()==req["backupSha256"].as_str()&&inspect(&target).await.is_ok(){let result=json!({"applied":true,"requestId":request_id,"backupId":id,"safetyBackupId":req["safetyBackupId"],"appliedAt":chrono::Utc::now().to_rfc3339(),"sourceSchemaVersion":source_check["schemaVersion"],"restoredSchemaVersion":source_check["schemaVersion"]});write_json_once(&result_for(app_data,request_id),&result)?;return Ok(result)}
    // WAL/SHM are never unlinked manually. SQLite checkpoints the closed current database after a verified final safety snapshot exists.
    checkpoint_current(&target).await?;
    let key=request_key(request_id);let staging=app_data.join(format!("foundation.restore.staging.{key}.sqlite3"));let previous=app_data.join(format!("foundation.restore.previous.{key}.sqlite3"));copy_verified_source(&source,&staging)?;let _=inspect(&staging).await?;
    if !previous.exists(){fs::rename(&target,&previous).map_err(|_|io_error("restore current database preserve failed"))?;sync_parent(&previous)?}else if target.exists(){fs::remove_file(&target).map_err(|_|io_error("restore retry target remove failed"))?}
    if let Err(_)=fs::rename(&staging,&target){if !target.exists()&&previous.exists(){let _=fs::rename(&previous,&target);}return Err(io_error("restore replacement failed"))}sync_parent(&target)?;
    let restored=match inspect(&target).await{Ok(v)=>v,Err(e)=>{let failed=app_data.join(format!("foundation.restore.failed.{key}.sqlite3"));let _=fs::rename(&target,&failed);recover_current_from_safety(app_data,req).await?;return Err(e)}};
    if file_hash(&target)?!=req["backupSha256"]{recover_current_from_safety(app_data,req).await?;return Err(io_error("restored database checksum mismatch"))}
    let result=json!({"applied":true,"requestId":request_id,"backupId":id,"safetyBackupId":req["safetyBackupId"],"appliedAt":chrono::Utc::now().to_rfc3339(),"sourceSchemaVersion":source_check["schemaVersion"],"restoredSchemaVersion":restored["schemaVersion"]});write_json_once(&result_for(app_data,request_id),&result)?;Ok(result)
}

/// Startup restore is fail-closed. We never swallow an error and then let Store::open(create_if_missing=true)
/// manufacture an empty replacement for user data. A failure aborts startup with the safety backup and ledger intact.
pub async fn apply_pending_restore(app_data:&Path)->Result<Option<Value>,sqlx::Error>{
    fs::create_dir_all(request_dir(app_data)).map_err(|_|io_error("restore ledger directory create failed"))?;fs::create_dir_all(result_dir(app_data)).map_err(|_|io_error("restore result directory create failed"))?;
    let Some(req)=pending_request(app_data)? else{return Ok(None)};apply_one(app_data,&req).await.map(Some)
}
