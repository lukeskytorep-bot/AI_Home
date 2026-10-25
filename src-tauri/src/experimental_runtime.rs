use chrono::{DateTime,Duration,Utc};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::{Row,SqlitePool};
use std::sync::Arc;
use crate::domain::schema;
use crate::experimental_continuity::{ResumePacket,WorkContinuityService};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct OpportunityEvent { pub sequence:i64, pub kind:String }

#[derive(Clone,Debug)]
pub struct OpportunityPacket {
    pub opportunity_id:String,
    pub profile_id:String,
    pub instance_id:String,
    pub runtime_generation:i64,
    pub now:DateTime<Utc>,
    pub wake_reason:String,
    pub events:Vec<OpportunityEvent>,
    pub active_refusals:Vec<Value>,
    pub resume_packet:Option<ResumePacket>,
}

#[derive(Clone,Debug)]
pub struct ActionIntent {
    pub connector:String,
    pub action_kind:String,
    pub sender_identity:String,
    pub destination:String,
    pub visibility:String,
    pub payload_ref:String,
    pub payload_bytes:Vec<u8>,
    pub idempotency_key:String,
    pub episode_id:String,
}

#[derive(Clone,Debug)]
pub enum DecisionChoice {
    Action(ActionIntent),
    NoAction,
    Rest,
    Sleep{delay_ms:i64,interrupt_events:Vec<String>},
    WaitEvent{interrupt_events:Vec<String>},
    Refusal{scope:String,reconsideration_conditions:Vec<String>},
    Defer{episode_id:String},
    Abandon{episode_id:String},
    Continue{episode_id:String},
    Revise{episode_id:String},
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ActionOutcome { Accepted{receipt_ref:String}, Failed, Uncertain }

pub trait Clock:Send+Sync { fn now(&self)->DateTime<Utc>; }
pub struct SystemClock;
impl Clock for SystemClock { fn now(&self)->DateTime<Utc>{Utc::now()} }

pub trait DecisionProvider:Send+Sync { fn decide(&self,packet:&OpportunityPacket)->Result<DecisionChoice,String>; }
pub trait ActionExecutor:Send+Sync { fn execute(&self,intent:&ActionIntent)->ActionOutcome; }

pub struct DisabledDecisionProvider;
impl DecisionProvider for DisabledDecisionProvider { fn decide(&self,_:&OpportunityPacket)->Result<DecisionChoice,String>{Err("RESOURCE_UNAVAILABLE".into())} }
pub struct DisabledActionExecutor;
impl ActionExecutor for DisabledActionExecutor { fn execute(&self,_:&ActionIntent)->ActionOutcome{ActionOutcome::Failed} }

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum RunResult { NotDue, Busy, Inactive, ResourceUnavailable, ReconciliationRequired, Completed{outcome:String,decision_id:String} }

pub struct ExperimentalRuntime<C:Clock,D:DecisionProvider,A:ActionExecutor>{pool:SqlitePool,clock:Arc<C>,decision:Arc<D>,actions:Arc<A>}
impl<C:Clock,D:DecisionProvider,A:ActionExecutor> ExperimentalRuntime<C,D,A>{
    pub fn new(pool:SqlitePool,clock:Arc<C>,decision:Arc<D>,actions:Arc<A>)->Self{Self{pool,clock,decision,actions}}

