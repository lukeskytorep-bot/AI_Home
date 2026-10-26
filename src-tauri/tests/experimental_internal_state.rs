use ai_home::{experimental_internal_state::{ConsciousCoreAdapter,ConsciousCoreEngine,InternalStateInput,InternalStateProvider,InternalStateService,InternalStateUpdate,NeutralEventDraft,NullStateProvider,WakeCandidateDraft},experimental_runtime::{ActionExecutor,ActionIntent,ActionOutcome,Clock,DecisionChoice,DecisionProvider,ExperimentalRuntime,OpportunityPacket,RunResult},persistence::Store};
use chrono::{DateTime,Duration,Utc};
use std::sync::{Arc,Mutex};

struct FakeClock(Mutex<DateTime<Utc>>);impl FakeClock{fn at(v:&str)->Self{Self(Mutex::new(DateTime::parse_from_rfc3339(v).unwrap().with_timezone(&Utc)))}fn advance(&self,ms:i64){let mut v=self.0.lock().unwrap();*v=v.clone()+Duration::milliseconds(ms)}}impl Clock for FakeClock{fn now(&self)->DateTime<Utc>{self.0.lock().unwrap().clone()}}
struct NoopAction;impl ActionExecutor for NoopAction{fn execute(&self,_:&ActionIntent)->ActionOutcome{ActionOutcome::Failed}}
struct Capture{packets:Mutex<Vec<OpportunityPacket>>}impl DecisionProvider for Capture{fn decide(&self,p:&OpportunityPacket)->Result<DecisionChoice,String>{self.packets.lock().unwrap().push(p.clone());Ok(DecisionChoice::NoAction)}}
struct Engine{calls:Arc<Mutex<Vec<i64>>>}impl ConsciousCoreEngine for Engine{fn engine_version(&self)->&str{"cc-test/1"}fn transition(&self,input:&InternalStateInput)->Result<InternalStateUpdate,String>{self.calls.lock().unwrap().push(input.elapsed_ms);Ok(InternalStateUpdate{state_blob:Some(format!("state:{}",input.previous_revision+1).into_bytes()),events:vec![NeutralEventDraft{kind:"mood.shift".into(),description:"neutral internal signal".into(),occurred_at:input.now}],wake_candidates:vec![WakeCandidateDraft{reason_code:"reflection_ready".into(),not_before:input.now}]})}}

#[tokio::test]async fn null_state_is_replaceable_and_creates_no_events_or_wake_candidates(){let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-25T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),Arc::new(Capture{packets:Mutex::new(vec![])}),Arc::new(NoopAction));runtime.create_instance("p","exp",60_000).await.unwrap();let state=InternalStateService::new(store.pool.clone()).refresh("p","exp",&NullStateProvider,clock.now()).await.unwrap();assert_eq!(state.snapshot["kind"],"null");assert!(state.neutral_events.is_empty());assert!(state.wake_candidates.is_empty());}

#[tokio::test]async fn conscious_core_adapter_persists_versioned_snapshot_neutral_event_and_candidate(){let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-25T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),Arc::new(Capture{packets:Mutex::new(vec![])}),Arc::new(NoopAction));runtime.create_instance("p","exp",60_000).await.unwrap();let provider=ConsciousCoreAdapter::new(Engine{calls:Arc::new(Mutex::new(vec![]))});let state=InternalStateService::new(store.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();assert_eq!(state.snapshot["kind"],"conscious_core");assert_eq!(state.snapshot["engineVersion"],"cc-test/1");assert_eq!(state.neutral_events.len(),1);assert_eq!(state.wake_candidates.len(),1);let blob_id=state.snapshot["stateBlobId"].as_str().unwrap();let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_internal_state_blobs WHERE profile_id='p' AND id=?").bind(blob_id).fetch_one(&store.pool).await.unwrap();assert_eq!(n,1);}

#[tokio::test]async fn restart_applies_one_elapsed_transition_instead_of_replaying_ticks(){let path=std::env::temp_dir().join(format!("ai-home-state-{}.db",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-25T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),Arc::new(Capture{packets:Mutex::new(vec![])}),Arc::new(NoopAction));runtime.create_instance("p","exp",60_000).await.unwrap();let calls=Arc::new(Mutex::new(vec![]));let engine=Engine{calls:calls.clone()};let provider=ConsciousCoreAdapter::new(engine);InternalStateService::new(store.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();let pool=store.pool.clone();drop(runtime);drop(store);pool.close().await;clock.advance(6*60*60*1000);let reopened=Store::open(&path).await.unwrap();let state=InternalStateService::new(reopened.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();assert_eq!(state.elapsed_ms,6*60*60*1000);assert_eq!(calls.lock().unwrap().len(),2);let pool=reopened.pool.clone();drop(reopened);pool.close().await;let _=std::fs::remove_file(path);}

