use ai_home::{experimental_runtime::{ActionExecutor,ActionIntent,ActionOutcome,Clock,DecisionChoice,DecisionProvider,ExperimentalRuntime,OpportunityEvent,OpportunityPacket,RunResult},persistence::Store};
use chrono::{DateTime,Duration,Utc};
use serde_json::{json,Value};
use sqlx::Row;
use std::{collections::VecDeque,path::PathBuf,sync::{Arc,Mutex}};

struct FakeClock(Mutex<DateTime<Utc>>);
impl FakeClock { fn at(s:&str)->Self{Self(Mutex::new(DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)))} fn advance_ms(&self,ms:i64){let mut n=self.0.lock().unwrap();*n=n.clone()+Duration::milliseconds(ms);} }
impl Clock for FakeClock{fn now(&self)->DateTime<Utc>{self.0.lock().unwrap().clone()}}

struct ScriptedDecision{choices:Mutex<VecDeque<DecisionChoice>>,packets:Mutex<Vec<OpportunityPacket>>}
impl ScriptedDecision{fn new(choices:Vec<DecisionChoice>)->Self{Self{choices:Mutex::new(choices.into()),packets:Mutex::new(vec![])}}fn calls(&self)->usize{self.packets.lock().unwrap().len()}fn packets(&self)->Vec<OpportunityPacket>{self.packets.lock().unwrap().clone()}}
impl DecisionProvider for ScriptedDecision{fn decide(&self,p:&OpportunityPacket)->Result<DecisionChoice,String>{self.packets.lock().unwrap().push(p.clone());self.choices.lock().unwrap().pop_front().ok_or_else(||"no scripted decision".into())}}

struct FakePeer{calls:Mutex<Vec<ActionIntent>>,outcome:Mutex<VecDeque<ActionOutcome>>}
impl FakePeer{fn accepted()->Self{Self{calls:Mutex::new(vec![]),outcome:Mutex::new(vec![ActionOutcome::Accepted{receipt_ref:"peer-receipt-1".into()}].into())}}}
impl ActionExecutor for FakePeer{fn execute(&self,a:&ActionIntent)->ActionOutcome{self.calls.lock().unwrap().push(a.clone());self.outcome.lock().unwrap().pop_front().unwrap_or(ActionOutcome::Failed)}}

async fn profile(store:&Store,id:&str){let defaults:Value=serde_json::from_str(include_str!("../../contracts/defaults.json")).unwrap();let body=json!({"schemaVersion":1,"id":id,"revision":1,"createdAt":"2026-10-24T12:00:00Z","displayName":id,"status":"draft","mode":"standard","scribeMode":"automatic","memorySettings":defaults["memorySettings"],"contextBudget":defaults["contextBudget"],"languagePolicy":"preserve_source"});sqlx::query("INSERT INTO profiles(id,revision,body) VALUES(?,1,?)").bind(id).bind(body.to_string()).execute(&store.pool).await.unwrap();}
fn disabled_action()->Arc<FakePeer>{Arc::new(FakePeer{calls:Mutex::new(vec![]),outcome:Mutex::new(VecDeque::new())})}

#[tokio::test]
async fn interval_is_an_opportunity_not_a_periodic_model_call(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::NoAction]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",120_000).await.unwrap();
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),0);
 clock.advance_ms(119_999);assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),0);
 clock.advance_ms(1);assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="no_action"));assert_eq!(decisions.calls(),1);
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),1);
}

#[tokio::test]
async fn ai_selected_sleep_is_not_overridden_by_unselected_human_event(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Sleep{delay_ms:6*60*60*1000,interrupt_events:vec!["message:orion".into()]},DecisionChoice::Rest]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",60_000).await.unwrap();clock.advance_ms(60_000);
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="sleep"));assert_eq!(decisions.calls(),1);
 clock.advance_ms(2*60*60*1000);let urgent=[OpportunityEvent{sequence:1,kind:"human:urgent".into()}];assert_eq!(runtime.run_due("p","exp",&urgent).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),1);
 let selected=[OpportunityEvent{sequence:2,kind:"message:orion".into()}];assert!(matches!(runtime.run_due("p","exp",&selected).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="rest"));assert_eq!(decisions.calls(),2);
}