    pub async fn create_instance(&self,profile_id:&str,instance_id:&str,initial_interval_ms:i64)->Result<(),String>{
        if initial_interval_ms<=0{return Err("VALIDATION_ERROR".into())}
        let now=self.clock.now(); let stamp=ts(now);
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let exists:Option<i64>=sqlx::query_scalar("SELECT 1 FROM profiles WHERE id=?").bind(profile_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        if exists.is_some(){return Err("POLICY_DENIED".into())}
        let defaults:Value=serde_json::from_str(include_str!("../../contracts/defaults.json")).map_err(|_|"STORAGE_ERROR".to_string())?;
        // Frozen P2 still names the legacy non-standard discriminator `conscious`.
        // v1.4 product direction is bound authoritatively by experimental_instances.
        let profile=json!({"schemaVersion":1,"id":profile_id,"revision":1,"createdAt":stamp,"displayName":profile_id,"status":"draft","mode":"conscious","scribeMode":"automatic","memorySettings":defaults["memorySettings"],"contextBudget":defaults["contextBudget"],"languagePolicy":"preserve_source"});
        if !schema::dto("Profile",&profile){return Err("STORAGE_ERROR".into())}
        sqlx::query("INSERT INTO profiles(id,revision,body) VALUES(?,1,?)").bind(profile_id).bind(profile.to_string()).execute(&mut *tx).await.map_err(storage)?;
        let instance=json!({"schemaVersion":1,"id":instance_id,"profileId":profile_id,"direction":"experimental","revision":1,"runtimeGeneration":1,"status":"ready","eventCursor":0,"createdAt":stamp,"updatedAt":stamp});
        validate("ExperimentalInstance",&instance)?;
        sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES(?,?,1,?)").bind(profile_id).bind(instance_id).bind(instance.to_string()).execute(&mut *tx).await.map_err(storage)?;
        let wake_id=uuid::Uuid::new_v4().to_string();
        let wake=json!({"schemaVersion":1,"id":wake_id,"instanceId":instance_id,"revision":1,"origin":"initial_configuration","decisionId":"","status":"active","createdAt":stamp,"mode":"interval","requestedDelayMs":initial_interval_ms,"interruptEvents":[]});
        validate("WakePlan",&wake)?;
        sqlx::query("INSERT INTO experimental_wake_plans(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&wake_id).bind(wake.to_string()).execute(&mut *tx).await.map_err(storage)?;
        let cursor=json!({"schemaVersion":1,"id":uuid::Uuid::new_v4().to_string(),"instanceId":instance_id,"revision":1,"receivedThrough":0,"consideredThrough":0,"committedThrough":0,"updatedAt":stamp});
        validate("ExperimentalEventCursor",&cursor)?;
        sqlx::query("INSERT INTO experimental_event_cursors(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(cursor["id"].as_str().unwrap()).bind(cursor.to_string()).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?; Ok(())
    }

    pub async fn run_due(&self,profile_id:&str,instance_id:&str,events:&[OpportunityEvent])->Result<RunResult,String>{
        let now=self.clock.now();
        // A durable sent/uncertain ledger row is an execution fence. Even if a previous
        // post-effect storage failure prevented us from persisting recovery_required,
        // no new decision may run until the external outcome is reconciled.
        if self.has_unresolved_external_action(profile_id,instance_id).await? {
            // Best effort only: the durable ledger row itself is the execution fence.
            let _=self.mark_recovery_required_if_possible(profile_id,instance_id,None,now,"ACTION_RECONCILIATION_REQUIRED").await;
            return Ok(RunResult::ReconciliationRequired)
        }
        let claim=self.claim(profile_id,instance_id,now,events).await?;
        let Some((mut packet,generation))=claim else {
            let status:Option<String>=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_optional(&self.pool).await.map_err(storage)?;
            return Ok(match status.as_deref(){Some("running")=>RunResult::Busy,Some("inactive")=>RunResult::Inactive,Some("recovery_required")=>RunResult::ReconciliationRequired,_=>RunResult::NotDue})
        };
        match WorkContinuityService::new(self.pool.clone()).latest_resume_packet(profile_id,instance_id,now).await {
            Ok(resume)=>packet.resume_packet=resume,
            Err(code)=>{self.release_if_running_with_retry(profile_id,instance_id,generation,now,&code).await?;return Err(code)}
        }
        let choice=match self.decision.decide(&packet){
            Ok(v)=>v,
            Err(_)=>{
                self.release_with_retry(profile_id,instance_id,generation,now,"RESOURCE_UNAVAILABLE").await?;
                return Ok(RunResult::ResourceUnavailable)
            }
        };
        match self.persist_decision(profile_id,instance_id,generation,&packet,choice).await {
            Ok(v)=>Ok(v),
            Err(code)=>{
                // Once an ActionRecord reached sent, an external effect may have occurred.
                // A later storage/finalization error must never be converted into a normal
                // decision retry. The durable action ledger fences all future claims until
                // reconciliation, even if the recovery status update itself cannot commit.
                if self.has_unresolved_external_action(profile_id,instance_id).await? {
                    // Do not replace the original failure if the best-effort recovery marker
                    // cannot be stored. The sent/uncertain action row still fences retries.
                    let _=self.mark_recovery_required_if_possible(profile_id,instance_id,Some(generation),self.clock.now(),&code).await;
                    return Err(code)
                }
                // Purely pre-effect validation/storage failures can use the technical retry gate.
                self.release_if_running_with_retry(profile_id,instance_id,generation,self.clock.now(),&code).await?;
                Err(code)
            }
        }
    }

    async fn claim(&self,profile_id:&str,instance_id:&str,now:DateTime<Utc>,events:&[OpportunityEvent])->Result<Option<(OpportunityPacket,i64)>,String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let Some(row)=sqlx::query("SELECT revision,body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)? else{return Err("NOT_FOUND".into())};
        let mut instance:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?; validate("ExperimentalInstance",&instance)?;
        let status=instance["status"].as_str().unwrap(); if status=="running"||status=="inactive"||status=="recovery_required"{return Ok(None)}
        if let Some(retry)=instance.get("retryNotBefore").and_then(Value::as_str){if now<parse(retry)?{return Ok(None)}}
        let cursor=self.event_cursor(&mut tx,profile_id,instance_id).await?;
        let committed=cursor["committedThrough"].as_i64().unwrap_or(0);
        let fresh_events:Vec<OpportunityEvent>=events.iter().filter(|e|e.sequence>committed).cloned().collect();
        let max_event=events.iter().map(|e|e.sequence).max().unwrap_or(0);
        if max_event>0 { self.update_cursor_received(&mut tx,profile_id,instance_id,max_event,now).await?; }
        let active_wake=self.active_wake(&mut tx,profile_id,instance_id).await?;
        let reason=if status=="interrupted" {Some("resume_interrupted".to_string())} else {due_reason(active_wake.as_ref(),now,&fresh_events)?};
        let Some(wake_reason)=reason else{return Ok(None)};
        let revision=instance["revision"].as_i64().unwrap()+1; let generation=instance["runtimeGeneration"].as_i64().unwrap()+1;
        instance["revision"]=json!(revision);instance["runtimeGeneration"]=json!(generation);instance["status"]=json!("running");instance["updatedAt"]=json!(ts(now));validate("ExperimentalInstance",&instance)?;
        let changed=sqlx::query("UPDATE experimental_instances SET revision=?,body=? WHERE profile_id=? AND id=? AND revision=?").bind(revision).bind(instance.to_string()).bind(profile_id).bind(instance_id).bind(row.get::<i64,_>("revision")).execute(&mut *tx).await.map_err(storage)?;
        if changed.rows_affected()!=1{return Ok(None)}
        let refusals=self.active_refusals(&mut tx,profile_id,instance_id).await?;
        let packet=OpportunityPacket{opportunity_id:uuid::Uuid::new_v4().to_string(),profile_id:profile_id.into(),instance_id:instance_id.into(),runtime_generation:generation,now,wake_reason,events:fresh_events,active_refusals:refusals,resume_packet:None};
        tx.commit().await.map_err(storage)?; Ok(Some((packet,generation)))
    }

    async fn persist_decision(&self,profile_id:&str,instance_id:&str,generation:i64,packet:&OpportunityPacket,choice:DecisionChoice)->Result<RunResult,String>{
        let decision_id=uuid::Uuid::new_v4().to_string(); let now=packet.now; let created=ts(now);
        match choice {
            DecisionChoice::Action(intent)=>self.persist_action(profile_id,instance_id,generation,packet,&decision_id,intent).await,
            other=>{
                let mut tx=self.pool.begin().await.map_err(storage)?; self.require_generation(&mut tx,profile_id,instance_id,generation).await?;
                let (outcome,episode_id)=match &other{
                    DecisionChoice::NoAction=>("no_action","".to_string()),
                    DecisionChoice::Rest=>("rest","".to_string()),
                    DecisionChoice::Sleep{..}=>("sleep","".to_string()),
                    DecisionChoice::WaitEvent{..}=>("wait_event","".to_string()),
                    DecisionChoice::Refusal{..}=>("refusal","".to_string()),
                    DecisionChoice::Defer{episode_id}=>("defer",episode_id.clone()),
                    DecisionChoice::Abandon{episode_id}=>("abandon",episode_id.clone()),
                    DecisionChoice::Continue{episode_id}=>("continue",episode_id.clone()),
                    DecisionChoice::Revise{episode_id}=>("revise",episode_id.clone()),
                    DecisionChoice::Action(_)=>unreachable!(),
                };
                let default_delay=self.default_interval(&mut tx,profile_id,instance_id).await?;
                let mut wake_id=None; let mut refusal_id=None;
                match &other{
                    DecisionChoice::Sleep{delay_ms,interrupt_events}=>{if *delay_ms<=0{return Err("VALIDATION_ERROR".into())} let id=self.replace_wake(&mut tx,profile_id,instance_id,&decision_id,"deadline",Some(now+Duration::milliseconds(*delay_ms)),None,interrupt_events,now).await?;wake_id=Some(id);}
                    DecisionChoice::WaitEvent{interrupt_events}=>{if interrupt_events.is_empty(){return Err("VALIDATION_ERROR".into())} let id=self.replace_wake(&mut tx,profile_id,instance_id,&decision_id,"event_only",None,None,interrupt_events,now).await?;wake_id=Some(id);}
                    DecisionChoice::Refusal{scope,reconsideration_conditions}=>{let id=uuid::Uuid::new_v4().to_string();let fp=format!("{:x}",Sha256::digest(scope.as_bytes()));let r=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"revision":1,"decisionId":decision_id,"scopeFingerprint":fp,"status":"active","reconsiderationConditions":reconsideration_conditions,"sourceRefs":[],"createdAt":created});validate("RefusalRecord",&r)?;sqlx::query("INSERT INTO experimental_refusals(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&id).bind(r.to_string()).execute(&mut *tx).await.map_err(storage)?;refusal_id=Some(id);self.replace_wake(&mut tx,profile_id,instance_id,&decision_id,"interval",None,Some(default_delay),&[],now).await?;}
                    DecisionChoice::NoAction|DecisionChoice::Rest|DecisionChoice::Defer{..}|DecisionChoice::Abandon{..}|DecisionChoice::Continue{..}|DecisionChoice::Revise{..}=>{self.replace_wake(&mut tx,profile_id,instance_id,&decision_id,"interval",None,Some(default_delay),&[],now).await?;}
                    DecisionChoice::Action(_)=>unreachable!(),
                }
                self.update_episode_for_choice(&mut tx,profile_id,instance_id,&other,now).await?;
                let mut d=json!({"schemaVersion":1,"id":decision_id,"instanceId":instance_id,"revision":1,"opportunityId":packet.opportunity_id.clone(),"episodeId":episode_id,"contextRevision":0,"createdAt":created,"outcome":outcome});
                if let Some(id)=wake_id{d["wakePlanId"]=json!(id)} if let Some(id)=refusal_id{d["refusalId"]=json!(id)}
                validate("DecisionRecord",&d)?; sqlx::query("INSERT INTO experimental_decisions(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&decision_id).bind(d.to_string()).execute(&mut *tx).await.map_err(storage)?;
                self.finish_cycle(&mut tx,profile_id,instance_id,generation,packet,now).await?;tx.commit().await.map_err(storage)?;
                Ok(RunResult::Completed{outcome:outcome.into(),decision_id})
            }
        }
    }

    async fn persist_action(&self,profile_id:&str,instance_id:&str,generation:i64,packet:&OpportunityPacket,decision_id:&str,intent:ActionIntent)->Result<RunResult,String>{
        if !["public","private","scoped","internal"].contains(&intent.visibility.as_str()){return Err("VALIDATION_ERROR".into())}
        let now=packet.now;let action_id=uuid::Uuid::new_v4().to_string();let hash=format!("{:x}",Sha256::digest(&intent.payload_bytes));
        let mut tx=self.pool.begin().await.map_err(storage)?;self.require_generation(&mut tx,profile_id,instance_id,generation).await?;
        let d=json!({"schemaVersion":1,"id":decision_id,"instanceId":instance_id,"revision":1,"opportunityId":packet.opportunity_id.clone(),"episodeId":intent.episode_id.clone(),"contextRevision":0,"createdAt":ts(now),"outcome":"action","actionId":action_id.clone()});validate("DecisionRecord",&d)?;
        sqlx::query("INSERT INTO experimental_decisions(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(decision_id).bind(d.to_string()).execute(&mut *tx).await.map_err(storage)?;
        let action=json!({"schemaVersion":1,"id":action_id.clone(),"instanceId":instance_id,"revision":1,"decisionId":decision_id,"episodeId":intent.episode_id.clone(),"connector":intent.connector.clone(),"actionKind":intent.action_kind.clone(),"senderIdentity":intent.sender_identity.clone(),"destination":intent.destination.clone(),"visibility":intent.visibility.clone(),"payloadHash":hash,"payloadRef":intent.payload_ref.clone(),"idempotencyKey":intent.idempotency_key.clone(),"state":"sent","receiptRef":"","createdAt":ts(now),"updatedAt":ts(now)});validate("ActionRecord",&action)?;
        sqlx::query("INSERT INTO experimental_actions(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&action_id).bind(action.to_string()).execute(&mut *tx).await.map_err(storage)?;tx.commit().await.map_err(storage)?;
        let result=self.actions.execute(&intent);
        let mut tx=self.pool.begin().await.map_err(storage)?;self.require_generation(&mut tx,profile_id,instance_id,generation).await?;
        let row=sqlx::query("SELECT body FROM experimental_actions WHERE profile_id=? AND id=?").bind(profile_id).bind(&action_id).fetch_one(&mut *tx).await.map_err(storage)?;let mut action:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
        action["revision"]=json!(2);action["updatedAt"]=json!(ts(self.clock.now()));let mut requires_reconciliation=false;match result{ActionOutcome::Accepted{receipt_ref}=>{if receipt_ref.is_empty(){action["state"]=json!("uncertain");requires_reconciliation=true}else{action["state"]=json!("accepted");action["receiptRef"]=json!(receipt_ref)}},ActionOutcome::Failed=>action["state"]=json!("failed"),ActionOutcome::Uncertain=>{action["state"]=json!("uncertain");requires_reconciliation=true}};validate("ActionRecord",&action)?;
        let changed=sqlx::query("UPDATE experimental_actions SET revision=2,body=? WHERE profile_id=? AND id=? AND revision=1").bind(action.to_string()).bind(profile_id).bind(&action_id).execute(&mut *tx).await.map_err(storage)?;if changed.rows_affected()!=1{return Err("REVISION_CONFLICT".into())}
        if requires_reconciliation {
            self.park_recovery_required(&mut tx,profile_id,instance_id,generation,self.clock.now(),"ACTION_OUTCOME_UNCERTAIN").await?;
            tx.commit().await.map_err(storage)?;
            return Err("ACTION_OUTCOME_UNCERTAIN".into())
        }
        let default_delay=self.default_interval(&mut tx,profile_id,instance_id).await?;self.replace_wake(&mut tx,profile_id,instance_id,decision_id,"interval",None,Some(default_delay),&[],self.clock.now()).await?;
        self.finish_cycle(&mut tx,profile_id,instance_id,generation,packet,self.clock.now()).await?;tx.commit().await.map_err(storage)?;
        Ok(RunResult::Completed{outcome:"action".into(),decision_id:decision_id.into()})
    }


    async fn has_unresolved_external_action(&self,profile_id:&str,instance_id:&str)->Result<bool,String>{
        let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_actions WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.state') IN ('sent','uncertain')")
            .bind(profile_id).bind(instance_id).fetch_one(&self.pool).await.map_err(storage)?;
        Ok(n>0)
    }

    async fn mark_recovery_required_if_possible(&self,profile_id:&str,instance_id:&str,generation:Option<i64>,now:DateTime<Utc>,code:&str)->Result<(),String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let row=sqlx::query("SELECT revision,body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row)=row else{return Ok(())};
        let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
        if let Some(g)=generation { if v["runtimeGeneration"].as_i64()!=Some(g){return Ok(())} }
        if v["status"].as_str()==Some("inactive"){return Ok(())}
        v["revision"]=json!(row.get::<i64,_>("revision")+1);v["status"]=json!("recovery_required");v["lastTechnicalError"]=json!(code);v["updatedAt"]=json!(ts(now));validate("ExperimentalInstance",&v)?;
        sqlx::query("UPDATE experimental_instances SET revision=?,body=? WHERE profile_id=? AND id=? AND revision=?")
            .bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(instance_id).bind(row.get::<i64,_>("revision")).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;Ok(())
    }

    async fn park_recovery_required(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,generation:i64,now:DateTime<Utc>,code:&str)->Result<(),String>{
        self.require_generation(tx,profile_id,instance_id,generation).await?;
        let row=sqlx::query("SELECT revision,body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;
        let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
        v["revision"]=json!(row.get::<i64,_>("revision")+1);v["status"]=json!("recovery_required");v["lastTechnicalError"]=json!(code);if let Some(obj)=v.as_object_mut(){obj.remove("retryNotBefore");obj.remove("retryAttempt");}v["updatedAt"]=json!(ts(now));validate("ExperimentalInstance",&v)?;
        sqlx::query("UPDATE experimental_instances SET revision=?,body=? WHERE profile_id=? AND id=?")
            .bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(instance_id).execute(&mut **tx).await.map_err(storage)?;Ok(())
    }

    async fn release_with_retry(&self,profile_id:&str,instance_id:&str,generation:i64,now:DateTime<Utc>,code:&str)->Result<(),String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;self.require_generation(&mut tx,profile_id,instance_id,generation).await?;
        self.apply_retry_release(&mut tx,profile_id,instance_id,now,code).await?;tx.commit().await.map_err(storage)?;Ok(())
    }
    async fn release_if_running_with_retry(&self,profile_id:&str,instance_id:&str,generation:i64,now:DateTime<Utc>,code:&str)->Result<(),String>{
        let mut tx=self.pool.begin().await.map_err(storage)?;
        let row=sqlx::query("SELECT body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_optional(&mut *tx).await.map_err(storage)?;
        let Some(row)=row else{return Ok(())};let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
        if v["status"].as_str()!=Some("running")||v["runtimeGeneration"].as_i64()!=Some(generation){return Ok(())}
        self.apply_retry_release(&mut tx,profile_id,instance_id,now,code).await?;tx.commit().await.map_err(storage)?;Ok(())
    }
    async fn apply_retry_release(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,now:DateTime<Utc>,code:&str)->Result<(),String>{
        let row=sqlx::query("SELECT revision,body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;
        let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;
        let attempt=v.get("retryAttempt").and_then(Value::as_i64).unwrap_or(0)+1;
        let shift=((attempt-1).min(4)) as u32;let delay_ms=30_000_i64*(1_i64<<shift);
        v["revision"]=json!(v["revision"].as_i64().unwrap()+1);v["status"]=json!("ready");v["retryAttempt"]=json!(attempt);v["retryNotBefore"]=json!(ts(now+Duration::milliseconds(delay_ms)));v["lastTechnicalError"]=json!(code);v["updatedAt"]=json!(ts(now));validate("ExperimentalInstance",&v)?;
        sqlx::query("UPDATE experimental_instances SET revision=?,body=? WHERE profile_id=? AND id=?").bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(instance_id).execute(&mut **tx).await.map_err(storage)?;Ok(())
    }

    async fn active_wake(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str)->Result<Option<Value>,String>{let row=sqlx::query("SELECT body FROM experimental_wake_plans WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid DESC LIMIT 1").bind(profile_id).bind(instance_id).fetch_optional(&mut **tx).await.map_err(storage)?;row.map(|r|serde_json::from_str(r.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())).transpose()}
    async fn event_cursor(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str)->Result<Value,String>{let row=sqlx::query("SELECT body FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("ExperimentalEventCursor",&v)?;Ok(v)}
    async fn active_refusals(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str)->Result<Vec<Value>,String>{let rows=sqlx::query("SELECT body FROM experimental_refusals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid").bind(profile_id).bind(instance_id).fetch_all(&mut **tx).await.map_err(storage)?;rows.into_iter().map(|r|{let v:Value=serde_json::from_str(r.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;validate("RefusalRecord",&v)?;Ok(v)}).collect()}
    async fn default_interval(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str)->Result<i64,String>{sqlx::query_scalar("SELECT json_extract(body,'$.requestedDelayMs') FROM experimental_wake_plans WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.origin')='initial_configuration' AND json_extract(body,'$.mode')='interval' ORDER BY rowid LIMIT 1").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)}
    async fn replace_wake(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,decision_id:&str,mode:&str,deadline:Option<DateTime<Utc>>,delay:Option<i64>,interrupts:&[String],now:DateTime<Utc>)->Result<String,String>{sqlx::query("UPDATE experimental_wake_plans SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.status','superseded') WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active'").bind(profile_id).bind(instance_id).execute(&mut **tx).await.map_err(storage)?;let id=uuid::Uuid::new_v4().to_string();let mut w=json!({"schemaVersion":1,"id":id,"instanceId":instance_id,"revision":1,"origin":"ai_decision","decisionId":decision_id,"status":"active","createdAt":ts(now),"mode":mode,"interruptEvents":interrupts});match mode{"deadline"=>w["nextWakeAt"]=json!(ts(deadline.ok_or("VALIDATION_ERROR")?)),"interval"=>w["requestedDelayMs"]=json!(delay.ok_or("VALIDATION_ERROR")?),"event_only"=>{},_=>return Err("VALIDATION_ERROR".into())};validate("WakePlan",&w)?;sqlx::query("INSERT INTO experimental_wake_plans(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind(profile_id).bind(instance_id).bind(&id).bind(w.to_string()).execute(&mut **tx).await.map_err(storage)?;Ok(id)}
    async fn require_generation(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,generation:i64)->Result<(),String>{let row=sqlx::query("SELECT body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;let v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;if v["status"].as_str()!=Some("running")||v["runtimeGeneration"].as_i64()!=Some(generation){return Err("STALE_EXECUTOR".into())}Ok(())}
    async fn update_cursor_received(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,seq:i64,now:DateTime<Utc>)->Result<(),String>{let row=sqlx::query("SELECT id,revision,body FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;if seq>v["receivedThrough"].as_i64().unwrap(){v["receivedThrough"]=json!(seq);v["revision"]=json!(row.get::<i64,_>("revision")+1);v["updatedAt"]=json!(ts(now));validate("ExperimentalEventCursor",&v)?;sqlx::query("UPDATE experimental_event_cursors SET revision=?,body=? WHERE profile_id=? AND id=?").bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(row.get::<&str,_>("id")).execute(&mut **tx).await.map_err(storage)?;}Ok(())}
    async fn finish_cycle(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,generation:i64,packet:&OpportunityPacket,now:DateTime<Utc>)->Result<(),String>{self.require_generation(tx,profile_id,instance_id,generation).await?;let seq=packet.events.iter().map(|e|e.sequence).max().unwrap_or(0);if seq>0{let row=sqlx::query("SELECT id,revision,body FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;let mut c:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;c["consideredThrough"]=json!(c["consideredThrough"].as_i64().unwrap().max(seq));c["committedThrough"]=json!(c["committedThrough"].as_i64().unwrap().max(seq));c["revision"]=json!(row.get::<i64,_>("revision")+1);c["updatedAt"]=json!(ts(now));validate("ExperimentalEventCursor",&c)?;sqlx::query("UPDATE experimental_event_cursors SET revision=?,body=? WHERE profile_id=? AND id=?").bind(c["revision"].as_i64().unwrap()).bind(c.to_string()).bind(profile_id).bind(row.get::<&str,_>("id")).execute(&mut **tx).await.map_err(storage)?;}
        let row=sqlx::query("SELECT revision,body FROM experimental_instances WHERE profile_id=? AND id=?").bind(profile_id).bind(instance_id).fetch_one(&mut **tx).await.map_err(storage)?;let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;v["revision"]=json!(row.get::<i64,_>("revision")+1);v["status"]=json!("ready");v["eventCursor"]=json!(v["eventCursor"].as_i64().unwrap().max(seq));if let Some(obj)=v.as_object_mut(){obj.remove("retryNotBefore");obj.remove("retryAttempt");obj.remove("lastTechnicalError");}v["updatedAt"]=json!(ts(now));validate("ExperimentalInstance",&v)?;sqlx::query("UPDATE experimental_instances SET revision=?,body=? WHERE profile_id=? AND id=?").bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(instance_id).execute(&mut **tx).await.map_err(storage)?;Ok(())}
    async fn update_episode_for_choice(&self,tx:&mut sqlx::Transaction<'_,sqlx::Sqlite>,profile_id:&str,instance_id:&str,choice:&DecisionChoice,now:DateTime<Utc>)->Result<(),String>{let (id,status)=match choice{DecisionChoice::Defer{episode_id}=>(episode_id,"deferred"),DecisionChoice::Abandon{episode_id}=>(episode_id,"abandoned"),DecisionChoice::Continue{episode_id}=>(episode_id,"active"),DecisionChoice::Revise{episode_id}=>(episode_id,"active"),_=>return Ok(())};let row=sqlx::query("SELECT revision,body FROM experimental_work_episodes WHERE profile_id=? AND instance_id=? AND id=?").bind(profile_id).bind(instance_id).bind(id).fetch_one(&mut **tx).await.map_err(storage)?;let mut v:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR".to_string())?;v["revision"]=json!(row.get::<i64,_>("revision")+1);v["status"]=json!(status);v["updatedAt"]=json!(ts(now));validate("WorkEpisode",&v)?;sqlx::query("UPDATE experimental_work_episodes SET revision=?,body=? WHERE profile_id=? AND id=?").bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(profile_id).bind(id).execute(&mut **tx).await.map_err(storage)?;Ok(())}
}

fn due_reason(wake:Option<&Value>,now:DateTime<Utc>,events:&[OpportunityEvent])->Result<Option<String>,String>{let Some(w)=wake else{return Ok(None)};validate("WakePlan",w)?;let interrupts=w["interruptEvents"].as_array().unwrap();if !interrupts.is_empty()&&events.iter().any(|e|interrupts.iter().any(|x|x.as_str()==Some(&e.kind))){return Ok(Some("selected_event".into()))}match w["mode"].as_str().unwrap(){"interval"=>{let created=parse(w["createdAt"].as_str().unwrap())?;let delay=w["requestedDelayMs"].as_i64().unwrap();Ok((now>=created+Duration::milliseconds(delay)).then(||"scheduled_interval".into()))},"deadline"=>{let deadline=parse(w["nextWakeAt"].as_str().unwrap())?;Ok((now>=deadline).then(||"scheduled_deadline".into()))},"event_only"=>Ok(None),_=>Err("STORAGE_ERROR".into())}}
fn parse(v:&str)->Result<DateTime<Utc>,String>{DateTime::parse_from_rfc3339(v).map(|x|x.with_timezone(&Utc)).map_err(|_|"STORAGE_ERROR".into())}
fn ts(v:DateTime<Utc>)->String{v.to_rfc3339_opts(chrono::SecondsFormat::Millis,true)}
fn validate(name:&str,v:&Value)->Result<(),String>{if schema::dto(name,v){Ok(())}else{Err("VALIDATION_ERROR".into())}}
fn storage<E:std::fmt::Display>(_e:E)->String{"STORAGE_ERROR".into()}