#[tokio::test]async fn internal_wake_candidate_can_create_opportunity_but_not_override_unselected_ai_sleep(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-25T12:00:00Z"));
 let capture=Arc::new(Capture{packets:Mutex::new(vec![])});let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),capture.clone(),Arc::new(NoopAction));runtime.create_instance("p","exp",60_000).await.unwrap();
 let provider=ConsciousCoreAdapter::new(Engine{calls:Arc::new(Mutex::new(vec![]))});InternalStateService::new(store.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{..}));assert!(capture.packets.lock().unwrap()[0].wake_reason.starts_with("internal_state:"));
 // Replace the current plan with an explicit AI sleep that does not allow internal-state interruption.
 sqlx::query("UPDATE experimental_wake_plans SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.status','superseded') WHERE profile_id='p' AND json_extract(body,'$.status')='active'").execute(&store.pool).await.unwrap();
 let sleep=serde_json::json!({"schemaVersion":1,"id":"sleep","instanceId":"exp","revision":1,"origin":"ai_decision","decisionId":"d-sleep","status":"active","createdAt":"2026-10-25T12:00:00.000Z","mode":"deadline","nextWakeAt":"2026-10-25T18:00:00.000Z","interruptEvents":["message:orion"]});
 sqlx::query("INSERT INTO experimental_wake_plans(profile_id,instance_id,id,revision,body) VALUES('p','exp','sleep',1,?)").bind(sleep.to_string()).execute(&store.pool).await.unwrap();
 clock.advance(1_000);InternalStateService::new(store.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);assert_eq!(capture.packets.lock().unwrap().len(),1);
}

struct WakeEngine{reasons:Vec<&'static str>}
impl ConsciousCoreEngine for WakeEngine{
 fn engine_version(&self)->&str{"cc-wake-test/1"}
 fn transition(&self,input:&InternalStateInput)->Result<InternalStateUpdate,String>{
  Ok(InternalStateUpdate{
   state_blob:Some(b"wake-test-state".to_vec()),
   events:vec![],
   wake_candidates:self.reasons.iter().map(|reason|WakeCandidateDraft{reason_code:(*reason).into(),not_before:input.now}).collect(),
  })
 }
}

async fn install_ai_wait_plan(store:&Store,mode:&str,interrupts:&[&str]){
 sqlx::query("UPDATE experimental_wake_plans SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.status','superseded') WHERE profile_id='p' AND instance_id='exp' AND json_extract(body,'$.status')='active'").execute(&store.pool).await.unwrap();
 let mut wake=serde_json::json!({"schemaVersion":1,"id":format!("wake-{mode}"),"instanceId":"exp","revision":1,"origin":"ai_decision","decisionId":"d-wake","status":"active","createdAt":"2026-10-25T12:00:00.000Z","mode":mode,"interruptEvents":interrupts});
 if mode=="deadline"{wake["nextWakeAt"]=serde_json::json!("2026-10-25T18:00:00.000Z");}
 sqlx::query("INSERT INTO experimental_wake_plans(profile_id,instance_id,id,revision,body) VALUES('p','exp',?,1,?)").bind(wake["id"].as_str().unwrap()).bind(wake.to_string()).execute(&store.pool).await.unwrap();
}

async fn assert_internal_candidate_selection(mode:&str,reasons:Vec<&'static str>,interrupts:&[&str],expected:Option<&str>){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-25T12:00:00Z"));
 let capture=Arc::new(Capture{packets:Mutex::new(vec![])});
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),capture.clone(),Arc::new(NoopAction));
 runtime.create_instance("p","exp",60_000).await.unwrap();
 install_ai_wait_plan(&store,mode,interrupts).await;
 let provider=ConsciousCoreAdapter::new(WakeEngine{reasons});
 InternalStateService::new(store.pool.clone()).refresh("p","exp",&provider,clock.now()).await.unwrap();
 let result=runtime.run_due("p","exp",&[]).await.unwrap();
 match expected{
  Some(reason)=>{
   assert!(matches!(result,RunResult::Completed{..}));
   let packets=capture.packets.lock().unwrap();
   assert_eq!(packets.len(),1);
   assert_eq!(packets[0].wake_reason,reason);
  }
  None=>{
   assert_eq!(result,RunResult::NotDue);
   assert_eq!(capture.packets.lock().unwrap().len(),0);
  }
 }
}

#[tokio::test]
async fn internal_wake_selects_first_candidate_allowed_by_ai_not_first_candidate_in_storage(){
 for mode in ["deadline","event_only"]{
  assert_internal_candidate_selection(mode,vec!["A","B"],&["internal_state:B"],Some("internal_state:B")).await;
  assert_internal_candidate_selection(mode,vec!["B","A"],&["internal_state:B"],Some("internal_state:B")).await;
  assert_internal_candidate_selection(mode,vec!["A","B"],&["internal_state:C"],None).await;
 }
}
