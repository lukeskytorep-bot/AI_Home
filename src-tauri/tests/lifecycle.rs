use ai_home::{application::Application,domain::schema,persistence::Store,vault::{SecretStore,Vault,VaultError}};
use serde_json::{json,Value};
use std::{collections::HashMap,sync::{Arc,Mutex}};
#[derive(Default)]struct MemoryVault{values:Mutex<HashMap<String,String>>}
impl SecretStore for MemoryVault{
 fn read(&self,id:&str)->Result<Option<String>,VaultError>{Ok(self.values.lock().unwrap().get(id).cloned())}
 fn write_new(&self,id:&str,value:&str)->Result<(),VaultError>{let mut m=self.values.lock().unwrap();if m.contains_key(id){return Err(VaultError::Exists)}m.insert(id.into(),value.into());Ok(())}
}
fn req(command:&str,payload:Value,profile:Option<&str>,expected:Option<i64>)->Value{let mut r=json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});if let Some(p)=profile{r["profileId"]=json!(p)}if let Some(e)=expected{r["expectedRevision"]=json!(e)}r}
async fn call(app:&Application,command:&str,payload:Value,profile:Option<&str>,expected:Option<i64>)->Value{app.dispatch(req(command,payload,profile,expected)).await.response}
async fn ok(app:&Application,command:&str,payload:Value,profile:Option<&str>,expected:Option<i64>)->Value{let r=call(app,command,payload,profile,expected).await;assert_eq!(r["ok"],true,"{r}");r["result"].clone()}
async fn setup()->Application{let store=Store::memory().await.unwrap();Application::with_vault(store,Vault::with_backend(Arc::new(MemoryVault::default())))}
async fn profile(app:&Application,name:&str)->Value{ok(app,"profile.create",json!({"displayName":name}),None,None).await}
async fn conversation(app:&Application,p:&str,title:&str)->Value{ok(app,"conversation.create",json!({"title":title,"language":"en"}),Some(p),None).await}

#[tokio::test]
async fn conversation_archive_is_reversible_and_preserves_raw_record(){
 let app=setup().await;let p=profile(&app,"One").await;let pid=p["id"].as_str().unwrap();let c=conversation(&app,pid,"Keep me").await;let cid=c["id"].as_str().unwrap();
 let saved=ok(&app,"conversation.draft.set",json!({"conversationId":cid,"text":"raw draft"}),Some(pid),Some(1)).await;
 let archived=ok(&app,"conversation.archive",json!({"conversationId":cid,"archived":true}),Some(pid),Some(saved["revision"].as_i64().unwrap())).await;assert_eq!(archived["status"],"archived");
 let active=ok(&app,"conversation.list",json!({"page":{"limit":50}}),Some(pid),None).await;assert!(active["items"].as_array().unwrap().is_empty());
 let archive=ok(&app,"conversation.archived.list",json!({"page":{"limit":50}}),Some(pid),None).await;assert_eq!(archive["items"].as_array().unwrap().len(),1);assert_eq!(archive["items"][0]["draftText"],"raw draft");
 let restored=ok(&app,"conversation.archive",json!({"conversationId":cid,"archived":false}),Some(pid),Some(archived["revision"].as_i64().unwrap())).await;assert_eq!(restored["status"],"open");assert_eq!(restored["draftText"],"raw draft");
 let active=ok(&app,"conversation.list",json!({"page":{"limit":50}}),Some(pid),None).await;assert_eq!(active["items"].as_array().unwrap().len(),1);
}

#[tokio::test]
async fn profile_archive_hides_only_that_profile_and_restore_recomputes_state(){
 let app=setup().await;let p1=profile(&app,"One").await;let p2=profile(&app,"Two").await;let a=p1["id"].as_str().unwrap();let b=p2["id"].as_str().unwrap();let c=conversation(&app,a,"Owned").await;
 let archived=ok(&app,"profile.archive",json!({"archived":true}),Some(a),Some(p1["revision"].as_i64().unwrap())).await;assert_eq!(archived["status"],"archived");
 let active=ok(&app,"profile.list",json!({"limit":100}),None,None).await;let ids:Vec<_>=active["items"].as_array().unwrap().iter().map(|v|v["id"].as_str().unwrap()).collect();assert!(!ids.contains(&a));assert!(ids.contains(&b));
 let archive=ok(&app,"profile.archived.list",json!({"limit":100}),None,None).await;assert_eq!(archive["items"].as_array().unwrap().len(),1);assert_eq!(archive["items"][0]["id"],a);
 let denied=call(&app,"conversation.get",json!({"conversationId":c["id"]}),Some(a),None).await;assert_eq!(denied["ok"],false);assert_eq!(denied["error"]["code"],"POLICY_DENIED");
 let restored=ok(&app,"profile.archive",json!({"archived":false}),Some(a),Some(archived["revision"].as_i64().unwrap())).await;assert_eq!(restored["status"],"draft");
 let got=ok(&app,"conversation.get",json!({"conversationId":c["id"]}),Some(a),None).await;assert_eq!(got["title"],"Owned");
}

