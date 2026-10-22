use ai_home::{application::Application,persistence::Store,vault::{SecretStore,Vault,VaultError}};
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
