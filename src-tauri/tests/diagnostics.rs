use ai_home::{application::Application,persistence::Store};
use base64::{engine::general_purpose::STANDARD,Engine as _};
use serde_json::{json,Value};
fn req(id:&str,command:&str)->Value{json!({"contractVersion":1,"requestId":id,"command":command,"payload":{}})}
#[tokio::test]
async fn diagnostics_export_is_content_free_and_reports_aggregate_errors(){
 let store=Store::memory().await.unwrap();let pool=store.pool.clone();let app=Application::new(store);
 let marker="PRIVATE_DIAGNOSTIC_MARKER_9F2A";
 sqlx::query("INSERT INTO profiles(id,revision,body) VALUES('p',1,?)").bind(json!({"id":"p","revision":1,"displayName":marker,"status":"draft"}).to_string()).execute(&pool).await.unwrap();
 sqlx::query("INSERT INTO conversations(profile_id,id,revision,body) VALUES('p','c',1,?)").bind(json!({"id":"c","profileId":"p","revision":1,"title":marker,"language":"en","draftText":marker,"status":"open"}).to_string()).execute(&pool).await.unwrap();
 sqlx::query("INSERT INTO memory_records(profile_id,id,revision,body) VALUES('p','m',1,?)").bind(json!({"id":"m","profileId":"p","revision":1,"kind":"working","state":"active","content":marker}).to_string()).execute(&pool).await.unwrap();
 sqlx::query("INSERT INTO connections(profile_id,id,revision,body) VALUES('p','cn',1,?)").bind(json!({"id":"cn","profileId":"p","revision":1,"normalizedEndpoint":marker,"credentialRef":marker}).to_string()).execute(&pool).await.unwrap();
 sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES('p','op',1,?)").bind(json!({"id":"op","profileId":"p","revision":1,"status":"failed","error":{"code":"PROVIDER_TIMEOUT","messageKey":"error.provider_timeout","retryable":true}}).to_string()).execute(&pool).await.unwrap();
 let snap=app.dispatch(req("d1","diagnostics.snapshot.get")).await.response;assert_eq!(snap["ok"],true,"{snap}");assert_eq!(snap["result"]["contentIncluded"],false);assert_eq!(snap["result"]["counts"]["profiles"],1);assert_eq!(snap["result"]["recentErrors"][0]["code"],"PROVIDER_TIMEOUT");
 let export=app.dispatch(req("d2","diagnostics.export.create")).await.response;assert_eq!(export["ok"],true,"{export}");let raw=STANDARD.decode(export["result"]["dataBase64"].as_str().unwrap()).unwrap();let text=String::from_utf8(raw).unwrap();assert!(!text.contains(marker),"diagnostics leaked private content: {text}");for forbidden in ["credentialRef","normalizedEndpoint","messages","memory_versions","profileId"]{assert!(!text.contains(forbidden),"diagnostics leaked field {forbidden}: {text}");}assert!(text.contains("PROVIDER_TIMEOUT"));assert!(text.contains("\"contentIncluded\": false"));
}
