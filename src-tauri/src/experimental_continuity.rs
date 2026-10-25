use chrono::{DateTime,Utc};
use serde_json::{json,Value};
use sqlx::{Row,SqlitePool};
use crate::domain::schema;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ContextComponentEstimate {
    pub label:String,
    pub estimated_tokens:u64,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ContextWindowBudget {
    pub context_tokens:u64,
    pub reserved_output_tokens:u64,
    pub input_reserve_tokens:u64,
    pub checkpoint_reserve_tokens:u64,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ContextPreflightStatus { Fits, CheckpointRequired, Impossible }

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ContextPreflightReport {
    pub measurement:String,
    pub estimated_input_tokens:u64,
    pub available_input_tokens:u64,
    pub remaining_after_request_tokens:u64,
    pub checkpoint_reserve_tokens:u64,
    pub status:ContextPreflightStatus,
}

pub fn preflight_context(budget:&ContextWindowBudget,components:&[ContextComponentEstimate])->Result<ContextPreflightReport,String>{
    if budget.context_tokens==0||budget.reserved_output_tokens==0||budget.input_reserve_tokens==0||budget.checkpoint_reserve_tokens==0{return Err("VALIDATION_ERROR".into())}
    let unavailable=budget.reserved_output_tokens.checked_add(budget.input_reserve_tokens).ok_or("CONTEXT_BUDGET_EXCEEDED")?;
    let available=budget.context_tokens.checked_sub(unavailable).ok_or("CONTEXT_BUDGET_EXCEEDED")?;
    let estimated=components.iter().try_fold(0u64,|acc,c|{
        if c.label.trim().is_empty(){return Err("VALIDATION_ERROR".to_string())}
        acc.checked_add(c.estimated_tokens).ok_or_else(||"CONTEXT_BUDGET_EXCEEDED".to_string())
    })?;
    let remaining=available.saturating_sub(estimated);
    let status=if estimated>available{ContextPreflightStatus::Impossible}else if remaining<budget.checkpoint_reserve_tokens{ContextPreflightStatus::CheckpointRequired}else{ContextPreflightStatus::Fits};
    Ok(ContextPreflightReport{measurement:"caller_estimated_tokens".into(),estimated_input_tokens:estimated,available_input_tokens:available,remaining_after_request_tokens:remaining,checkpoint_reserve_tokens:budget.checkpoint_reserve_tokens,status})
}

#[derive(Clone,Debug,PartialEq)]
pub struct ResumePacket {
    pub profile_id:String,
    pub instance_id:String,
    pub episode:Value,
    pub technical_checkpoint:Value,
    pub semantic_checkpoint:Option<Value>,
    pub active_refusals:Vec<Value>,
    pub unresolved_actions:Vec<Value>,
    pub committed_event_sequence:i64,
    pub built_at:String,
}

pub struct WorkContinuityService { pool:SqlitePool }
impl WorkContinuityService {
    pub fn new(pool:SqlitePool)->Self{Self{pool}}

    pub async fn create_episode(&self,profile_id:&str,instance_id:&str,title:&str,origin_decision_id:&str,now:DateTime<Utc>)->Result<Value,String>{
        if title.trim().is_empty(){return Err("VALIDATION_ERROR".into())}
        let mut tx=self.pool.begin().await.map_err(storage)?;
        require_instance(&mut tx,profile_id,instance_id).await?;
        let id=uuid::Uuid::new_v4().to_string();
        let body=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"revision":1,"status":"active","title":title,"originDecisionId":origin_decision_id,"createdAt":ts(now),"updatedAt":ts(now)});
        validate("WorkEpisode",&body)?;
        sqlx::query("INSERT INTO experimental_work_episodes(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&id).bind(body.to_string()).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;Ok(body)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn write_technical_checkpoint(&self,profile_id:&str,instance_id:&str,episode_id:&str,expected_parent:&str,status:&str,source_event_sequence:i64,decision_refs:&[String],action_refs:&[String],source_refs:&[Value],now:DateTime<Utc>)->Result<Value,String>{
        if source_event_sequence<0{return Err("VALIDATION_ERROR".into())}
        let mut tx=self.pool.begin().await.map_err(storage)?;
        require_episode(&mut tx,profile_id,instance_id,episode_id).await?;
        let actual_parent:Option<(String,i64)>=sqlx::query_as("SELECT id,CAST(json_extract(body,'$.sourceEventSequence') AS INTEGER) FROM experimental_technical_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).bind(episode_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if actual_parent.as_ref().map(|(id,_)|id.as_str()).unwrap_or("")!=expected_parent{return Err("REVISION_CONFLICT".into())}
        if let Some((_,parent_sequence))=actual_parent.as_ref(){if source_event_sequence<*parent_sequence{return Err("REVISION_CONFLICT".into())}}
        let cursor:Option<i64>=sqlx::query_scalar("SELECT json_extract(body,'$.committedThrough') FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if source_event_sequence>cursor.unwrap_or(0){return Err("REVISION_CONFLICT".into())}
        let id=uuid::Uuid::new_v4().to_string();
        let body=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"episodeId":episode_id,"revision":1,"parentCheckpointId":expected_parent,"sourceEventSequence":source_event_sequence,"status":status,"decisionRefs":decision_refs,"actionRefs":action_refs,"sourceRefs":source_refs,"createdAt":ts(now)});
        validate("TechnicalWorkCheckpoint",&body)?;
        sqlx::query("INSERT INTO experimental_technical_checkpoints(profile_id,instance_id,episode_id,id,revision,body) VALUES(?,?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(episode_id).bind(&id).bind(body.to_string()).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;Ok(body)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn write_semantic_checkpoint(&self,profile_id:&str,instance_id:&str,episode_id:&str,technical_checkpoint_id:&str,expected_parent:&str,proposer:&str,disposition:&str,content:Value,source_refs:&[Value],now:DateTime<Utc>)->Result<Value,String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        require_episode(&mut tx,profile_id,instance_id,episode_id).await?;
        let tech:Option<(String,i64)>=sqlx::query_as("SELECT id,CAST(json_extract(body,'$.sourceEventSequence') AS INTEGER) FROM experimental_technical_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? AND id=?").bind(profile_id).bind(instance_id).bind(episode_id).bind(technical_checkpoint_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some((_,source_seq))=tech else{return Err("NOT_FOUND".into())};
        let actual_parent:Option<String>=sqlx::query_scalar("SELECT id FROM experimental_semantic_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).bind(episode_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if actual_parent.as_deref().unwrap_or("")!=expected_parent{return Err("REVISION_CONFLICT".into())}
        let latest_tech:Option<String>=sqlx::query_scalar("SELECT id FROM experimental_technical_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).bind(episode_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if latest_tech.as_deref()!=Some(technical_checkpoint_id){return Err("REVISION_CONFLICT".into())}
        let id=uuid::Uuid::new_v4().to_string();
        let body=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"episodeId":episode_id,"revision":1,"parentCheckpointId":expected_parent,"technicalCheckpointId":technical_checkpoint_id,"proposer":proposer,"primaryDisposition":disposition,"sourceEventSequence":source_seq,"sourceRefs":source_refs,"content":content,"createdAt":ts(now)});
        validate("SemanticWorkCheckpoint",&body)?;
        sqlx::query("INSERT INTO experimental_semantic_checkpoints(profile_id,instance_id,episode_id,technical_checkpoint_id,id,revision,body) VALUES(?,?,?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(episode_id).bind(technical_checkpoint_id).bind(&id).bind(body.to_string()).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;Ok(body)
    }

    pub async fn latest_resume_packet(&self,profile_id:&str,instance_id:&str,now:DateTime<Utc>)->Result<Option<ResumePacket>,String>{
        let episode:Option<String>=sqlx::query_scalar("SELECT e.id FROM experimental_work_episodes e WHERE e.profile_id=? AND e.instance_id=? AND json_extract(e.body,'$.status') IN ('active','waiting','interrupted') AND EXISTS (SELECT 1 FROM experimental_technical_checkpoints t WHERE t.profile_id=e.profile_id AND t.instance_id=e.instance_id AND t.episode_id=e.id) ORDER BY e.rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).fetch_optional(&self.pool).await.map_err(storage)?;
        let Some(episode_id)=episode else{return Ok(None)};
        self.build_resume_packet(profile_id,instance_id,&episode_id,now).await.map(Some)
    }

    pub async fn build_resume_packet(&self,profile_id:&str,instance_id:&str,episode_id:&str,now:DateTime<Utc>)->Result<ResumePacket,String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let episode=require_episode(&mut tx,profile_id,instance_id,episode_id).await?;
        let tech_row=sqlx::query("SELECT body FROM experimental_technical_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).bind(episode_id).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or("NOT_FOUND")?;
        let technical:Value=serde_json::from_str(tech_row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("TechnicalWorkCheckpoint",&technical)?;
        let semantic=if let Some(row)=sqlx::query("SELECT body FROM experimental_semantic_checkpoints WHERE profile_id=? AND instance_id=? AND episode_id=? AND CAST(json_extract(body,'$.sourceEventSequence') AS INTEGER)<=? ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).bind(episode_id).bind(technical["sourceEventSequence"].as_i64().unwrap_or(0)).fetch_optional(&mut *tx).await.map_err(storage)?{let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("SemanticWorkCheckpoint",&v)?;Some(v)}else{None};
        let active_refusals=load_values(&mut tx,"SELECT body FROM experimental_refusals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid",profile_id,instance_id,"RefusalRecord").await?;
        let unresolved_actions=load_values(&mut tx,"SELECT body FROM experimental_actions WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.state') IN ('sent','uncertain') ORDER BY rowid",profile_id,instance_id,"ActionRecord").await?;
        let committed:i64=sqlx::query_scalar("SELECT CAST(json_extract(body,'$.committedThrough') AS INTEGER) FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?").bind(profile_id).bind(instance_id).fetch_one(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(ResumePacket{profile_id:profile_id.into(),instance_id:instance_id.into(),episode,technical_checkpoint:technical,semantic_checkpoint:semantic,active_refusals,unresolved_actions,committed_event_sequence:committed,built_at:ts(now)})
    }
}

async fn require_instance(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile:&str,instance:&str)->Result<Value,String>{
    let row=sqlx::query("SELECT body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile).bind(instance).fetch_optional(&mut **tx).await.map_err(storage)?.ok_or("NOT_FOUND")?;
    let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("ExperimentalInstance",&v)?;Ok(v)
}
async fn require_episode(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile:&str,instance:&str,episode:&str)->Result<Value,String>{
    let row=sqlx::query("SELECT body FROM experimental_work_episodes WHERE profile_id=? AND instance_id=? AND id=?").bind(profile).bind(instance).bind(episode).fetch_optional(&mut **tx).await.map_err(storage)?.ok_or("NOT_FOUND")?;
    let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("WorkEpisode",&v)?;Ok(v)
}
async fn load_values(tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,sql:&str,profile:&str,instance:&str,dto:&str)->Result<Vec<Value>,String>{
    let rows=sqlx::query(sql).bind(profile).bind(instance).fetch_all(&mut **tx).await.map_err(storage)?;
    rows.into_iter().map(|r|{let v:Value=serde_json::from_str(r.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate(dto,&v)?;Ok(v)}).collect()
}
fn validate(name:&str,v:&Value)->Result<(),String>{if schema::dto(name,v){Ok(())}else{Err("VALIDATION_ERROR".into())}}
fn storage(_:sqlx::Error)->String{"STORAGE_ERROR".into()}
fn ts(v:DateTime<Utc>)->String{v.to_rfc3339_opts(chrono::SecondsFormat::Millis,true)}