#[tokio::test]
async fn event_only_wait_calls_primary_only_for_selected_event(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::WaitEvent{interrupt_events:vec!["message:aura".into()]},DecisionChoice::NoAction]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="wait_event"));
 clock.advance_ms(86_400_000);assert_eq!(runtime.run_due("p","exp",&[OpportunityEvent{sequence:4,kind:"message:other".into()}]).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),1);
 assert!(matches!(runtime.run_due("p","exp",&[OpportunityEvent{sequence:5,kind:"message:aura".into()}]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="no_action"));assert_eq!(decisions.calls(),2);
}

#[tokio::test]
async fn refusal_is_durable_and_visible_to_later_decision_packet(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Refusal{scope:"task:delete-history".into(),reconsideration_conditions:vec!["materially new information".into()]},DecisionChoice::NoAction]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="refusal"));
 let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_refusals WHERE profile_id='p' AND json_extract(body,'$.status')='active'").fetch_one(&store.pool).await.unwrap();assert_eq!(n,1);
 clock.advance_ms(1_000);assert!(matches!(runtime.run_due("p","exp",&[OpportunityEvent{sequence:1,kind:"human_request:repeat".into()}]).await.unwrap(),RunResult::Completed{..}));
 let packets=decisions.packets();assert_eq!(packets[1].active_refusals.len(),1);assert_eq!(packets[1].active_refusals[0]["status"],"active");
}

#[tokio::test]
async fn fake_peer_action_persists_evidence_before_returning_ready(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let peer=Arc::new(FakePeer::accepted());
 let intent=ActionIntent{connector:"local_test_peer".into(),action_kind:"message.send".into(),sender_identity:"agent:test".into(),destination:"peer:test".into(),visibility:"private".into(),payload_ref:"test-payload:1".into(),payload_bytes:b"hello peer".to_vec(),idempotency_key:"idem-1".into(),episode_id:"".into()};
 let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Action(intent)]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions,peer.clone());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="action"));assert_eq!(peer.calls.lock().unwrap().len(),1);
 let body:String=sqlx::query("SELECT body FROM experimental_actions WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap().get(0);let action:Value=serde_json::from_str(&body).unwrap();assert_eq!(action["state"],"accepted");assert_eq!(action["receiptRef"],"peer-receipt-1");assert_eq!(action["payloadHash"].as_str().unwrap().len(),64);assert!(action.get("rawPayload").is_none());
}

#[tokio::test]
async fn restart_recovers_running_cycle_and_sent_action_without_fabricating_outcome(){
 let path:PathBuf=std::env::temp_dir().join(format!("ai-home-exp1-restart-{}.db",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();profile(&store,"p").await;
 let inst=json!({"schemaVersion":1,"id":"exp","profileId":"p","direction":"experimental","revision":1,"runtimeGeneration":4,"status":"running","eventCursor":0,"createdAt":"2026-10-24T12:00:00Z","updatedAt":"2026-10-24T12:01:00Z"});sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES('p','exp',1,?)").bind(inst.to_string()).execute(&store.pool).await.unwrap();
 let action=json!({"schemaVersion":1,"id":"a","instanceId":"exp","revision":1,"decisionId":"d","episodeId":"","connector":"local_test_peer","actionKind":"message.send","senderIdentity":"agent:test","destination":"peer:test","visibility":"private","payloadHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","payloadRef":"test:a","idempotencyKey":"idem","state":"sent","receiptRef":"","createdAt":"2026-10-24T12:01:00Z","updatedAt":"2026-10-24T12:01:00Z"});sqlx::query("INSERT INTO experimental_actions(profile_id,instance_id,id,revision,body) VALUES('p','exp','a',1,?)").bind(action.to_string()).execute(&store.pool).await.unwrap();
 let wake=json!({"schemaVersion":1,"id":"w","instanceId":"exp","revision":1,"origin":"initial_configuration","decisionId":"","status":"active","createdAt":"2026-10-24T12:00:00Z","mode":"interval","requestedDelayMs":120000,"interruptEvents":[]});sqlx::query("INSERT INTO experimental_wake_plans(profile_id,instance_id,id,revision,body) VALUES('p','exp','w',1,?)").bind(wake.to_string()).execute(&store.pool).await.unwrap();let cursor=json!({"schemaVersion":1,"id":"c","instanceId":"exp","revision":1,"receivedThrough":0,"consideredThrough":0,"committedThrough":0,"updatedAt":"2026-10-24T12:00:00Z"});sqlx::query("INSERT INTO experimental_event_cursors(profile_id,instance_id,id,revision,body) VALUES('p','exp','c',1,?)").bind(cursor.to_string()).execute(&store.pool).await.unwrap();
 let pool=store.pool.clone();drop(store);pool.close().await;
 let reopened=Store::open(&path).await.unwrap();let ibody:String=sqlx::query("SELECT body FROM experimental_instances WHERE profile_id='p'").fetch_one(&reopened.pool).await.unwrap().get(0);let i:Value=serde_json::from_str(&ibody).unwrap();assert_eq!(i["status"],"interrupted");assert_eq!(i["runtimeGeneration"],5);
 let abody:String=sqlx::query("SELECT body FROM experimental_actions WHERE profile_id='p' AND id='a'").fetch_one(&reopened.pool).await.unwrap().get(0);let a:Value=serde_json::from_str(&abody).unwrap();assert_eq!(a["state"],"uncertain");
 let clock=Arc::new(FakeClock::at("2026-10-24T12:01:01Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::NoAction]));let runtime=ExperimentalRuntime::new(reopened.pool.clone(),clock,decisions.clone(),disabled_action());assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ReconciliationRequired);assert_eq!(decisions.calls(),0);let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&reopened.pool).await.unwrap();assert_eq!(status,"recovery_required");
 let pool=reopened.pool.clone();drop(reopened);pool.close().await;let _=std::fs::remove_file(path);
}

struct UnavailableDecision;
impl DecisionProvider for UnavailableDecision{fn decide(&self,_:&OpportunityPacket)->Result<DecisionChoice,String>{Err("provider unavailable".into())}}

#[tokio::test]
async fn resource_unavailability_is_not_persisted_as_ai_refusal(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),Arc::new(UnavailableDecision),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ResourceUnavailable);
 let refusals:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_refusals WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(refusals,0);
 let decisions:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_decisions WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(decisions,0);
 let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(status,"ready");
}

#[tokio::test]
async fn missed_interval_is_coalesced_to_one_opportunity(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::NoAction]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",60_000).await.unwrap();
 clock.advance_ms(24*60*60*1000);assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{..}));assert_eq!(decisions.calls(),1);
 for _ in 0..10 {assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);}assert_eq!(decisions.calls(),1);
}


