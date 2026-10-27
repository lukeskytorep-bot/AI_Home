use ai_home::{domain::schema,persistence::Store};
use serde_json::{json,Value};
use sqlx::Row;

fn source(availability:&str)->Value{json!({
    "sourceKind":"conversation","sourceId":"conversation-a","sourceRevision":3,
    "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","availability":availability
})}
fn instance(pid:&str)->Value{json!({
    "schemaVersion":1,"id":"exp-a","profileId":pid,"direction":"experimental","revision":1,
    "runtimeGeneration":1,"status":"inactive","eventCursor":0,
    "createdAt":"2026-10-24T12:00:00Z","updatedAt":"2026-10-24T12:00:00Z"
})}
fn decision()->Value{json!({
    "schemaVersion":1,"id":"decision-a","instanceId":"exp-a","revision":1,"opportunityId":"opp-a",
    "episodeId":"episode-a","outcome":"refusal","refusalId":"refusal-a",
    "contextRevision":1,"createdAt":"2026-10-24T12:01:00Z"
})}

#[test]
fn experimental_dtos_validate_and_reject_privacy_shortcuts(){
    let values=[
        ("ExperimentalSourceRef",source("available")),
        ("ExperimentalSourceRef",source("permanently_deleted")),
        ("ExperimentalInstance",instance("profile-a")),
        ("WakePlan",json!({"schemaVersion":1,"id":"wake-a","instanceId":"exp-a","revision":1,"origin":"ai_decision","decisionId":"decision-a","mode":"deadline","nextWakeAt":"2026-10-24T18:00:00Z","interruptEvents":["message:orion"],"status":"active","createdAt":"2026-10-24T12:01:00Z"})),
        ("DecisionRecord",decision()),
        ("RefusalRecord",json!({"schemaVersion":1,"id":"refusal-a","instanceId":"exp-a","revision":1,"decisionId":"decision-a","scopeFingerprint":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","status":"active","reconsiderationConditions":["materially new evidence"],"sourceRefs":[source("available")],"createdAt":"2026-10-24T12:01:00Z"})),
        ("WorkEpisode",json!({"schemaVersion":1,"id":"episode-a","instanceId":"exp-a","revision":1,"status":"active","title":"Review a topic","originDecisionId":"decision-a","createdAt":"2026-10-24T12:01:00Z","updatedAt":"2026-10-24T12:01:00Z"})),
        ("TechnicalWorkCheckpoint",json!({"schemaVersion":1,"id":"tech-a","instanceId":"exp-a","episodeId":"episode-a","revision":1,"parentCheckpointId":"","sourceEventSequence":4,"status":"active","decisionRefs":["decision-a"],"actionRefs":[],"sourceRefs":[source("available")],"createdAt":"2026-10-24T12:02:00Z"})),
        ("SemanticWorkCheckpoint",json!({"schemaVersion":1,"id":"sem-a","instanceId":"exp-a","episodeId":"episode-a","revision":1,"parentCheckpointId":"","technicalCheckpointId":"tech-a","proposer":"scribe","primaryDisposition":"modified","sourceEventSequence":4,"sourceRefs":[source("available")],"content":{"focus":"Review a topic","currentIntention":"Understand it","findings":["One bounded finding"],"openQuestions":["What remains?"],"nextSuggestedStep":"Inspect referenced source"},"createdAt":"2026-10-24T12:03:00Z"})),
        ("ActionRecord",json!({"schemaVersion":1,"id":"action-a","instanceId":"exp-a","revision":1,"decisionId":"decision-a","episodeId":"episode-a","connector":"community","actionKind":"reply","senderIdentity":"agent:orion","destination":"thread:42","visibility":"public","payloadHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","payloadRef":"payload:action-a:v1","idempotencyKey":"idem-a","state":"prepared","receiptRef":"","createdAt":"2026-10-24T12:04:00Z","updatedAt":"2026-10-24T12:04:00Z"})),
        ("ExperimentalEventCursor",json!({"schemaVersion":1,"id":"cursor-a","instanceId":"exp-a","revision":1,"receivedThrough":9,"consideredThrough":7,"committedThrough":7,"updatedAt":"2026-10-24T12:04:00Z"}))
    ];
    for (name,value) in values {assert!(schema::dto(name,&value),"invalid {name}: {value}");}

    let mut action=json!({"schemaVersion":1,"id":"action-a","instanceId":"exp-a","revision":1,"decisionId":"decision-a","episodeId":"","connector":"community","actionKind":"reply","senderIdentity":"agent:orion","destination":"thread:42","visibility":"public","payloadHash":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","payloadRef":"payload:action-a:v1","idempotencyKey":"idem-a","state":"prepared","receiptRef":"","createdAt":"2026-10-24T12:04:00Z","updatedAt":"2026-10-24T12:04:00Z"});
    action["rawPayload"]=json!("private message must not live in ActionRecord");
    assert!(!schema::dto("ActionRecord",&action));

    let accepted_without_receipt=json!({"schemaVersion":1,"id":"action-accepted","instanceId":"exp-a","revision":1,"decisionId":"decision-a","episodeId":"","connector":"community","actionKind":"reply","senderIdentity":"agent:orion","destination":"thread:42","visibility":"public","payloadHash":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","payloadRef":"payload:action-accepted:v1","idempotencyKey":"idem-accepted","state":"accepted","receiptRef":"","createdAt":"2026-10-24T12:04:00Z","updatedAt":"2026-10-24T12:04:00Z"});
    assert!(!schema::dto("ActionRecord",&accepted_without_receipt));
    let accepted_with_receipt=json!({"schemaVersion":1,"id":"action-accepted","instanceId":"exp-a","revision":1,"decisionId":"decision-a","episodeId":"","connector":"community","actionKind":"reply","senderIdentity":"agent:orion","destination":"thread:42","visibility":"public","payloadHash":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","payloadRef":"payload:action-accepted:v1","idempotencyKey":"idem-accepted","state":"accepted","receiptRef":"receipt:community:42","createdAt":"2026-10-24T12:04:00Z","updatedAt":"2026-10-24T12:04:00Z"});
    assert!(schema::dto("ActionRecord",&accepted_with_receipt));
    let reconciled_without_outcome=json!({"schemaVersion":1,"id":"action-reconciled","instanceId":"exp-a","revision":1,"decisionId":"decision-a","episodeId":"","connector":"community","actionKind":"reply","senderIdentity":"agent:orion","destination":"thread:42","visibility":"public","payloadHash":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee","payloadRef":"payload:action-reconciled:v1","idempotencyKey":"idem-reconciled","state":"reconciled","receiptRef":"reconcile:community:42","createdAt":"2026-10-24T12:04:00Z","updatedAt":"2026-10-24T12:04:00Z"});
    assert!(!schema::dto("ActionRecord",&reconciled_without_outcome));
    let mut reconciled=reconciled_without_outcome;reconciled["reconciliationOutcome"]=json!("still_unknown");
    assert!(schema::dto("ActionRecord",&reconciled));

    let mut technical=json!({"schemaVersion":1,"id":"tech-a","instanceId":"exp-a","episodeId":"episode-a","revision":1,"parentCheckpointId":"","sourceEventSequence":4,"status":"active","decisionRefs":[],"actionRefs":[],"sourceRefs":[],"createdAt":"2026-10-24T12:02:00Z"});
    technical["focus"]=json!("semantic content does not belong in technical checkpoint");
    assert!(!schema::dto("TechnicalWorkCheckpoint",&technical));
}

#[tokio::test]
async fn schema_14_persists_experimental_and_internal_state_foundation_with_foreign_keys(){
    let store=Store::memory().await.unwrap();
    let pool=&store.pool;
    assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(pool).await.unwrap(),16);
    let profile=json!({"schemaVersion":1,"id":"profile-a","revision":1,"displayName":"Experimental","createdAt":"2026-10-24T12:00:00Z","updatedAt":"2026-10-24T12:00:00Z"});
    sqlx::query("INSERT INTO profiles(id,revision,body) VALUES(?,1,?)").bind("profile-a").bind(profile.to_string()).execute(pool).await.unwrap();
    let inst=instance("profile-a");
    sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES(?,?,1,?)").bind("profile-a").bind("exp-a").bind(inst.to_string()).execute(pool).await.unwrap();
    let episode=json!({"schemaVersion":1,"id":"episode-a","instanceId":"exp-a","revision":1,"status":"active","title":"Review a topic","originDecisionId":"","createdAt":"2026-10-24T12:01:00Z","updatedAt":"2026-10-24T12:01:00Z"});
    sqlx::query("INSERT INTO experimental_work_episodes(profile_id,instance_id,id,revision,body) VALUES(?,?,?,1,?)").bind("profile-a").bind("exp-a").bind("episode-a").bind(episode.to_string()).execute(pool).await.unwrap();
    let technical=json!({"schemaVersion":1,"id":"tech-a","instanceId":"exp-a","episodeId":"episode-a","revision":1,"parentCheckpointId":"","sourceEventSequence":0,"status":"active","decisionRefs":[],"actionRefs":[],"sourceRefs":[],"createdAt":"2026-10-24T12:02:00Z"});
    sqlx::query("INSERT INTO experimental_technical_checkpoints(profile_id,instance_id,episode_id,id,revision,body) VALUES(?,?,?,?,1,?)").bind("profile-a").bind("exp-a").bind("episode-a").bind("tech-a").bind(technical.to_string()).execute(pool).await.unwrap();
    assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(pool).await.unwrap().is_empty());
    let tables:i64=sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE 'experimental_%'").fetch_one(pool).await.unwrap();
    assert_eq!(tables,17);
}

#[tokio::test]
async fn actual_v13_database_migrates_to_v14_without_changing_existing_experimental_or_standard_data(){
    let path=std::env::temp_dir().join(format!("ai-home-exp-state-v13-{}.db",uuid::Uuid::new_v4()));
    let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();
    let profile_body=json!({"id":"standard-a","marker":"keep-standard-byte-content"}).to_string();
    sqlx::query("INSERT INTO profiles(id,revision,body) VALUES('standard-a',1,?)").bind(&profile_body).execute(&pool).await.unwrap();
    let exp_profile=json!({"id":"experimental-a","marker":"keep-experimental-byte-content"}).to_string();
    sqlx::query("INSERT INTO profiles(id,revision,body) VALUES('experimental-a',1,?)").bind(&exp_profile).execute(&pool).await.unwrap();
    let inst=json!({"schemaVersion":1,"id":"exp-a","profileId":"experimental-a","direction":"experimental","revision":1,"runtimeGeneration":1,"status":"inactive","eventCursor":0,"createdAt":"2026-10-25T12:00:00Z","updatedAt":"2026-10-25T12:00:00Z"});
    sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES('experimental-a','exp-a',1,?)").bind(inst.to_string()).execute(&pool).await.unwrap();
    for table in ["experimental_wake_candidates","experimental_internal_state_events","experimental_internal_state_snapshots","experimental_internal_state_blobs"] {sqlx::query(&format!("DROP TABLE {table}")).execute(&pool).await.unwrap();}
    sqlx::query("PRAGMA user_version=13").execute(&pool).await.unwrap();drop(store);pool.close().await;
    let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();
    assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&pool).await.unwrap(),16);
    let body:String=sqlx::query("SELECT body FROM profiles WHERE id='standard-a'").fetch_one(&pool).await.unwrap().get(0);assert_eq!(body,profile_body);
    let exp_body:String=sqlx::query("SELECT body FROM profiles WHERE id='experimental-a'").fetch_one(&pool).await.unwrap().get(0);assert_eq!(exp_body,exp_profile);
    let instance_count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM experimental_instances WHERE profile_id='experimental-a'").fetch_one(&pool).await.unwrap();assert_eq!(instance_count,1);
    assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap().is_empty());
    drop(store);pool.close().await;let _=std::fs::remove_file(path);
}
