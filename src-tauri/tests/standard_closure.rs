use ai_home::{application::Application,domain::schema,persistence::Store};
use serde_json::{json,Value};
fn req(id:&str,command:&str,payload:Value,profile:Option<&str>)->Value{let mut r=json!({"contractVersion":1,"requestId":id,"command":command,"payload":payload});if let Some(p)=profile{r["profileId"]=json!(p)}r}
#[tokio::test]
async fn activity_history_is_profile_scoped_and_survives_restart(){
 let path=std::env::temp_dir().join(format!("ai-home-standard-{}-{}.sqlite3",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
 let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::new(store);
 let a=app.dispatch(req("p-a","profile.create",json!({"displayName":"A"}),None)).await.response;assert_eq!(a["ok"],true,"{a}");let aid=a["result"]["id"].as_str().unwrap().to_string();
 let b=app.dispatch(req("p-b","profile.create",json!({"displayName":"B"}),None)).await.response;assert_eq!(b["ok"],true,"{b}");let bid=b["result"]["id"].as_str().unwrap().to_string();
 for (pid,opid) in [(&aid,"op-a"),(&bid,"op-b")]{let op=json!({"schemaVersion":1,"id":opid,"profileId":pid,"revision":1,"createdAt":"2026-10-23T12:00:00Z","kind":"connection_test","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":1,"limits":{"maxPhysicalCalls":4,"maxElapsedMs":180000},"recordRefs":[]});assert!(schema::dto("Operation",&op),"invalid operation {op}");let seq:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?").bind(pid).fetch_one(&pool).await.unwrap();let event=json!({"contractVersion":1,"eventId":format!("event-{opid}"),"profileId":pid,"operationId":opid,"sequence":seq,"eventType":"operation.changed","occurredAt":"2026-10-23T12:00:01Z","payload":op});assert!(schema::dto("EventEnvelope",&event),"invalid event {event}");sqlx::query("INSERT INTO profile_events(profile_id,sequence,body) VALUES(?,?,?)").bind(pid).bind(seq).bind(event.to_string()).execute(&pool).await.unwrap();}
 let first=app.dispatch(req("activity-a","activity.list",json!({"afterSequence":0,"limit":100}),Some(&aid))).await.response;assert_eq!(first["ok"],true,"{first}");assert_eq!(first["result"]["items"].as_array().unwrap().len(),1);let activity=&first["result"]["items"][0];assert!(schema::dto("Activity",activity),"invalid activity {activity}");assert_eq!(activity["profileId"],aid);assert_eq!(activity["operationId"],"op-a");assert_eq!(activity["status"],"running");assert_eq!(activity["code"],"operation.connection_test");
 drop(app);pool.close().await;
 let app=Application::new(Store::open(&path).await.unwrap());let replay=app.dispatch(req("activity-restart","activity.list",json!({"afterSequence":0,"limit":100}),Some(&aid))).await.response;assert_eq!(replay["ok"],true,"{replay}");assert_eq!(replay["result"]["items"].as_array().unwrap().len(),1);assert_eq!(replay["result"]["items"][0]["operationId"],"op-a");let foreign=app.dispatch(req("activity-b","activity.list",json!({"afterSequence":0,"limit":100}),Some(&bid))).await.response;assert_eq!(foreign["result"]["items"][0]["operationId"],"op-b");
 drop(app);let _=std::fs::remove_file(&path);let _=std::fs::remove_file(path.with_extension("sqlite3-wal"));let _=std::fs::remove_file(path.with_extension("sqlite3-shm"));
}

#[tokio::test]
async fn activity_skips_permanent_deletion_tombstones_and_reaches_later_events_after_restart(){
 let path=std::env::temp_dir().join(format!("ai-home-activity-delete-{}-{}.sqlite3",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
 let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::new(store);
 let a=app.dispatch(req("p-delete-a","profile.create",json!({"displayName":"A"}),None)).await.response;assert_eq!(a["ok"],true,"{a}");let aid=a["result"]["id"].as_str().unwrap().to_string();
 let b=app.dispatch(req("p-delete-b","profile.create",json!({"displayName":"B"}),None)).await.response;assert_eq!(b["ok"],true,"{b}");let bid=b["result"]["id"].as_str().unwrap().to_string();
 let c=app.dispatch(req("c-delete","conversation.create",json!({"title":"Deleted activity source","language":"en"}),Some(&aid))).await.response;assert_eq!(c["ok"],true,"{c}");let cid=c["result"]["id"].as_str().unwrap().to_string();let crev=c["result"]["revision"].as_i64().unwrap();
 async fn insert_event(pool:&sqlx::SqlitePool,pid:&str,opid:&str,sequence:i64,result:Value){
  let op=json!({"schemaVersion":1,"id":opid,"profileId":pid,"revision":2,"createdAt":"2026-10-23T12:00:00Z","kind":"chat","status":"succeeded","finishedAt":"2026-10-23T12:00:01Z","capacityAttemptCount":0,"physicalCallCount":1,"limits":{"maxPhysicalCalls":4,"maxElapsedMs":180000},"recordRefs":[],"result":result});assert!(schema::dto("Operation",&op),"invalid operation {op}");
  let event=json!({"contractVersion":1,"eventId":format!("event-{opid}"),"profileId":pid,"operationId":opid,"sequence":sequence,"eventType":"operation.changed","occurredAt":"2026-10-23T12:00:01Z","payload":op});assert!(schema::dto("EventEnvelope",&event),"invalid event {event}");
  sqlx::query("INSERT INTO profile_events(profile_id,sequence,body) VALUES(?,?,?)").bind(pid).bind(sequence).bind(event.to_string()).execute(pool).await.unwrap();
 }
 let base_a:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM profile_events WHERE profile_id=?").bind(&aid).fetch_one(&pool).await.unwrap();
 insert_event(&pool,&aid,"deleted-chat",base_a+1,json!({"kind":"chat","conversationId":cid,"userMessageId":"u","assistantMessageId":"a"})).await;
 insert_event(&pool,&aid,"later-safe",base_a+2,json!({"kind":"chat","conversationId":"unrelated","userMessageId":"u2","assistantMessageId":"a2"})).await;
 let base_b:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM profile_events WHERE profile_id=?").bind(&bid).fetch_one(&pool).await.unwrap();
 insert_event(&pool,&bid,"b-safe",base_b+1,json!({"kind":"chat","conversationId":"b-unrelated","userMessageId":"ub","assistantMessageId":"ab"})).await;
 let archived=app.dispatch(json!({"contractVersion":1,"requestId":"archive-delete-source","command":"conversation.archive","profileId":aid,"expectedRevision":crev,"payload":{"conversationId":cid,"archived":true}})).await.response;assert_eq!(archived["ok"],true,"{archived}");
 let deleted=app.dispatch(json!({"contractVersion":1,"requestId":"delete-source","command":"conversation.delete.permanent","profileId":aid,"expectedRevision":archived["result"]["revision"],"payload":{"conversationId":cid,"confirm":true}})).await.response;assert_eq!(deleted["ok"],true,"{deleted}");
 let first=app.dispatch(req("activity-after-delete","activity.list",json!({"afterSequence":base_a,"limit":1}),Some(&aid))).await.response;assert_eq!(first["ok"],true,"{first}");assert_eq!(first["result"]["items"].as_array().unwrap().len(),1);assert_eq!(first["result"]["items"][0]["operationId"],"later-safe");assert_eq!(first["result"]["lastSequence"],base_a+2);
 let foreign=app.dispatch(req("activity-b-safe","activity.list",json!({"afterSequence":base_b,"limit":1}),Some(&bid))).await.response;assert_eq!(foreign["ok"],true,"{foreign}");assert_eq!(foreign["result"]["items"][0]["operationId"],"b-safe");
 drop(app);pool.close().await;
 let app=Application::new(Store::open(&path).await.unwrap());let replay=app.dispatch(req("activity-after-delete-restart","activity.list",json!({"afterSequence":base_a,"limit":1}),Some(&aid))).await.response;assert_eq!(replay["ok"],true,"{replay}");assert_eq!(replay["result"]["items"][0]["operationId"],"later-safe");
 drop(app);let _=std::fs::remove_file(&path);let _=std::fs::remove_file(path.with_extension("sqlite3-wal"));let _=std::fs::remove_file(path.with_extension("sqlite3-shm"));
}