#[tokio::test]
async fn archive_scope_and_cursors_do_not_cross(){
 let app=setup().await;let p1=profile(&app,"One").await;let p2=profile(&app,"Two").await;let a=p1["id"].as_str().unwrap();let b=p2["id"].as_str().unwrap();
 let c=conversation(&app,a,"A").await;let wrong=call(&app,"conversation.archive",json!({"conversationId":c["id"],"archived":true}),Some(b),Some(1)).await;assert_eq!(wrong["ok"],false);
 let page=ok(&app,"profile.list",json!({"limit":1}),None,None).await;let cursor=page["nextCursor"].as_str().unwrap();if !cursor.is_empty(){let misuse=call(&app,"profile.archived.list",json!({"limit":1,"cursor":cursor}),None,None).await;assert_eq!(misuse["ok"],false);assert_eq!(misuse["error"]["code"],"VALIDATION_ERROR");}
}


#[tokio::test]
async fn permanent_conversation_delete_requires_archive_and_cas_and_removes_dependents(){
 let app=setup().await;let p=profile(&app,"One").await;let pid=p["id"].as_str().unwrap();let c=conversation(&app,pid,"Delete me").await;let cid=c["id"].as_str().unwrap();
 let denied=call(&app,"conversation.delete.permanent",json!({"conversationId":cid,"confirm":true}),Some(pid),Some(c["revision"].as_i64().unwrap())).await;assert_eq!(denied["ok"],false);assert_eq!(denied["error"]["code"],"POLICY_DENIED");
 let archived=ok(&app,"conversation.archive",json!({"conversationId":cid,"archived":true}),Some(pid),Some(c["revision"].as_i64().unwrap())).await;
 let stale=call(&app,"conversation.delete.permanent",json!({"conversationId":cid,"confirm":true}),Some(pid),Some(archived["revision"].as_i64().unwrap()-1)).await;assert_eq!(stale["ok"],false);assert_eq!(stale["error"]["code"],"REVISION_CONFLICT");
 let rid=uuid::Uuid::new_v4().to_string();let r=json!({"contractVersion":1,"requestId":rid,"command":"conversation.delete.permanent","profileId":pid,"expectedRevision":archived["revision"],"payload":{"conversationId":cid,"confirm":true}});let first=app.dispatch(r.clone()).await.response;assert_eq!(first["ok"],true,"{first}");assert_eq!(first["result"]["deleted"],true);
 let replay=app.dispatch(r).await.response;assert_eq!(replay,first);
 let missing=call(&app,"conversation.get",json!({"conversationId":cid}),Some(pid),None).await;assert_eq!(missing["ok"],false);assert_eq!(missing["error"]["code"],"NOT_FOUND");
}

#[tokio::test]
async fn permanent_profile_delete_requires_archived_profile_and_is_idempotent(){
 let app=setup().await;let p=profile(&app,"Gone").await;let pid=p["id"].as_str().unwrap();let _c=conversation(&app,pid,"Owned").await;
 let denied=call(&app,"profile.delete.permanent",json!({"confirm":true}),Some(pid),Some(p["revision"].as_i64().unwrap())).await;assert_eq!(denied["ok"],false);assert_eq!(denied["error"]["code"],"POLICY_DENIED");
 let archived=ok(&app,"profile.archive",json!({"archived":true}),Some(pid),Some(p["revision"].as_i64().unwrap())).await;
 let rid=uuid::Uuid::new_v4().to_string();let r=json!({"contractVersion":1,"requestId":rid,"command":"profile.delete.permanent","profileId":pid,"expectedRevision":archived["revision"],"payload":{"confirm":true}});let first=app.dispatch(r.clone()).await.response;assert_eq!(first["ok"],true,"{first}");let replay=app.dispatch(r).await.response;assert_eq!(replay,first);
 let archive=ok(&app,"profile.archived.list",json!({"limit":100}),None,None).await;assert!(!archive["items"].as_array().unwrap().iter().any(|x|x["id"]==pid));
}