struct EmptyReceiptPeer{calls:Mutex<usize>}
impl ActionExecutor for EmptyReceiptPeer{fn execute(&self,_:&ActionIntent)->ActionOutcome{*self.calls.lock().unwrap()+=1;ActionOutcome::Accepted{receipt_ref:String::new()}}}

#[tokio::test]
async fn committed_event_cannot_wake_wait_event_twice_and_new_sequence_can(){
 let store=Store::memory().await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));
 let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::WaitEvent{interrupt_events:vec!["message:aura".into()]},DecisionChoice::WaitEvent{interrupt_events:vec!["message:aura".into()]},DecisionChoice::NoAction]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="wait_event"));
 let e5=[OpportunityEvent{sequence:5,kind:"message:aura".into()}];assert!(matches!(runtime.run_due("p","exp",&e5).await.unwrap(),RunResult::Completed{outcome,..} if outcome=="wait_event"));assert_eq!(decisions.calls(),2);
 assert_eq!(runtime.run_due("p","exp",&e5).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),2);
 let e6=[OpportunityEvent{sequence:6,kind:"message:aura".into()}];assert!(matches!(runtime.run_due("p","exp",&e6).await.unwrap(),RunResult::Completed{..}));assert_eq!(decisions.calls(),3);
 let committed:i64=sqlx::query_scalar("SELECT json_extract(body,'$.committedThrough') FROM experimental_event_cursors WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(committed,6);
}

#[tokio::test]
async fn committed_event_cannot_interrupt_new_sleep_after_restart(){
 let path:PathBuf=std::env::temp_dir().join(format!("ai-home-exp1-event-restart-{}.db",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Sleep{delay_ms:60_000,interrupt_events:vec!["message:aura".into()]},DecisionChoice::Sleep{delay_ms:60_000,interrupt_events:vec!["message:aura".into()]}]));
 let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);runtime.run_due("p","exp",&[]).await.unwrap();
 let e5=[OpportunityEvent{sequence:5,kind:"message:aura".into()}];runtime.run_due("p","exp",&e5).await.unwrap();assert_eq!(decisions.calls(),2);
 let pool=store.pool.clone();drop(runtime);drop(store);pool.close().await;
 let reopened=Store::open(&path).await.unwrap();let runtime=ExperimentalRuntime::new(reopened.pool.clone(),clock.clone(),decisions.clone(),disabled_action());assert_eq!(runtime.run_due("p","exp",&e5).await.unwrap(),RunResult::NotDue);assert_eq!(decisions.calls(),2);
 let pool=reopened.pool.clone();drop(runtime);drop(reopened);pool.close().await;let _=std::fs::remove_file(path);
}

