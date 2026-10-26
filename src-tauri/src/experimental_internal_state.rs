use chrono::{DateTime,Utc};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::{Row,SqlitePool};
use crate::domain::schema;

#[derive(Clone,Debug)]
pub struct InternalStateInput {
    pub previous_state:Option<Vec<u8>>,
    pub previous_revision:i64,
    pub elapsed_ms:i64,
    pub now:DateTime<Utc>,
}
#[derive(Clone,Debug)]
pub struct NeutralEventDraft { pub kind:String, pub description:String, pub occurred_at:DateTime<Utc> }
#[derive(Clone,Debug)]
pub struct WakeCandidateDraft { pub reason_code:String, pub not_before:DateTime<Utc> }
#[derive(Clone,Debug)]
pub struct InternalStateUpdate {
    pub state_blob:Option<Vec<u8>>,
    pub events:Vec<NeutralEventDraft>,
    pub wake_candidates:Vec<WakeCandidateDraft>,
}
pub trait InternalStateProvider:Send+Sync {
    fn kind(&self)->&'static str;
    fn engine_version(&self)->&str;
    fn advance(&self,input:&InternalStateInput)->Result<InternalStateUpdate,String>;
}
#[derive(Default)]
pub struct NullStateProvider;
impl InternalStateProvider for NullStateProvider {
    fn kind(&self)->&'static str{"null"}
    fn engine_version(&self)->&str{"null-state/1"}
    fn advance(&self,_:&InternalStateInput)->Result<InternalStateUpdate,String>{Ok(InternalStateUpdate{state_blob:None,events:vec![],wake_candidates:vec![]})}
}

pub trait ConsciousCoreEngine:Send+Sync {
    fn engine_version(&self)->&str;
    fn transition(&self,input:&InternalStateInput)->Result<InternalStateUpdate,String>;
}
pub struct ConsciousCoreAdapter<E:ConsciousCoreEngine>{engine:E}
impl<E:ConsciousCoreEngine> ConsciousCoreAdapter<E>{pub fn new(engine:E)->Self{Self{engine}}}
impl<E:ConsciousCoreEngine> InternalStateProvider for ConsciousCoreAdapter<E>{
    fn kind(&self)->&'static str{"conscious_core"}
    fn engine_version(&self)->&str{self.engine.engine_version()}
    fn advance(&self,input:&InternalStateInput)->Result<InternalStateUpdate,String>{self.engine.transition(input)}
}

#[derive(Clone,Debug)]
pub struct PersistedInternalState {
    pub snapshot:Value,
    pub neutral_events:Vec<Value>,
    pub wake_candidates:Vec<Value>,
    pub elapsed_ms:i64,
}