#[tokio::test]
async fn permanent_conversation_delete_redacts_event_payloads_and_tombstones_old_receipts(){
 let store=Store::memory().await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(Arc::new(MemoryVault::default())));
 let p=profile(&app,"One").await;let pid=p["id"].as_str().unwrap().to_string();
 let create_id=uuid::Uuid::new_v4().to_string();
 let create_req=json!({"contractVersion":1,"requestId":create_id,"command":"conversation.create","profileId":pid,"payload":{"title":"DELETE_TITLE_MARKER","language":"en"}});
 let created=app.dispatch(create_req.clone()).await.response;assert_eq!(created["ok"],true,"{created}");let c=created["result"].clone();let cid=c["id"].as_str().unwrap().to_string();
 let drafted=ok(&app,"conversation.draft.set",json!({"conversationId":cid,"text":"DELETE_DRAFT_MARKER"}),Some(&pid),Some(c["revision"].as_i64().unwrap())).await;
 let message=json!({"schemaVersion":1,"id":uuid::Uuid::new_v4().to_string(),"profileId":pid,"conversationId":cid,"sequence":1,"revision":1,"createdAt":"2026-10-23T00:00:00Z","role":"human","content":"DELETED_MESSAGE_MARKER"});
 sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,?,?,1,1,?)").bind(&pid).bind(message["id"].as_str().unwrap()).bind(&cid).bind(message.to_string()).execute(&pool).await.unwrap();
 let seq:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?").bind(&pid).fetch_one(&pool).await.unwrap();
 let event=json!({"contractVersion":1,"eventId":"delete-test-event","sequence":seq,"eventType":"message.created","occurredAt":"2026-10-23T00:00:00Z","payload":message});
 sqlx::query("INSERT INTO profile_events(profile_id,sequence,body) VALUES(?,?,?)").bind(&pid).bind(seq).bind(event.to_string()).execute(&pool).await.unwrap();
 let archived=ok(&app,"conversation.archive",json!({"conversationId":cid,"archived":true}),Some(&pid),Some(drafted["revision"].as_i64().unwrap())).await;
 let deleted=ok(&app,"conversation.delete.permanent",json!({"conversationId":cid,"confirm":true}),Some(&pid),Some(archived["revision"].as_i64().unwrap())).await;assert_eq!(deleted["deleted"],true);
 let event_bodies:Vec<String>=sqlx::query_scalar("SELECT body FROM profile_events WHERE profile_id=? ORDER BY sequence").bind(&pid).fetch_all(&pool).await.unwrap();let joined=event_bodies.join("\n");
 assert!(!joined.contains("DELETED_MESSAGE_MARKER"));assert!(!joined.contains("DELETE_TITLE_MARKER"));assert!(!joined.contains("DELETE_DRAFT_MARKER"));assert!(joined.contains("permanent_deletion"));
 let receipt_bodies:Vec<String>=sqlx::query_scalar("SELECT response FROM receipts").fetch_all(&pool).await.unwrap();let receipts_joined=receipt_bodies.join("\n");assert!(!receipts_joined.contains("DELETE_TITLE_MARKER"));assert!(!receipts_joined.contains("DELETE_DRAFT_MARKER"));
 let old_create=app.dispatch(create_req.clone()).await.response;assert_eq!(old_create["ok"],false,"{old_create}");assert_eq!(old_create["error"]["code"],"POLICY_DENIED");assert_eq!(old_create["error"]["messageKey"],"error.permanently_deleted");
 let mut conflict=create_req;conflict["payload"]["title"]=json!("different");let conflict=app.dispatch(conflict).await.response;assert_eq!(conflict["ok"],false);assert_eq!(conflict["error"]["code"],"IDEMPOTENCY_CONFLICT");
}

#[tokio::test]
async fn permanent_profile_delete_never_tombstones_foreign_receipt_by_text_match(){
 let store=Store::memory().await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(Arc::new(MemoryVault::default())));
 let create_a_id=uuid::Uuid::new_v4().to_string();let create_a=json!({"contractVersion":1,"requestId":create_a_id,"command":"profile.create","payload":{"displayName":"A"}});let a=app.dispatch(create_a.clone()).await.response;assert_eq!(a["ok"],true);let aid=a["result"]["id"].as_str().unwrap().to_string();
 let b=profile(&app,"B").await;let bid=b["id"].as_str().unwrap().to_string();
 let b_create_id=uuid::Uuid::new_v4().to_string();let b_create=json!({"contractVersion":1,"requestId":b_create_id,"command":"conversation.create","profileId":bid,"payload":{"title":format!("Discussion about {aid}"),"language":"en"}});let first_b=app.dispatch(b_create.clone()).await.response;assert_eq!(first_b["ok"],true,"{first_b}");
 let archived=ok(&app,"profile.archive",json!({"archived":true}),Some(&aid),Some(a["result"]["revision"].as_i64().unwrap())).await;let del=ok(&app,"profile.delete.permanent",json!({"confirm":true}),Some(&aid),Some(archived["revision"].as_i64().unwrap())).await;assert_eq!(del["deleted"],true);
 let replay_b=app.dispatch(b_create.clone()).await.response;assert_eq!(replay_b,first_b);
 let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE profile_id=?").bind(&bid).fetch_one(&pool).await.unwrap();assert_eq!(count,1);
 let replay_a=app.dispatch(create_a.clone()).await.response;assert_eq!(replay_a["ok"],false);assert_eq!(replay_a["error"]["code"],"POLICY_DENIED");assert_eq!(replay_a["error"]["messageKey"],"error.permanently_deleted");
 let mut conflict=create_a;conflict["payload"]["displayName"]=json!("A2");let conflict=app.dispatch(conflict).await.response;assert_eq!(conflict["error"]["code"],"IDEMPOTENCY_CONFLICT");
}