#[tokio::test]
async fn invalid_decisions_release_claim_instead_of_leaving_running(){
 for choice in [DecisionChoice::Sleep{delay_ms:0,interrupt_events:vec![]},DecisionChoice::WaitEvent{interrupt_events:vec![]},DecisionChoice::Continue{episode_id:"missing".into()}] {
  let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let decisions=Arc::new(ScriptedDecision::new(vec![choice]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions,disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
  assert!(runtime.run_due("p","exp",&[]).await.is_err());let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(status,"ready");
 }
}

#[tokio::test]
async fn invalid_action_intent_releases_claim_without_external_effect(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let peer=Arc::new(FakePeer::accepted());let intent=ActionIntent{connector:"local_test_peer".into(),action_kind:"message.send".into(),sender_identity:"agent:test".into(),destination:"peer:test".into(),visibility:"invalid".into(),payload_ref:"test:1".into(),payload_bytes:b"x".to_vec(),idempotency_key:"i".into(),episode_id:"".into()};let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Action(intent)]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions,peer.clone());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);assert!(runtime.run_due("p","exp",&[]).await.is_err());assert_eq!(peer.calls.lock().unwrap().len(),0);let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(status,"ready");
}

#[tokio::test]
async fn accepted_without_receipt_becomes_uncertain_and_requires_reconciliation(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let peer=Arc::new(EmptyReceiptPeer{calls:Mutex::new(0)});let intent=ActionIntent{connector:"local_test_peer".into(),action_kind:"message.send".into(),sender_identity:"agent:test".into(),destination:"peer:test".into(),visibility:"private".into(),payload_ref:"test:1".into(),payload_bytes:b"x".to_vec(),idempotency_key:"i".into(),episode_id:"".into()};let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Action(intent)]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions,peer.clone());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap_err(),"ACTION_OUTCOME_UNCERTAIN");assert_eq!(*peer.calls.lock().unwrap(),1);let state:String=sqlx::query_scalar("SELECT json_extract(body,'$.state') FROM experimental_actions WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(state,"uncertain");let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(status,"recovery_required");assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ReconciliationRequired);assert_eq!(*peer.calls.lock().unwrap(),1);
}

struct CountingUnavailable{calls:Mutex<usize>}
impl DecisionProvider for CountingUnavailable{fn decide(&self,_:&OpportunityPacket)->Result<DecisionChoice,String>{*self.calls.lock().unwrap()+=1;Err("provider unavailable".into())}}

#[tokio::test]
async fn resource_unavailable_uses_technical_backoff_not_ai_wake_plan(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let provider=Arc::new(CountingUnavailable{calls:Mutex::new(0)});let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),provider.clone(),disabled_action());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ResourceUnavailable);for _ in 0..10{assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::NotDue);}assert_eq!(*provider.calls.lock().unwrap(),1);let origin:String=sqlx::query_scalar("SELECT json_extract(body,'$.origin') FROM experimental_wake_plans WHERE profile_id='p' AND json_extract(body,'$.status')='active'").fetch_one(&store.pool).await.unwrap();assert_eq!(origin,"initial_configuration");clock.advance_ms(30_000);assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ResourceUnavailable);assert_eq!(*provider.calls.lock().unwrap(),2);
}

#[tokio::test]
async fn existing_standard_profile_cannot_be_converted_to_experimental(){
 let store=Store::memory().await.unwrap();profile(&store,"standard").await;let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock,Arc::new(ScriptedDecision::new(vec![])),disabled_action());assert_eq!(runtime.create_instance("standard","exp",1_000).await.unwrap_err(),"POLICY_DENIED");let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_instances WHERE profile_id='standard'").fetch_one(&store.pool).await.unwrap();assert_eq!(n,0);
}

#[tokio::test]
async fn create_instance_atomically_creates_direction_bound_profile(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock,Arc::new(ScriptedDecision::new(vec![])),disabled_action());runtime.create_instance("experimental","exp",1_000).await.unwrap();let mode:String=sqlx::query_scalar("SELECT json_extract(body,'$.mode') FROM profiles WHERE id='experimental'").fetch_one(&store.pool).await.unwrap();assert_eq!(mode,"conscious");let direction:String=sqlx::query_scalar("SELECT json_extract(body,'$.direction') FROM experimental_instances WHERE profile_id='experimental'").fetch_one(&store.pool).await.unwrap();assert_eq!(direction,"experimental");
}

