include!("support/memory_setup.rs");
async fn memory_view(app: &Application, p: &str) -> Value {
    ok(app, "memory.working.get", json!({}), Some(p), None).await
}
async fn memory_settings(app: &Application, p: &str, capacity: Value, mode: &str) -> Value {
    let v = memory_view(app, p).await;
    ok(app,"memory.settings.set",json!({"scopeId":v["scopeId"],"capacityPolicy":capacity,"scribeMode":mode,"primaryContextTokens":32768,"scribeContextTokens":32768}),Some(p),v["profileRevision"].as_i64()).await
}
async fn memory_request(app: &Application, p: &str, c: &Value) -> Value {
    let src = ok(
        app,
        "memory.working.sources",
        json!({"conversationId":c["id"]}),
        Some(p),
        None,
    )
    .await;
    request(
        "memory.update.request",
        json!({"recordId":src["recordId"],"sourceRefs":src["sourceRefs"]}),
        Some(p),
        src["recordRevision"].as_i64(),
    )
}
fn answer(s: &Server, content: Value) {
    s.responses.lock().unwrap().push_back((
        200,
        json!({"choices":[{"message":{"content":content.to_string()},"finish_reason":"stop"}]})
            .to_string(),
    ));
}
fn propose_answer(s: &Server, text: &str) {
    answer(
        s,
        json!({"content":text,"language":"pl","claimKind":"mixed"}),
    );
}
fn decide_answer(s: &Server, kind: &str, text: &str) {
    if kind == "modify" {
        answer(
            s,
            json!({"decision":kind,"modifiedContent":text,"reason":"Jawna korekta."}),
        );
    } else {
        answer(s, json!({"decision":kind,"reason":"Jawna decyzja."}));
    }
}
async fn memory_fixture() -> (
    Application,
    sqlx::SqlitePool,
    Arc<MemoryVault>,
    Server,
    String,
    Value,
    Value,
) {
    let (app, pool, vault, s, p, c, conn) = configured().await;
    let r = app
        .dispatch(send_request(&p, &c, "Budujemy AI Home. Zadanie: pamięć."))
        .await
        .response;
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(
        finished(&app, &p, &r["result"]["operation"]["id"]).await["status"],
        "succeeded"
    );
    memory_settings(&app, &p, json!({"kind":"bounded","tokens":300}), "disabled").await;
    s.wires.lock().unwrap().clear();
    (app, pool, vault, s, p, c, conn)
}
async fn update(app: &Application, p: &str, c: &Value) -> Value {
    let req = memory_request(app, p, c).await;
    let a = app.dispatch(req).await.response;
    assert_eq!(a["ok"], true, "{a}");
    finished(app, p, &a["result"]["operation"]["id"]).await
}
async fn wait_posts(s: &Server, n: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while s.wires.lock().unwrap().len() < n {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

async fn fresh(app:&Application,p:&str,c:&Value)->Value {ok(app,"conversation.get",json!({"conversationId":c["id"]}),Some(p),None).await}
async fn preflight(app:&Application,p:&str,c:&Value,text:&str)->Value {
    let r=send_request(p,c,text);ok(app,"chat.preflight",r["payload"].clone(),Some(p),c["revision"].as_i64()).await
}
async fn context(app:&Application,p:&str,op:&Value)->Value {ok(app,"chat.context.get",json!({"operationId":op["id"]}),Some(p),None).await}
async fn save_memory(app:&Application,s:&Server,p:&str,c:&Value,text:&str)->Value {
    propose_answer(s,text);decide_answer(s,"accept","");let op=update(app,p,c).await;assert_eq!(op["status"],"succeeded","{op}");memory_view(app,p).await["version"].clone()
}

async fn chat(app:&Application,p:&str,c:&Value,text:&str)->Value {let a=app.dispatch(send_request(p,c,text)).await.response;assert_eq!(a["ok"],true,"{a}");finished(app,p,&a["result"]["operation"]["id"]).await}


async fn listing(app:&Application,p:&str,record:&str,limit:i64,cursor:Option<&str>)->Value{
 let mut page=json!({"limit":limit});if let Some(c)=cursor{page["cursor"]=json!(c);}
 ok(app,"memory.versions.list",json!({"recordId":record,"page":page}),Some(p),None).await
}
async fn detail(app:&Application,p:&str,record:&str,version:&Value)->Value{ok(app,"memory.version.get",json!({"recordId":record,"versionId":version["id"]}),Some(p),None).await}
async fn toggle(app:&Application,p:&str,outdated:bool)->Value{let v=memory_view(app,p).await;ok(app,"memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":outdated}),Some(p),v["record"]["revision"].as_i64()).await}
#[tokio::test]
async fn unconfigured_empty_history_is_local_and_readable_even_if_vault_is_unavailable(){
 let(app,pool,vault)=setup().await;let p=profile(&app,"No provider").await;let pid=p["id"].as_str().unwrap();let row:String=sqlx::query_scalar("SELECT body FROM memory_records WHERE profile_id=?").bind(pid).fetch_one(&pool).await.unwrap();let record:Value=serde_json::from_str(&row).unwrap();vault.fail.store(true,Ordering::SeqCst);
 let page=listing(&app,pid,record["id"].as_str().unwrap(),10,None).await;assert_eq!(page,json!({"items":[],"nextCursor":""}));let a=invoke(&app,"memory.outdated.set",json!({"recordId":record["id"],"outdated":true}),Some(pid),Some(1)).await;assert_eq!(a["error"]["code"],"POLICY_DENIED");assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM memory_versions").fetch_one(&pool).await.unwrap(),0);
}
#[tokio::test]
async fn history_reads_all_versions_and_primary_receipts_without_writes_or_model_calls(){
 let(app,pool,vault,s,p,c,_)=memory_fixture().await;let first=save_memory(&app,&s,&p,&c,"Cel pierwszy.").await;let second=save_memory(&app,&s,&p,&c,"Cel drugi.").await;let v=memory_view(&app,&p).await;let rid=v["record"]["id"].as_str().unwrap();let before=s.wires.lock().unwrap().len();let receipts:i64=sqlx::query_scalar("SELECT count(*) FROM receipts").fetch_one(&pool).await.unwrap();let events:i64=sqlx::query_scalar("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap();vault.fail.store(true,Ordering::SeqCst);
 let page=listing(&app,&p,rid,10,None).await;assert_eq!(page["items"],json!([second,first]));let d=detail(&app,&p,rid,&page["items"][1]).await;assert_eq!(d["version"],first);assert_eq!(d["decision"]["id"],first["decisionReceiptId"]);assert_eq!(d["decision"]["decision"],"accept");assert_eq!(d["decision"]["candidateSha256"],first["contentSha256"]);assert_eq!(s.wires.lock().unwrap().len(),before);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM receipts").fetch_one(&pool).await.unwrap(),receipts);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap(),events);
}
#[tokio::test]
async fn pagination_is_scoped_and_stable_when_new_versions_arrive(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let one=save_memory(&app,&s,&p,&c,"One").await;let two=save_memory(&app,&s,&p,&c,"Two").await;let three=save_memory(&app,&s,&p,&c,"Three").await;let v=memory_view(&app,&p).await;let rid=v["record"]["id"].as_str().unwrap();let first=listing(&app,&p,rid,1,None).await;assert_eq!(first["items"],json!([three]));let cursor=first["nextCursor"].as_str().unwrap();assert!(!cursor.is_empty());save_memory(&app,&s,&p,&c,"Four").await;
 let second=listing(&app,&p,rid,1,Some(cursor)).await;assert_eq!(second["items"],json!([two]));assert_eq!(listing(&app,&p,rid,1,Some(cursor)).await,second);let last=listing(&app,&p,rid,1,second["nextCursor"].as_str()).await;assert_eq!(last,json!({"items":[one],"nextCursor":""}));
 let q=profile(&app,"Other").await;let qp=q["id"].as_str().unwrap();let qr=memory_view(&app,qp).await;
 for (pid,record,limit,token) in [(qp,qr["record"]["id"].as_str().unwrap(),1,cursor),(&p,rid,2,cursor),(&p,rid,1,"not a cursor")]{let a=invoke(&app,"memory.versions.list",json!({"recordId":record,"page":{"limit":limit,"cursor":token}}),Some(pid),None).await;assert_eq!(a["error"]["code"],"VALIDATION_ERROR","{a}");}
}
#[tokio::test]
async fn old_view_cannot_exclude_a_newly_approved_version_and_receipt_replay_is_exact(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;let one=save_memory(&app,&s,&p,&c,"One").await;let old=memory_view(&app,&p).await;save_memory(&app,&s,&p,&c,"Two").await;let v=memory_view(&app,&p).await;let before:i64=sqlx::query_scalar("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap();let a=invoke(&app,"memory.outdated.set",json!({"recordId":old["record"]["id"],"outdated":true}),Some(&p),old["record"]["revision"].as_i64()).await;assert_eq!(a["error"]["code"],"REVISION_CONFLICT");assert_eq!(memory_view(&app,&p).await["record"],v["record"]);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap(),before);
 let req=request("memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":true}),Some(&p),v["record"]["revision"].as_i64());let a=app.dispatch(req.clone()).await.response;assert_eq!(a["ok"],true);assert_eq!(a["result"]["revision"],v["record"]["revision"].as_i64().unwrap()+1);let events:i64=sqlx::query_scalar("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap();assert_eq!(app.dispatch(req.clone()).await.response,a);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap(),events);
 let mut changed=req;changed["payload"]["outdated"]=json!(false);assert_eq!(app.dispatch(changed).await.response["error"]["code"],"IDEMPOTENCY_CONFLICT");assert_eq!(detail(&app,&p,old["record"]["id"].as_str().unwrap(),&one).await["version"],one);
}
#[tokio::test]
async fn exclusion_and_reenable_preserve_version_and_change_actual_chat_context(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Cel tylko w pamięci: 54321.").await;let before=memory_view(&app,&p).await;let posts=s.wires.lock().unwrap().len();let off=toggle(&app,&p,true).await;assert_eq!(off["state"],"outdated");assert_eq!(off["activeVersionId"],version["id"]);let c=fresh(&app,&p,&c).await;let f=preflight(&app,&p,&c,"Without").await;assert_eq!(f["workingContext"]["included"],false);let op=chat(&app,&p,&c,"Without").await;assert_eq!(op["status"],"succeeded");assert!(!s.wires.lock().unwrap().last().unwrap().contains("54321"));assert_eq!(memory_view(&app,&p).await["version"],version);
 let on=toggle(&app,&p,false).await;assert_eq!(on["state"],"active");assert_eq!(on["activeVersionId"],version["id"]);let c=fresh(&app,&p,&c).await;let f=preflight(&app,&p,&c,"With").await;assert_eq!(f["workingContext"]["versionId"],version["id"]);assert_eq!(s.wires.lock().unwrap().len(),posts+1);assert_eq!(memory_view(&app,&p).await["version"],before["version"]);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM memory_versions").fetch_one(&pool).await.unwrap(),1);
}
#[tokio::test]
async fn exclusion_during_chat_blocks_late_answer_and_keeps_historical_context(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Exclude in flight").await;s.wires.lock().unwrap().clear();let c=fresh(&app,&p,&c).await;s.hold.store(true,Ordering::SeqCst);let a=app.dispatch(send_request(&p,&c,"Delayed")).await.response;wait_posts(&s,1).await;toggle(&app,&p,true).await;s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert_eq!(context(&app,&p,&op).await["workingContext"]["versionId"],version["id"]);assert_eq!(s.wires.lock().unwrap().len(),1);
}
#[tokio::test]
async fn exclusion_during_memory_update_prevents_candidate_activation(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Old committed").await;s.wires.lock().unwrap().clear();propose_answer(&s,"New candidate");s.hold.store(true,Ordering::SeqCst);let a=app.dispatch(memory_request(&app,&p,&c).await).await.response;assert_eq!(a["ok"],true);wait_posts(&s,1).await;toggle(&app,&p,true).await;s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");let v=memory_view(&app,&p).await;assert_eq!(v["record"]["state"],"outdated");assert_eq!(v["version"],version);
}
#[tokio::test]
async fn foreign_record_version_and_forged_reenable_never_disclose_or_activate_data(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Private content").await;let v=memory_view(&app,&p).await;let q=profile(&app,"Other").await;let qp=q["id"].as_str().unwrap();let qr=memory_view(&app,qp).await;
 for (cmd,payload,pid,rev) in [("memory.versions.list",json!({"recordId":v["record"]["id"],"page":{"limit":10}}),qp,None),("memory.version.get",json!({"recordId":qr["record"]["id"],"versionId":version["id"]}),qp,None),("memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":true}),qp,Some(2))]{let a=invoke(&app,cmd,payload,Some(pid),rev).await;assert_eq!(a["ok"],false);assert!(!a.to_string().contains("Private content"));}
 toggle(&app,&p,true).await;sqlx::query("UPDATE memory_decisions SET body=json_set(body,'$.decision','reject') WHERE id=?").bind(version["decisionReceiptId"].as_str().unwrap()).execute(&pool).await.unwrap();let r=invoke(&app,"memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":false}),Some(&p),Some(3)).await;assert_eq!(r["error"]["code"],"STORAGE_ERROR");let body:String=sqlx::query_scalar("SELECT body FROM memory_records WHERE profile_id=?").bind(&p).fetch_one(&pool).await.unwrap();assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["state"],"outdated");
}
#[tokio::test]
async fn corrupted_historical_content_is_not_returned_as_a_valid_version(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Valid").await;save_memory(&app,&s,&p,&c,"Current").await;let v=memory_view(&app,&p).await;sqlx::query("UPDATE memory_versions SET body=json_set(body,'$.content','TAMPERED') WHERE id=?").bind(version["id"].as_str().unwrap()).execute(&pool).await.unwrap();for(cmd,payload)in [("memory.version.get",json!({"recordId":v["record"]["id"],"versionId":version["id"]})),("memory.versions.list",json!({"recordId":v["record"]["id"],"page":{"limit":10}}))]{let a=invoke(&app,cmd,payload,Some(&p),None).await;assert_eq!(a["error"]["code"],"STORAGE_ERROR");assert!(!a.to_string().contains("TAMPERED"));}
}
#[tokio::test]
async fn event_or_receipt_failure_rolls_back_state_revision_and_notifications(){
 for table in ["receipts","profile_events"] {
 let(app,pool,_,s,p,c,_)=memory_fixture().await;save_memory(&app,&s,&p,&c,"Atomic").await;let before=memory_view(&app,&p).await;let events:i64=sqlx::query_scalar("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap();sqlx::query(&format!("CREATE TRIGGER fail_history BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'injected'); END")).execute(&pool).await.unwrap();let a=invoke(&app,"memory.outdated.set",json!({"recordId":before["record"]["id"],"outdated":true}),Some(&p),before["record"]["revision"].as_i64()).await;assert_eq!(a["error"]["code"],"STORAGE_ERROR");assert_eq!(memory_view(&app,&p).await["record"],before["record"]);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap(),events);
 }
}
#[tokio::test]
async fn file_restart_preserves_exclusion_and_pagination_cursor_without_network(){
 let path=std::env::temp_dir().join(format!("ai-home-history-{}.sqlite",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let vault=Arc::new(MemoryVault::default());let app=Application::with_vault(store,Vault::with_backend(vault.clone()));let(app,pool,vault,s,p,c,_)=configure(app,pool,vault).await;
 chat(&app,&p,&c,"History source").await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"disabled").await;let one=save_memory(&app,&s,&p,&c,"One").await;save_memory(&app,&s,&p,&c,"Two").await;let off=toggle(&app,&p,true).await;let rid=off["id"].as_str().unwrap();let page=listing(&app,&p,rid,1,None).await;let posts=s.wires.lock().unwrap().len();drop(app);pool.close().await;
 let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault));assert_eq!(memory_view(&app,&p).await["record"],off);let older=listing(&app,&p,rid,1,page["nextCursor"].as_str()).await;assert_eq!(older["items"],json!([one]));assert_eq!(s.wires.lock().unwrap().len(),posts);assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap().is_empty());drop(app);pool.close().await;let _=std::fs::remove_file(path);
}
#[tokio::test]
async fn archived_record_is_readable_but_cannot_be_reenabled_by_outdated_toggle(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;let version=save_memory(&app,&s,&p,&c,"Archived").await;let r=memory_view(&app,&p).await["record"].clone();sqlx::query("UPDATE memory_records SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.state','archived') WHERE profile_id=?").bind(&p).execute(&pool).await.unwrap();assert_eq!(listing(&app,&p,r["id"].as_str().unwrap(),10,None).await["items"],json!([version]));let a=invoke(&app,"memory.outdated.set",json!({"recordId":r["id"],"outdated":false}),Some(&p),Some(3)).await;assert_eq!(a["error"]["code"],"POLICY_DENIED");
}