#[tokio::test]
async fn permanent_conversation_delete_redacts_memory_run_sources_and_cancels_active_memory_operation(){
 let store=Store::memory().await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(Arc::new(MemoryVault::default())));
 let p=profile(&app,"Memory owner").await;let pid=p["id"].as_str().unwrap().to_string();let c=conversation(&app,&pid,"Memory source").await;let cid=c["id"].as_str().unwrap().to_string();
 let record_id=format!("working-{pid}");
 let record=json!({"schemaVersion":1,"id":record_id,"profileId":pid,"revision":1,"createdAt":"2026-10-23T00:00:00Z","kind":"working","state":"active","activeVersionId":null});
 sqlx::query("UPDATE memory_records SET body=? WHERE profile_id=? AND id=?").bind(record.to_string()).bind(&pid).bind(&record_id).execute(&pool).await.unwrap();
 let opid=uuid::Uuid::new_v4().to_string();let op=json!({"schemaVersion":1,"id":opid,"profileId":pid,"revision":1,"createdAt":"2026-10-23T00:00:00Z","kind":"memory_update","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"expectedRecordRevision":1,"limits":{"maxPhysicalCalls":3,"maxElapsedMs":120000},"recordRefs":[{"profileId":pid,"recordKind":"memory_record","recordId":record_id}]});assert!(schema::dto("Operation",&op),"invalid Operation fixture: {op}");
 sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,?,1,?)").bind(&pid).bind(&opid).bind(op.to_string()).execute(&pool).await.unwrap();
 let source=json!({"id":"memory-source-message","profileId":pid,"conversationId":cid,"sequence":1,"role":"human","content":"RAW_MEMORY_SOURCE_MARKER"});
 let run=json!({"conversationId":cid,"conversationLanguage":"en","messages":[source],"sources":[{"profileId":pid,"kind":"message","entityId":"memory-source-message","contentSha256":"deadbeef"}],"formatRepairCount":0});
 sqlx::query("INSERT INTO memory_runs(profile_id,operation_id,record_id,body) VALUES(?,?,?,?)").bind(&pid).bind(&opid).bind(&record_id).bind(run.to_string()).execute(&pool).await.unwrap();
 let archived=ok(&app,"conversation.archive",json!({"conversationId":cid,"archived":true}),Some(&pid),Some(c["revision"].as_i64().unwrap())).await;
 let operation_body:String=sqlx::query_scalar("SELECT body FROM operations WHERE profile_id=? AND id=?").bind(&pid).bind(&opid).fetch_one(&pool).await.unwrap();let operation:Value=serde_json::from_str(&operation_body).unwrap();assert_eq!(operation["status"],"cancelled");
 let _deleted=ok(&app,"conversation.delete.permanent",json!({"conversationId":cid,"confirm":true}),Some(&pid),Some(archived["revision"].as_i64().unwrap())).await;
 let stored:String=sqlx::query_scalar("SELECT body FROM memory_runs WHERE profile_id=? AND operation_id=?").bind(&pid).bind(&opid).fetch_one(&pool).await.unwrap();assert!(!stored.contains("RAW_MEMORY_SOURCE_MARKER"));let run:Value=serde_json::from_str(&stored).unwrap();assert_eq!(run["messages"],json!([]));assert_eq!(run["sources"],json!([]));assert!(run["conversationId"].is_null());assert_eq!(run["sourceTombstone"]["resumable"],false);
 let record_count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM memory_records WHERE profile_id=? AND id=?").bind(&pid).bind(&record_id).fetch_one(&pool).await.unwrap();assert_eq!(record_count,1);
}
