use ai_home::{
    experimental_continuity::{preflight_context,ContextComponentEstimate,ContextPreflightStatus,ContextWindowBudget,WorkContinuityService},
    experimental_runtime::{ActionExecutor,ActionIntent,ActionOutcome,Clock,DecisionChoice,DecisionProvider,DisabledActionExecutor,DisabledDecisionProvider,ExperimentalRuntime,OpportunityPacket,RunResult,SystemClock},
    persistence::Store,
};
use chrono::{DateTime,Utc};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::VecDeque,path::PathBuf,sync::{Arc,Mutex}};

fn now()->DateTime<Utc>{DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z").unwrap().with_timezone(&Utc)}

struct TestClock(Mutex<DateTime<Utc>>);
impl TestClock{fn new()->Self{Self(Mutex::new(now()))}fn advance(&self,ms:i64){let mut v=self.0.lock().unwrap();*v=v.clone()+chrono::Duration::milliseconds(ms);}}
impl Clock for TestClock{fn now(&self)->DateTime<Utc>{self.0.lock().unwrap().clone()}}
struct CaptureDecision{packets:Mutex<Vec<OpportunityPacket>>,choices:Mutex<VecDeque<DecisionChoice>>}
impl CaptureDecision{fn new()->Self{Self{packets:Mutex::new(vec![]),choices:Mutex::new(vec![DecisionChoice::NoAction].into())}}}
impl DecisionProvider for CaptureDecision{fn decide(&self,p:&OpportunityPacket)->Result<DecisionChoice,String>{self.packets.lock().unwrap().push(p.clone());self.choices.lock().unwrap().pop_front().ok_or_else(||"no decision".into())}}
struct NoActionExecutor;impl ActionExecutor for NoActionExecutor{fn execute(&self,_:&ActionIntent)->ActionOutcome{ActionOutcome::Failed}}

async fn setup(store:&Store)->WorkContinuityService{
    let runtime=ExperimentalRuntime::new(store.pool.clone(),Arc::new(SystemClock),Arc::new(DisabledDecisionProvider),Arc::new(DisabledActionExecutor));
    runtime.create_instance("p","exp",120_000).await.unwrap();
    WorkContinuityService::new(store.pool.clone())
}
fn source(kind:&str,id:&str,rev:i64,seed:&str)->Value{json!({"sourceKind":kind,"sourceId":id,"sourceRevision":rev,"sha256":format!("{:x}",Sha256::digest(seed.as_bytes())),"availability":"available"})}
fn content(focus:&str)->Value{json!({"focus":focus,"currentIntention":"continue carefully","findings":["finding one"],"openQuestions":["what next?"],"nextSuggestedStep":"review the next source"})}

#[test]
fn preflight_preserves_room_for_checkpoint_before_context_exhaustion(){
    let budget=ContextWindowBudget{context_tokens:10_000,reserved_output_tokens:2_000,input_reserve_tokens:500,checkpoint_reserve_tokens:1_000};
    let fits=preflight_context(&budget,&[ContextComponentEstimate{label:"resume".into(),estimated_tokens:3_000}]).unwrap();
    assert_eq!(fits.status,ContextPreflightStatus::Fits);
    let pressure=preflight_context(&budget,&[ContextComponentEstimate{label:"resume".into(),estimated_tokens:6_800}]).unwrap();
    assert_eq!(pressure.status,ContextPreflightStatus::CheckpointRequired);
    assert_eq!(pressure.measurement,"caller_estimated_tokens");
    let impossible=preflight_context(&budget,&[ContextComponentEstimate{label:"resume".into(),estimated_tokens:7_600}]).unwrap();
    assert_eq!(impossible.status,ContextPreflightStatus::Impossible);
}

#[test]
fn route_change_requires_fresh_preflight_against_new_context_window(){
    let components=[ContextComponentEstimate{label:"resume-packet".into(),estimated_tokens:20_000}];
    let large=ContextWindowBudget{context_tokens:32_000,reserved_output_tokens:4_000,input_reserve_tokens:1_000,checkpoint_reserve_tokens:2_000};
    let small=ContextWindowBudget{context_tokens:16_000,reserved_output_tokens:4_000,input_reserve_tokens:1_000,checkpoint_reserve_tokens:2_000};
    assert_eq!(preflight_context(&large,&components).unwrap().status,ContextPreflightStatus::Fits);
    assert_eq!(preflight_context(&small,&components).unwrap().status,ContextPreflightStatus::Impossible);
}

#[tokio::test]
async fn technical_and_semantic_checkpoints_build_resume_packet_without_hidden_reasoning(){
    let store=Store::memory().await.unwrap();let service=setup(&store).await;
    let ep=service.create_episode("p","exp","Investigate thread","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    let refs=vec![source("tool_result","tool:1",1,"very large tool result")];
    let tech=service.write_technical_checkpoint("p","exp",eid,"","active",0,&[],&[],&refs,now()).await.unwrap();
    let sem=service.write_semantic_checkpoint("p","exp",eid,tech["id"].as_str().unwrap(),"","scribe","modified",content("Reviewing evidence"),&refs,now()).await.unwrap();
    let packet=service.build_resume_packet("p","exp",eid,now()).await.unwrap();
    assert_eq!(packet.technical_checkpoint["id"],tech["id"]);assert_eq!(packet.semantic_checkpoint.as_ref().unwrap()["id"],sem["id"]);
    let encoded=format!("{:?}",packet);
    assert!(!encoded.contains("very large tool result"));
    assert!(!encoded.to_lowercase().contains("chain-of-thought"));
    assert!(encoded.contains("tool:1"));
}

#[tokio::test]
async fn stale_semantic_checkpoint_cannot_overwrite_newer_technical_work_state(){
    let store=Store::memory().await.unwrap();let service=setup(&store).await;
    let ep=service.create_episode("p","exp","Work","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    let t1=service.write_technical_checkpoint("p","exp",eid,"","active",0,&[],&[],&[],now()).await.unwrap();
    let t2=service.write_technical_checkpoint("p","exp",eid,t1["id"].as_str().unwrap(),"active",0,&[],&[],&[],now()).await.unwrap();
    assert_eq!(service.write_semantic_checkpoint("p","exp",eid,t1["id"].as_str().unwrap(),"","primary","accepted",content("stale"),&[],now()).await.unwrap_err(),"REVISION_CONFLICT");
    let good=service.write_semantic_checkpoint("p","exp",eid,t2["id"].as_str().unwrap(),"","primary","accepted",content("current"),&[],now()).await.unwrap();
    assert_eq!(good["content"]["focus"],"current");
}

#[tokio::test]
async fn technical_checkpoint_parent_is_compare_and_swap_fence(){
    let store=Store::memory().await.unwrap();let service=setup(&store).await;
    let ep=service.create_episode("p","exp","CAS","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    let first=service.write_technical_checkpoint("p","exp",eid,"","active",0,&[],&[],&[],now()).await.unwrap();
    assert_eq!(service.write_technical_checkpoint("p","exp",eid,"","active",0,&[],&[],&[],now()).await.unwrap_err(),"REVISION_CONFLICT");
    service.write_technical_checkpoint("p","exp",eid,first["id"].as_str().unwrap(),"waiting",0,&[],&[],&[],now()).await.unwrap();
}

#[tokio::test]
async fn restart_rebuilds_resume_packet_from_durable_work_state(){
    let path:PathBuf=std::env::temp_dir().join(format!("ai-home-exp2-continuity-{}.db",uuid::Uuid::new_v4()));
    let store=Store::open(&path).await.unwrap();let service=setup(&store).await;
    let ep=service.create_episode("p","exp","Persistent work","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap().to_string();
    let tech=service.write_technical_checkpoint("p","exp",&eid,"","interrupted",0,&[],&[],&[source("note","note:7",3,"note")],now()).await.unwrap();
    service.write_semantic_checkpoint("p","exp",&eid,tech["id"].as_str().unwrap(),"","primary","accepted",content("I was comparing two sources"),&[],now()).await.unwrap();
    let pool=store.pool.clone();drop(service);drop(store);pool.close().await;
    let reopened=Store::open(&path).await.unwrap();let service=WorkContinuityService::new(reopened.pool.clone());
    let packet=service.build_resume_packet("p","exp",&eid,now()).await.unwrap();
    assert_eq!(packet.episode["title"],"Persistent work");assert_eq!(packet.technical_checkpoint["status"],"interrupted");assert_eq!(packet.semantic_checkpoint.unwrap()["content"]["focus"],"I was comparing two sources");
    let pool=reopened.pool.clone();drop(service);drop(reopened);pool.close().await;let _=std::fs::remove_file(path);
}

#[tokio::test]
async fn resume_packet_exposes_unresolved_external_action_instead_of_hiding_it(){
    let store=Store::memory().await.unwrap();let service=setup(&store).await;
    let ep=service.create_episode("p","exp","External work","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    service.write_technical_checkpoint("p","exp",eid,"","interrupted",0,&[],&[],&[],now()).await.unwrap();
    let action=json!({"schemaVersion":1,"id":"a1","instanceId":"exp","revision":1,"decisionId":"d1","episodeId":eid,"connector":"local_test_peer","actionKind":"message.send","senderIdentity":"agent:test","destination":"peer:test","visibility":"private","payloadHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","payloadRef":"payload:a1","idempotencyKey":"idem-a1","createdAt":"2026-10-25T12:00:00Z","updatedAt":"2026-10-25T12:00:00Z","state":"uncertain","receiptRef":""});
    sqlx::query("INSERT INTO experimental_actions(profile_id,instance_id,id,revision,body) VALUES('p','exp','a1',1,?)").bind(action.to_string()).execute(&store.pool).await.unwrap();
    let packet=service.build_resume_packet("p","exp",eid,now()).await.unwrap();
    assert_eq!(packet.unresolved_actions.len(),1);assert_eq!(packet.unresolved_actions[0]["state"],"uncertain");
}


#[tokio::test]
async fn technical_checkpoint_source_sequence_is_monotonic_and_cannot_hide_semantic_state(){
    let store=Store::memory().await.unwrap();let service=setup(&store).await;
    let cursor_body:Value=serde_json::from_str(sqlx::query_scalar::<_,String>("SELECT body FROM experimental_event_cursors WHERE profile_id='p' AND instance_id='exp'").fetch_one(&store.pool).await.unwrap().as_str()).unwrap();
    let mut cursor=cursor_body;cursor["committedThrough"]=json!(10);cursor["consideredThrough"]=json!(10);cursor["receivedThrough"]=json!(10);cursor["revision"]=json!(2);
    sqlx::query("UPDATE experimental_event_cursors SET revision=2,body=? WHERE profile_id='p' AND instance_id='exp'").bind(cursor.to_string()).execute(&store.pool).await.unwrap();
    let ep=service.create_episode("p","exp","Monotonic work","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    let t1=service.write_technical_checkpoint("p","exp",eid,"","active",10,&[],&[],&[],now()).await.unwrap();
    let s1=service.write_semantic_checkpoint("p","exp",eid,t1["id"].as_str().unwrap(),"","primary","accepted",content("sequence ten"),&[],now()).await.unwrap();
    assert_eq!(service.write_technical_checkpoint("p","exp",eid,t1["id"].as_str().unwrap(),"active",0,&[],&[],&[],now()).await.unwrap_err(),"REVISION_CONFLICT");
    let unchanged=service.build_resume_packet("p","exp",eid,now()).await.unwrap();assert_eq!(unchanged.technical_checkpoint["id"],t1["id"]);assert_eq!(unchanged.semantic_checkpoint.as_ref().unwrap()["id"],s1["id"]);
    let t2=service.write_technical_checkpoint("p","exp",eid,t1["id"].as_str().unwrap(),"active",10,&[],&[],&[],now()).await.unwrap();
    let same=service.build_resume_packet("p","exp",eid,now()).await.unwrap();assert_eq!(same.technical_checkpoint["id"],t2["id"]);assert_eq!(same.semantic_checkpoint.as_ref().unwrap()["id"],s1["id"]);
}

#[tokio::test]
async fn empty_newer_episode_does_not_hide_older_checkpointed_work_after_restart(){
    let path:PathBuf=std::env::temp_dir().join(format!("ai-home-exp2-episode-selection-{}.db",uuid::Uuid::new_v4()));
    let store=Store::open(&path).await.unwrap();let service=setup(&store).await;
    let e1=service.create_episode("p","exp","Checkpointed work","",now()).await.unwrap();let e1id=e1["id"].as_str().unwrap().to_string();
    let t1=service.write_technical_checkpoint("p","exp",&e1id,"","active",0,&[],&[],&[],now()).await.unwrap();service.write_semantic_checkpoint("p","exp",&e1id,t1["id"].as_str().unwrap(),"","primary","accepted",content("older work remains resumable"),&[],now()).await.unwrap();
    let e2=service.create_episode("p","exp","Empty newer episode","",now()+chrono::Duration::milliseconds(1)).await.unwrap();let e2id=e2["id"].as_str().unwrap().to_string();
    let pool=store.pool.clone();drop(service);drop(store);pool.close().await;
    let reopened=Store::open(&path).await.unwrap();let service=WorkContinuityService::new(reopened.pool.clone());
    let packet=service.latest_resume_packet("p","exp",now()+chrono::Duration::seconds(1)).await.unwrap().expect("checkpointed work must remain resumable");assert_eq!(packet.episode["id"],e1id);assert_ne!(packet.episode["id"],e2id);
    sqlx::query("UPDATE experimental_work_episodes SET body=json_set(body,'$.status','completed') WHERE profile_id='p' AND id=?").bind(&e1id).execute(&reopened.pool).await.unwrap();
    assert!(service.latest_resume_packet("p","exp",now()).await.unwrap().is_none());
    let pool=reopened.pool.clone();drop(service);drop(reopened);pool.close().await;let _=std::fs::remove_file(path);
}

#[tokio::test]
async fn decision_opportunity_receives_resume_packet_when_active_work_has_checkpoint(){
    let store=Store::memory().await.unwrap();let clock=Arc::new(TestClock::new());let decisions=Arc::new(CaptureDecision::new());
    let runtime=ExperimentalRuntime::new(store.pool.clone(),clock.clone(),decisions.clone(),Arc::new(NoActionExecutor));
    runtime.create_instance("p","exp",1_000).await.unwrap();
    let service=WorkContinuityService::new(store.pool.clone());let ep=service.create_episode("p","exp","Resume me","",now()).await.unwrap();let eid=ep["id"].as_str().unwrap();
    let tech=service.write_technical_checkpoint("p","exp",eid,"","active",0,&[],&[],&[],now()).await.unwrap();
    service.write_semantic_checkpoint("p","exp",eid,tech["id"].as_str().unwrap(),"","primary","accepted",content("Current focus survives rollover"),&[],now()).await.unwrap();
    clock.advance(1_000);assert!(matches!(runtime.run_due("p","exp",&[]).await.unwrap(),RunResult::Completed{..}));
    let packets=decisions.packets.lock().unwrap();let resume=packets[0].resume_packet.as_ref().expect("resume packet");
    assert_eq!(resume.episode["id"],eid);assert_eq!(resume.semantic_checkpoint.as_ref().unwrap()["content"]["focus"],"Current focus survives rollover");
}