#[tokio::test]
async fn same_state_is_a_noop_and_concurrent_exclusions_have_one_cas_winner(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;save_memory(&app,&s,&p,&c,"Concurrent").await;let v=memory_view(&app,&p).await;let event_count:i64=sqlx::query_scalar("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap();assert_eq!(toggle(&app,&p,false).await,v["record"]);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profile_events").fetch_one(&pool).await.unwrap(),event_count);
 let a=request("memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":true}),Some(&p),v["record"]["revision"].as_i64());let b=request("memory.outdated.set",json!({"recordId":v["record"]["id"],"outdated":true}),Some(&p),v["record"]["revision"].as_i64());let(a,b)=tokio::join!(app.dispatch(a),app.dispatch(b));let replies=[a.response,b.response];assert_eq!(replies.iter().filter(|r|r["ok"]==true).count(),1);assert_eq!(replies.iter().filter(|r|r["error"]["code"]=="REVISION_CONFLICT").count(),1);let after=memory_view(&app,&p).await;assert_eq!(after["record"]["revision"],v["record"]["revision"].as_i64().unwrap()+1);let n:i64=sqlx::query_scalar("SELECT count(*) FROM memory_versions").fetch_one(&pool).await.unwrap();assert_eq!(n,1);
}
#[tokio::test]
async fn modified_memory_keeps_actual_primary_decision_and_historical_measurement(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;propose_answer(&s,"Candidate");decide_answer(&s,"modify","Poprawiony zapis.");assert_eq!(update(&app,&p,&c).await["status"],"succeeded");let v=memory_view(&app,&p).await;let d=detail(&app,&p,v["record"]["id"].as_str().unwrap(),&v["version"]).await;assert_eq!(d["version"]["content"],"Poprawiony zapis.");assert_eq!(d["decision"]["decision"],"modify");assert_eq!(d["decision"]["candidateMeasurement"],d["version"]["measurement"]);
 let settings=ok(&app,"chat.settings.get",json!({}),Some(&p),None).await;ok(&app,"chat.settings.set",json!({"routeId":settings["routeId"],"contextTokens":65536}),Some(&p),settings["routeRevision"].as_i64()).await;assert_eq!(detail(&app,&p,v["record"]["id"].as_str().unwrap(),&v["version"]).await,d);
}