struct OneShotAcceptedPeer{calls:Mutex<usize>}
impl ActionExecutor for OneShotAcceptedPeer{fn execute(&self,_:&ActionIntent)->ActionOutcome{*self.calls.lock().unwrap()+=1;ActionOutcome::Accepted{receipt_ref:"receipt-once".into()}}}

#[tokio::test]
async fn post_effect_action_update_failure_blocks_new_decision_until_reconciliation(){
 let store=Store::memory().await.unwrap();let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let peer=Arc::new(OneShotAcceptedPeer{calls:Mutex::new(0)});
 let intent=ActionIntent{connector:"local_test_peer".into(),action_kind:"message.send".into(),sender_identity:"agent:test".into(),destination:"peer:test".into(),visibility:"private".into(),payload_ref:"test:post-effect".into(),payload_bytes:b"once".to_vec(),idempotency_key:"idem-fixed".into(),episode_id:"".into()};
 let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Action(intent),DecisionChoice::NoAction]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),peer.clone());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 sqlx::query("CREATE TRIGGER fail_action_receipt_update BEFORE UPDATE ON experimental_actions BEGIN SELECT RAISE(ABORT,'forced post effect write failure'); END;").execute(&store.pool).await.unwrap();
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap_err(),"STORAGE_ERROR");assert_eq!(*peer.calls.lock().unwrap(),1);
 sqlx::query("DROP TRIGGER fail_action_receipt_update").execute(&store.pool).await.unwrap();
 let state:String=sqlx::query_scalar("SELECT json_extract(body,'$.state') FROM experimental_actions WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(state,"sent");
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ReconciliationRequired);assert_eq!(*peer.calls.lock().unwrap(),1);assert_eq!(decisions.calls(),1);
 let status:String=sqlx::query_scalar("SELECT json_extract(body,'$.status') FROM experimental_instances WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(status,"recovery_required");
}

#[tokio::test]
async fn finalization_failure_after_executor_is_fenced_across_restart(){
 let path:PathBuf=std::env::temp_dir().join(format!("ai-home-exp1-post-effect-{}.db",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();
 let clock=Arc::new(FakeClock::at("2026-10-24T12:00:00Z"));let peer=Arc::new(OneShotAcceptedPeer{calls:Mutex::new(0)});
 let intent=ActionIntent{connector:"local_test_peer".into(),action_kind:"message.send".into(),sender_identity:"agent:test".into(),destination:"peer:test".into(),visibility:"private".into(),payload_ref:"test:commit".into(),payload_bytes:b"once".to_vec(),idempotency_key:"idem-stable".into(),episode_id:"".into()};
 let decisions=Arc::new(ScriptedDecision::new(vec![DecisionChoice::Action(intent),DecisionChoice::NoAction]));let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),peer.clone());runtime.create_instance("p","exp",1_000).await.unwrap();clock.advance_ms(1_000);
 sqlx::query("CREATE TRIGGER fail_ready_finalization BEFORE UPDATE ON experimental_instances WHEN json_extract(NEW.body,'$.status')='ready' BEGIN SELECT RAISE(ABORT,'forced finish failure'); END;").execute(&store.pool).await.unwrap();
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap_err(),"STORAGE_ERROR");assert_eq!(*peer.calls.lock().unwrap(),1);
 sqlx::query("DROP TRIGGER fail_ready_finalization").execute(&store.pool).await.unwrap();
 let state:String=sqlx::query_scalar("SELECT json_extract(body,'$.state') FROM experimental_actions WHERE profile_id='p'").fetch_one(&store.pool).await.unwrap();assert_eq!(state,"sent");
 let pool=store.pool.clone();drop(runtime);drop(store);pool.close().await;
 let reopened=Store::open(&path).await.unwrap();let runtime=ExperimentalRuntime::new(reopened.pool.clone(),clock,decisions.clone(),peer.clone());
 assert_eq!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::ReconciliationRequired);assert_eq!(*peer.calls.lock().unwrap(),1);assert_eq!(decisions.calls(),1);
 let recovered_state:String=sqlx::query_scalar("SELECT json_extract(body,'$.state') FROM experimental_actions WHERE profile_id='p'").fetch_one(&reopened.pool).await.unwrap();assert!(recovered_state=="sent"||recovered_state=="uncertain");
 let pool=reopened.pool.clone();drop(runtime);drop(reopened);pool.close().await;let _=std::fs::remove_file(path);
}
