//! Local maintenance only: drain writers, delete scoped secrets, snapshot and
//! merge metadata without rolling back live profiles or trusting restored tokens.
use super::super::tokens::CommunityTokenStore;
use super::*;
use sqlx::Connection;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    Registry(RegistryError),
    Identity(IdentityError),
    Io,
    InvalidSnapshot,
}
impl From<RegistryError> for LifecycleError {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}
impl From<IdentityError> for LifecycleError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
impl From<sqlx::Error> for LifecycleError {
    fn from(_: sqlx::Error) -> Self {
        Self::Registry(RegistryError::Storage)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub sha256: String,
    pub profile_count: usize,
    pub secrets_included: bool,
    pub requires_reconciliation: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub profiles: usize,
    pub skipped_live_or_deleted: usize,
    pub imported_tombstones: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciled {
    pub active: bool,
    pub missing_keys: usize,
    pub pending_provisioning: usize,
    pub token_recovery_required: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deleted {
    pub key_references_processed: usize,
    pub token_references_processed: usize,
}

pub async fn state(
    registry: &RegistrationRegistry,
    profile: &str,
) -> Result<Option<String>, LifecycleError> {
    if !scalar(profile, 256) {
        return Err(RegistryError::PolicyDenied.into());
    }
    Ok(
        sqlx::query_scalar("SELECT state FROM community_profile_lifecycle WHERE profile_id=?")
            .bind(profile)
            .fetch_optional(&registry.pool)
            .await?,
    )
}
/// Local maintenance is not cancelled midway when the caller drops its future.
/// The worker and blocking secret calls retain the exclusive owner until done.
pub async fn delete_profile(
    registry: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    tokens: &CommunityTokenStore,
    profile: &str,
) -> Result<Deleted, LifecycleError> {
    let r = registry.clone();
    let keys = keys.clone();
    let tokens = tokens.clone();
    let profile = profile.to_string();
    tokio::spawn(async move{
        if !scalar(&profile,256){return Err(RegistryError::PolicyDenied.into());}
        let exclusive=r.gate.exclusive().await;if r.gate.fenced(){return Err(RegistryError::PolicyDenied.into());}let keys=keys.scoped(exclusive.clone());let tokens=tokens.scoped(exclusive);
        sqlx::query("INSERT INTO community_profile_lifecycle(profile_id,state,revision) VALUES(?,'deleting',1) ON CONFLICT(profile_id) DO UPDATE SET state=CASE WHEN state='deleted' THEN 'deleted' ELSE 'deleting' END,revision=revision+1").bind(&profile).execute(&r.pool).await?;
        let scopes=scopes(&r,&profile).await?;let mut key_count=0;let mut token_count=0;
        for scope in scopes {
            let reference:String=sqlx::query_scalar("SELECT key_reference FROM community_preparations WHERE scope_binding=?").bind(scope.binding()).fetch_one(&r.pool).await?;
            keys.remove(&scope,&reference).await?;key_count+=1;
            let mut refs:HashSet<String>=sqlx::query_scalar("SELECT token_ref FROM community_registration_dispatches WHERE scope_binding=? AND token_ref IS NOT NULL").bind(scope.binding()).fetch_all(&r.pool).await?.into_iter().collect();
            if let Some(raw)=sqlx::query_scalar::<_,String>("SELECT body FROM community_registration_finishes WHERE scope_binding=?").bind(scope.binding()).fetch_optional(&r.pool).await? {
                let v:Value=serde_json::from_str(&raw).map_err(|_|LifecycleError::InvalidSnapshot)?;
                if let Some(reference)=v["tokenRef"].as_str(){refs.insert(reference.into());}
            }
            for reference in refs {tokens.remove_scoped(&scope,&reference).await?;token_count+=1;}
        }
        let mut tx=r.pool.begin().await?;
        for table in ["community_registration_dispatches","community_registration_finishes","community_registration_starts"]{
            sqlx::query(&format!("DELETE FROM {table} WHERE scope_binding IN(SELECT scope_binding FROM community_preparations WHERE profile_id=?)")).bind(&profile).execute(&mut *tx).await?;
        }
        sqlx::query("DELETE FROM community_preparations WHERE profile_id=?").bind(&profile).execute(&mut *tx).await?;
        sqlx::query("UPDATE community_profile_lifecycle SET state='deleted',revision=revision+1 WHERE profile_id=?").bind(&profile).execute(&mut *tx).await?;tx.commit().await?;
        Ok(Deleted{key_references_processed:key_count,token_references_processed:token_count})
    }).await.map_err(|_|LifecycleError::Io)?
}
pub async fn snapshot(
    registry: &RegistrationRegistry,
    path: &Path,
) -> Result<Snapshot, LifecycleError> {
    let registry = registry.clone();
    let path = path.to_path_buf();
    tokio::spawn(async move{let lease=registry.maintenance().await;if registry.gate.fenced(){return Err(RegistryError::PolicyDenied.into());}snapshot_with_lease(&registry,&path,&lease).await}).await.map_err(|_|LifecycleError::Io)?
}
pub(crate) async fn snapshot_with_lease(registry:&RegistrationRegistry,path:&Path,lease:&gate::Lease)->Result<Snapshot,LifecycleError>{
    if !lease.exclusive_for(&registry.gate){return Err(RegistryError::PolicyDenied.into());}
        if !path.is_absolute() || path.to_str().is_none() { return Err(LifecycleError::Io); }
        if sqlx::query_scalar::<_,i64>("SELECT count(*) FROM community_profile_lifecycle WHERE state='deleting'").fetch_one(&registry.pool).await?!=0{return Err(RegistryError::PolicyDenied.into());}
        let mut options=std::fs::OpenOptions::new();options.create_new(true).write(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let file=options.open(&path).map_err(|_|LifecycleError::Io)?;drop(file);
        let result=async {
            let quoted=path.to_str().ok_or(LifecycleError::Io)?.replace('\'',"''");
            sqlx::query(&format!("VACUUM INTO '{quoted}'")).execute(&registry.pool).await?;
            let mut copy=sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path).create_if_missing(false)).await?;
            let mut tx=copy.begin().await?;
            sqlx::query("UPDATE community_profile_lifecycle SET state='reconcile',revision=revision+1 WHERE state='active'").execute(&mut *tx).await?;
            sqlx::query("UPDATE community_registration_finishes SET revision=revision+1,body=json_set(body,'$.state','uncertain','$.tokenRef',NULL)").execute(&mut *tx).await?;
            tx.commit().await?;copy.close().await?;
            if path.metadata().map_err(|_|LifecycleError::Io)?.len()>64*1024*1024 { return Err(LifecycleError::InvalidSnapshot); }
            let bytes=std::fs::read(&path).map_err(|_|LifecycleError::Io)?;
            let profile_count=sqlx::query_scalar::<_,i64>("SELECT count(*) FROM community_profile_lifecycle").fetch_one(&registry.pool).await? as usize;
            std::fs::OpenOptions::new().read(true).write(true).open(&path).and_then(|f|f.sync_all()).map_err(|_|LifecycleError::Io)?;
            #[cfg(unix)] {std::fs::File::open(path.parent().ok_or(LifecycleError::Io)?).and_then(|f|f.sync_all()).map_err(|_|LifecycleError::Io)?;}
            Ok(Snapshot{sha256:super::super::crypto::digest(&bytes),profile_count,secrets_included:false,requires_reconciliation:true})
        }.await;
        if result.is_err(){let _=std::fs::remove_file(&path);}result

}
// Merge only absent profiles. Never roll back live metadata or erase a deletion
// tombstone. Restored credentials stay disabled until explicit reconciliation.
pub async fn restore_metadata(
    registry: &RegistrationRegistry,
    path: &Path,
    expected_sha256: &str,
) -> Result<Restored, LifecycleError> {
    let registry = registry.clone();
    let path = path.to_path_buf();
    let sha = expected_sha256.to_string();
    tokio::spawn(async move{let lease=registry.maintenance().await;if registry.gate.fenced(){return Err(RegistryError::PolicyDenied.into());}restore_with_lease(&registry,&path,&sha,&lease).await}).await.map_err(|_|LifecycleError::Io)?
}
pub(crate) async fn restore_with_lease(registry:&RegistrationRegistry,path:&Path,sha:&str,lease:&gate::Lease)->Result<Restored,LifecycleError>{
    if !lease.exclusive_for(&registry.gate){return Err(RegistryError::PolicyDenied.into());}
        let before=read_snapshot(&path,&sha)?;
        let mut source=sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path).read_only(true).immutable(true).create_if_missing(false)).await?;
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut source).await?;
        let app:i64=sqlx::query_scalar("PRAGMA application_id").fetch_one(&mut source).await?;
        if version!=4||app!=1095254851{return Err(LifecycleError::InvalidSnapshot);}
        let source_schema=sqlx::query("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").fetch_all(&mut source).await?;
        let target_schema=sqlx::query("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").fetch_all(&registry.pool).await?;
        if schema_shape(source_schema)?!=schema_shape(target_schema)?{return Err(LifecycleError::InvalidSnapshot);}
        let check:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut source).await?;
        if version!=4||app!=1095254851||check!="ok"||!sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut source).await?.is_empty(){return Err(LifecycleError::InvalidSnapshot);}
        let profiles=sqlx::query("SELECT profile_id,state FROM community_profile_lifecycle").fetch_all(&mut source).await?;
        let mut tx=registry.pool.begin().await?;let mut report=Restored{profiles:0,skipped_live_or_deleted:0,imported_tombstones:0};
        for row in profiles {
            let profile:String=row.try_get("profile_id")?;let status:String=row.try_get("state")?;
            if !scalar(&profile,256)||!["reconcile","deleted"].contains(&status.as_str()){return Err(LifecycleError::InvalidSnapshot);}
            if sqlx::query_scalar::<_,i64>("SELECT count(*) FROM community_profile_lifecycle WHERE profile_id=?").bind(&profile).fetch_one(&mut *tx).await?!=0{report.skipped_live_or_deleted+=1;continue;}
            sqlx::query("INSERT INTO community_profile_lifecycle VALUES(?,?,1)").bind(&profile).bind(&status).execute(&mut *tx).await?;
            if status=="deleted"{report.imported_tombstones+=1;continue;}
            for table in ["community_preparations","community_registration_starts","community_registration_finishes","community_registration_dispatches"]{
                copy_rows(&mut source,&mut tx,table,&profile).await?;
            }
            let unsafe_ready:i64=sqlx::query_scalar("SELECT count(*) FROM community_registration_finishes WHERE scope_binding IN(SELECT scope_binding FROM community_preparations WHERE profile_id=?) AND (json_extract(body,'$.state')!='uncertain' OR json_extract(body,'$.tokenRef') IS NOT NULL)").bind(&profile).fetch_one(&mut *tx).await?;
            if unsafe_ready!=0{return Err(LifecycleError::InvalidSnapshot);}report.profiles+=1;
        }
        source.close().await?;if read_snapshot(&path,&sha)?!=before{return Err(LifecycleError::InvalidSnapshot);}
        tx.commit().await?;Ok(report)

}
fn schema_shape(rows:Vec<sqlx::sqlite::SqliteRow>)->Result<Vec<(String,String,String,Option<String>)>,LifecycleError>{
    rows.into_iter().map(|r|Ok((r.try_get("type")?,r.try_get("name")?,r.try_get("tbl_name")?,r.try_get("sql")?))).collect()
}
pub(crate) async fn inspect_snapshot(registry:&RegistrationRegistry,path:&Path,sha:&str,lease:&gate::Lease)->Result<Vec<(String,String)>,LifecycleError>{
    if !lease.exclusive_for(&registry.gate){return Err(RegistryError::PolicyDenied.into());}
    let before=read_snapshot(path,sha)?;
    let mut source=sqlx::SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path).read_only(true).immutable(true).create_if_missing(false)).await?;
    let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut source).await?;
    let app:i64=sqlx::query_scalar("PRAGMA application_id").fetch_one(&mut source).await?;
    if version!=4||app!=1095254851{return Err(LifecycleError::InvalidSnapshot);}
    let a=sqlx::query("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").fetch_all(&mut source).await?;
    let b=sqlx::query("SELECT type,name,tbl_name,sql FROM sqlite_master ORDER BY type,name").fetch_all(&registry.pool).await?;
    if schema_shape(a)?!=schema_shape(b)?{return Err(LifecycleError::InvalidSnapshot);}
    let check:String=sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut source).await?;
    if check!="ok"||!sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut source).await?.is_empty(){return Err(LifecycleError::InvalidSnapshot);}
    let states:Vec<(String,String)>=sqlx::query_as("SELECT profile_id,state FROM community_profile_lifecycle ORDER BY profile_id").fetch_all(&mut source).await?;
    for (profile,state) in &states{
        if !scalar(profile,256)||!["reconcile","deleted"].contains(&state.as_str()){return Err(LifecycleError::InvalidSnapshot);}
        if state=="deleted"&&sqlx::query_scalar::<_,i64>("SELECT count(*) FROM community_preparations WHERE profile_id=?").bind(profile).fetch_one(&mut source).await?!=0{return Err(LifecycleError::InvalidSnapshot);}
    }
    let unsafe_ready:i64=sqlx::query_scalar("SELECT count(*) FROM community_registration_finishes WHERE json_extract(body,'$.state') IS NOT 'uncertain' OR json_extract(body,'$.tokenRef') IS NOT NULL").fetch_one(&mut source).await?;
    let orphan:i64=sqlx::query_scalar("SELECT count(*) FROM community_preparations p WHERE NOT EXISTS(SELECT 1 FROM community_profile_lifecycle l WHERE l.profile_id=p.profile_id AND l.state='reconcile')").fetch_one(&mut source).await?;
    source.close().await?;
    if unsafe_ready!=0||orphan!=0||read_snapshot(path,sha)?!=before{return Err(LifecycleError::InvalidSnapshot);}
    Ok(states)
}
fn read_snapshot(path: &Path, expected: &str) -> Result<Vec<u8>, LifecycleError> {
    if !path.is_absolute() || path.to_str().is_none() { return Err(LifecycleError::Io); }
    let meta = path.metadata().map_err(|_| LifecycleError::Io)?;
    if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
        return Err(LifecycleError::InvalidSnapshot);
    }
    let bytes = std::fs::read(path).map_err(|_| LifecycleError::Io)?;
    if super::super::crypto::digest(&bytes) != expected {
        return Err(LifecycleError::InvalidSnapshot);
    }
    Ok(bytes)
}
async fn copy_rows(
    source: &mut sqlx::SqliteConnection,
    target: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    profile: &str,
) -> Result<(), LifecycleError> {
    let columns = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(&mut **target)
        .await?;
    let names: Vec<String> = columns.iter().map(|r| r.get("name")).collect();
    let list = names.join(",");
    let filter = if table == "community_preparations" {
        "profile_id=?"
    } else {
        "scope_binding IN(SELECT scope_binding FROM community_preparations WHERE profile_id=?)"
    };
    let rows = sqlx::query(&format!("SELECT {list} FROM {table} WHERE {filter}"))
        .bind(profile)
        .fetch_all(&mut *source)
        .await?;
    let sql = format!(
        "INSERT INTO {table}({list}) VALUES({})",
        vec!["?"; names.len()].join(",")
    );
    for row in rows {
        let mut insert = sqlx::query(&sql);
        for (index, col) in columns.iter().enumerate() {
            let kind: String = col.try_get("type")?;
            insert = if kind == "INTEGER" {
                insert.bind(row.try_get::<Option<i64>, _>(index)?)
            } else {
                insert.bind(row.try_get::<Option<String>, _>(index)?)
            };
        }
        insert.execute(&mut **target).await?;
    }
    Ok(())
}
pub async fn reconcile_profile(
    registry: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    profile: &str,
) -> Result<Reconciled, LifecycleError> {
    let registry = registry.clone();
    let keys = keys.clone();
    let profile = profile.to_string();
    tokio::spawn(async move{
        let exclusive=registry.gate.exclusive().await;if registry.gate.fenced(){return Err(RegistryError::PolicyDenied.into());}let keys=keys.scoped(exclusive);
        match state(&registry,&profile).await?.as_deref(){Some("active")=>return Ok(Reconciled{active:true,missing_keys:0,pending_provisioning:0,token_recovery_required:0}),Some("reconcile")=>{},_=>return Err(RegistryError::PolicyDenied.into())}
        let mut result=Reconciled{active:false,missing_keys:0,pending_provisioning:0,token_recovery_required:0};
        for scope in scopes(&registry,&profile).await? {
            let record=registry.get(&scope).await?;
            match record.state {
                PreparationState::Ready=>{
                    match registry.load_intent(&keys,&scope).await {
                        Ok(intent)=>{
                            let raw:Option<Option<String>>=sqlx::query_scalar("SELECT response FROM community_registration_starts WHERE scope_binding=?").bind(scope.binding()).fetch_optional(&registry.pool).await?;
                            if let Some(raw)=raw.flatten(){let v:Value=serde_json::from_str(&raw).map_err(|_|LifecycleError::InvalidSnapshot)?;let context=intent.context(&v,Utc::now(),true,true)?;super::finish::validate_restored(&registry,&scope,context.registration_id()).await.map_err(|_|LifecycleError::InvalidSnapshot)?;result.token_recovery_required+=1;}
                        },Err(RegistryError::Identity(IdentityError::NotFound))=>result.missing_keys+=1,Err(e)=>return Err(e.into())
                    }
                },PreparationState::Reserved=>result.pending_provisioning+=1,
                PreparationState::Provisioning=>result.pending_provisioning+=1,
            }
        }
        if result.missing_keys==0&&result.pending_provisioning==0{sqlx::query("UPDATE community_profile_lifecycle SET state='active',revision=revision+1 WHERE profile_id=? AND state='reconcile'").bind(profile).execute(&registry.pool).await?;result.active=true;}Ok(result)
    }).await.map_err(|_|LifecycleError::Io)?
}
/// Explicitly adopt only an already existing scoped keyset after a restore or
/// interrupted provision. Never create replacement keys for a missing entry.
pub async fn reconcile_keys(
    registry: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    scope: &CommunityScope,
    expected_revision: i64,
) -> Result<Preparation, LifecycleError> {
    let registry = registry.clone();
    let keys = keys.clone();
    let scope = scope.clone();
    tokio::spawn(async move{
        let exclusive=registry.gate.exclusive().await;if registry.gate.fenced(){return Err(RegistryError::PolicyDenied.into());}let keys=keys.scoped(exclusive);
        if !matches!(state(&registry,&scope.profile_id).await?.as_deref(),Some("active"|"reconcile")){return Err(RegistryError::PolicyDenied.into());}
        let record=registry.get(&scope).await?;if record.revision!=expected_revision{return Err(RegistryError::Conflict.into());}
        let public=keys.load(&scope,&record.key_reference).await?;
        match record.state {
            PreparationState::Ready=>{if record.public.as_ref()!=Some(&public){return Err(IdentityError::ScopeMismatch.into());}Ok(record)},
            PreparationState::Provisioning=>Ok(registry.recover(&keys,&scope,expected_revision).await?),
            PreparationState::Reserved=>{
                let n=sqlx::query("UPDATE community_preparations SET state='ready',owner=NULL,revision=revision+1,identity_public_key=?,operational_public_key=? WHERE scope_binding=? AND revision=? AND state='reserved'").bind(public.identity_public_key()).bind(public.operational_public_key()).bind(scope.binding()).bind(expected_revision).execute(&registry.pool).await?.rows_affected();
                if n!=1{return Err(RegistryError::Conflict.into());}Ok(registry.get(&scope).await?)
            }
        }
    }).await.map_err(|_|LifecycleError::Io)?
}
async fn scopes(
    registry: &RegistrationRegistry,
    profile: &str,
) -> Result<Vec<CommunityScope>, LifecycleError> {
    let rows=sqlx::query("SELECT api_root,origin,instance,scope_binding FROM community_preparations WHERE profile_id=?").bind(profile).fetch_all(&registry.pool).await?;
    rows.into_iter()
        .map(|r| {
            let scope = CommunityScope::new(
                profile,
                r.try_get("api_root")?,
                r.try_get("origin")?,
                r.try_get("instance")?,
            )?;
            if scope.binding() != r.try_get::<String, _>("scope_binding")? {
                return Err(LifecycleError::InvalidSnapshot);
            }
            Ok(scope)
        })
        .collect()
}
#[cfg(test)]
mod tests;