pub struct InternalStateService{pool:SqlitePool}
impl InternalStateService{
    pub fn new(pool:SqlitePool)->Self{Self{pool}}
    pub async fn refresh<P:InternalStateProvider>(&self,profile_id:&str,instance_id:&str,provider:&P,now:DateTime<Utc>)->Result<PersistedInternalState,String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let exists:Option<i64>=sqlx::query_scalar("SELECT 1 FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if exists.is_none(){return Err("NOT_FOUND".into())}
        let previous=sqlx::query("SELECT body FROM experimental_internal_state_snapshots WHERE profile_id=? AND instance_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let (previous_state,previous_revision,previous_at)=if let Some(row)=previous{
            let body:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
            if !schema::dto("InternalState",&body){return Err("STORAGE_ERROR".into())}
            let blob=if let Some(blob_id)=body.get("stateBlobId").and_then(Value::as_str){sqlx::query_scalar::<_,Vec<u8>>("SELECT bytes FROM experimental_internal_state_blobs WHERE profile_id=? AND instance_id=? AND id=?").bind(profile_id).bind(instance_id).bind(blob_id).fetch_optional(&mut *tx).await.map_err(storage)?}else{None};
            let at=parse(body["createdAt"].as_str().ok_or("STORAGE_ERROR")?)?;
            (blob,body["revision"].as_i64().unwrap_or(0),Some(at))
        }else{(None,0,None)};
        let elapsed_ms=previous_at.map(|v|(now-v).num_milliseconds().max(0)).unwrap_or(0);
        // One transition receives the whole elapsed interval. We never replay one call per missed tick.
        let input=InternalStateInput{previous_state,previous_revision,elapsed_ms,now};
        let update=provider.advance(&input).map_err(|_|"STATE_PROVIDER_UNAVAILABLE".to_string())?;
        if update.events.iter().any(|e|e.kind.trim().is_empty()||e.description.trim().is_empty())||update.wake_candidates.iter().any(|w|w.reason_code.trim().is_empty()){return Err("VALIDATION_ERROR".into())}
        sqlx::query("UPDATE experimental_wake_candidates SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.status','superseded') WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active'").bind(profile_id).bind(instance_id).execute(&mut *tx).await.map_err(storage)?;
        let snapshot_id=uuid::Uuid::new_v4().to_string(); let revision=previous_revision+1; let created=ts(now);
        let mut snapshot=json!({"schemaVersion":1,"id":snapshot_id,"profileId":profile_id,"revision":revision,"createdAt":created,"kind":provider.kind()});
        if provider.kind()=="conscious_core"{
            let blob=update.state_blob.as_ref().ok_or("VALIDATION_ERROR")?;
            let blob_id=uuid::Uuid::new_v4().to_string();let sha=format!("{:x}",Sha256::digest(blob));
            sqlx::query("INSERT INTO experimental_internal_state_blobs(profile_id,instance_id,id,sha256,bytes) VALUES(?,?,?,?,?)").bind(profile_id).bind(instance_id).bind(&blob_id).bind(sha).bind(blob).execute(&mut *tx).await.map_err(storage)?;
            snapshot["engineVersion"]=json!(provider.engine_version());snapshot["stateBlobId"]=json!(blob_id);
        }
        if !schema::dto("InternalState",&snapshot){return Err("VALIDATION_ERROR".into())}
        sqlx::query("INSERT INTO experimental_internal_state_snapshots(profile_id,instance_id,id,revision,elapsed_ms,body) VALUES(?,?,?,?,?,?)").bind(profile_id).bind(instance_id).bind(&snapshot_id).bind(revision).bind(elapsed_ms).bind(snapshot.to_string()).execute(&mut *tx).await.map_err(storage)?;
        let mut events=Vec::with_capacity(update.events.len());
        for draft in update.events{
            let id=uuid::Uuid::new_v4().to_string();let body=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"snapshotId":snapshot_id,"revision":1,"kind":draft.kind,"description":draft.description,"occurredAt":ts(draft.occurred_at)});
            validate("NeutralStateEvent",&body)?;sqlx::query("INSERT INTO experimental_internal_state_events(profile_id,instance_id,snapshot_id,id,revision,body) VALUES(?,?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&snapshot_id).bind(&id).bind(body.to_string()).execute(&mut *tx).await.map_err(storage)?;events.push(body);
        }
        let mut candidates=Vec::with_capacity(update.wake_candidates.len());
        for draft in update.wake_candidates{
            let id=uuid::Uuid::new_v4().to_string();let body=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"snapshotId":snapshot_id,"revision":1,"reasonCode":draft.reason_code,"notBefore":ts(draft.not_before),"status":"active","createdAt":created});
            validate("InternalWakeCandidate",&body)?;sqlx::query("INSERT INTO experimental_wake_candidates(profile_id,instance_id,snapshot_id,id,revision,body) VALUES(?,?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&snapshot_id).bind(&id).bind(body.to_string()).execute(&mut *tx).await.map_err(storage)?;candidates.push(body);
        }
        tx.commit().await.map_err(storage)?;
        Ok(PersistedInternalState{snapshot,neutral_events:events,wake_candidates:candidates,elapsed_ms})
    }
}

pub(crate) async fn claim_context(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,now:DateTime<Utc>)->Result<(Option<Value>,Vec<Value>,Vec<Value>),String>{
    let snapshot_row=sqlx::query("SELECT id,body FROM experimental_internal_state_snapshots WHERE profile_id=? AND instance_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).fetch_optional(&mut **tx).await.map_err(storage)?;
    let Some(row)=snapshot_row else{return Ok((None,vec![],vec![]))};let snapshot_id=row.get::<String,_>("id");let snapshot:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;if !schema::dto("InternalState",&snapshot){return Err("STORAGE_ERROR".into())}
    let mut events=vec![];for r in sqlx::query("SELECT body FROM experimental_internal_state_events WHERE profile_id=? AND instance_id=? AND snapshot_id=? ORDER BY rowid").bind(profile_id).bind(instance_id).bind(&snapshot_id).fetch_all(&mut **tx).await.map_err(storage)?{let v:Value=serde_json::from_str(r.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("NeutralStateEvent",&v)?;events.push(v)}
    let mut candidates=vec![];for r in sqlx::query("SELECT body FROM experimental_wake_candidates WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' AND json_extract(body,'$.notBefore')<=? ORDER BY rowid").bind(profile_id).bind(instance_id).bind(ts(now)).fetch_all(&mut **tx).await.map_err(storage)?{let v:Value=serde_json::from_str(r.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("InternalWakeCandidate",&v)?;candidates.push(v)}
    Ok((Some(snapshot),events,candidates))
}

pub(crate) async fn consume_candidates(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,candidates:&[Value])->Result<(),String>{
    for c in candidates{let Some(id)=c.get("id").and_then(Value::as_str) else{continue};sqlx::query("UPDATE experimental_wake_candidates SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.status','consumed') WHERE profile_id=? AND instance_id=? AND id=? AND json_extract(body,'$.status')='active'").bind(profile_id).bind(instance_id).bind(id).execute(&mut **tx).await.map_err(storage)?;}
    Ok(())
}
fn validate(name:&str,v:&Value)->Result<(),String>{if schema::dto(name,v){Ok(())}else{Err("VALIDATION_ERROR".into())}}
fn storage(_:sqlx::Error)->String{"STORAGE_ERROR".into()}
fn parse(v:&str)->Result<DateTime<Utc>,String>{DateTime::parse_from_rfc3339(v).map(|x|x.with_timezone(&Utc)).map_err(|_|"STORAGE_ERROR".into())}
fn ts(v:DateTime<Utc>)->String{v.to_rfc3339_opts(chrono::SecondsFormat::Millis,true)}
