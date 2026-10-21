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

async fn fixture()->(Application,sqlx::SqlitePool,Arc<MemoryVault>,Server,String,Value,Value,Value){
 let(app,pool,vault,s,p,c,conn)=memory_fixture().await;let old=save_memory(&app,&s,&p,&c,"Stary polski cel.").await;save_memory(&app,&s,&p,&c,"Bieżący cel.").await;s.wires.lock().unwrap().clear();(app,pool,vault,s,p,c,conn,old)
}
async fn req(app:&Application,p:&str,old:&Value)->Value{let v=memory_view(app,p).await;request("memory.restore",json!({"recordId":v["record"]["id"],"versionId":old["id"]}),Some(p),v["record"]["revision"].as_i64())}
async fn restore(app:&Application,p:&str,old:&Value)->Value{let a=app.dispatch(req(app,p,old).await).await.response;assert_eq!(a["ok"],true,"{a}");finished(app,p,&a["result"]["operation"]["id"]).await}
async fn counts(pool:&sqlx::SqlitePool)->Vec<i64>{let mut n=vec![];for t in ["operations","receipts","profile_events","memory_versions","memory_proposals"]{n.push(sqlx::query_scalar::<_,i64>(&format!("SELECT count(*) FROM {t}")).fetch_one(pool).await.unwrap());}n}
#[tokio::test]
async fn accept_restores_as_new_version_with_new_primary_receipt_and_original_provenance(){
 let(app,pool,_,s,p,c,_,old)=fixture().await;let before=memory_view(&app,&p).await;decide_answer(&s,"accept","");let request=req(&app,&p,&old).await;let a=app.dispatch(request.clone()).await.response;assert_eq!(a["ok"],true,"{a}");let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["kind"],"restore");assert_eq!(op["status"],"succeeded","{op}");assert!(schema::dto("Operation",&op));assert!(schema::dto("RestoreOperationResult",&op["result"]));assert_eq!(op["physicalCallCount"],1);assert_eq!(op["result"]["restoredFromVersionId"],old["id"]);
 let v=memory_view(&app,&p).await;assert_eq!(v["version"]["content"],old["content"]);assert_eq!(v["version"]["origin"],"restore");assert_eq!(v["version"]["restoredFromVersionId"],old["id"]);assert_eq!(v["version"]["previousVersionId"],before["version"]["id"]);assert_eq!(v["version"]["versionNumber"],3);assert_ne!(v["version"]["decisionReceiptId"],old["decisionReceiptId"]);assert_eq!(v["version"]["language"],"pl");assert!(v["version"]["sources"].as_array().unwrap().contains(&json!({"profileId":p,"kind":"memory_version","entityId":old["id"],"contentSha256":old["contentSha256"]})));assert_eq!(v["decisions"][0]["operationId"],op["id"]);
 assert_eq!(detail(&app,&p,v["record"]["id"].as_str().unwrap(),&old).await["version"],old);assert_eq!(detail(&app,&p,v["record"]["id"].as_str().unwrap(),&before["version"]).await["version"],before["version"]);assert_eq!(app.dispatch(request).await.response,a);assert_eq!(s.wires.lock().unwrap().len(),1);
 let c=fresh(&app,&p,&c).await;let f=preflight(&app,&p,&c,"Use restored memory").await;assert_eq!(f["workingContext"]["versionId"],v["version"]["id"]);assert_eq!(chat(&app,&p,&c,"Continue").await["status"],"succeeded");assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap().is_empty());
}
#[tokio::test]
async fn primary_modify_saves_actual_text_and_fresh_measurement(){
 let(app,_,_,s,p,_,_,old)=fixture().await;let settings=ok(&app,"chat.settings.get",json!({}),Some(&p),None).await;ok(&app,"chat.settings.set",json!({"routeId":settings["routeId"],"contextTokens":65536}),Some(&p),settings["routeRevision"].as_i64()).await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"disabled").await;
 decide_answer(&s,"modify","Zmieniony cel: Żółć.");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");let v=memory_view(&app,&p).await;assert_eq!(v["version"]["content"],"Zmieniony cel: Żółć.");assert_eq!(v["version"]["measurement"]["value"],"Zmieniony cel: Żółć.".len());assert_ne!(v["version"]["measurement"]["modelRouteRevision"],old["measurement"]["modelRouteRevision"]);assert_eq!(v["decisions"][0]["candidateMeasurement"],v["version"]["measurement"]);assert_eq!(v["version"]["claimKind"],"mixed");
}
#[tokio::test]
async fn reject_preserves_active_and_outdated_state_without_success_result(){
 for outdated in [false,true]{let(app,_,_,s,p,_,_,old)=fixture().await;if outdated{toggle(&app,&p,true).await;}let before=memory_view(&app,&p).await;decide_answer(&s,"reject","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"failed");assert_eq!(op["error"]["code"],"MEMORY_PRIMARY_REJECTED");assert!(op.get("result").is_none());let after=memory_view(&app,&p).await;assert_eq!(after["record"],before["record"]);assert_eq!(after["version"],before["version"]);assert_eq!(after["decisions"][0]["decision"],"reject");}
}
#[tokio::test]
async fn approved_restore_reactivates_outdated_record(){let(app,_,_,s,p,_,_,old)=fixture().await;toggle(&app,&p,true).await;decide_answer(&s,"accept","");assert_eq!(restore(&app,&p,&old).await["status"],"succeeded");assert_eq!(memory_view(&app,&p).await["record"]["state"],"active");}
#[tokio::test]
async fn automatic_scribe_is_not_called_or_required_by_restore(){
 let(app,_,_,s,p,_,conn,old)=fixture().await;let snap=ok(&app,"profile.snapshot",json!({}),Some(&p),None).await;let route=ok(&app,"route.save",json!({"route":{"role":"scribe","connectionId":conn["id"],"modelId":"scribe-model","outputLimitTokens":1024}}),Some(&p),snap["profile"]["revision"].as_i64()).await;let snap=ok(&app,"profile.snapshot",json!({}),Some(&p),None).await;ok(&app,"route.assign",json!({"role":"scribe","routeId":route["id"]}),Some(&p),snap["profile"]["revision"].as_i64()).await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"automatic").await;let v=memory_view(&app,&p).await;ok(&app,"route.save",json!({"routeId":v["scribe"]["id"],"route":{"role":"scribe","connectionId":conn["id"],"modelId":"changed-scribe-without-context","outputLimitTokens":1024}}),Some(&p),v["scribe"]["revision"].as_i64()).await;decide_answer(&s,"accept","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");assert_eq!(s.wires.lock().unwrap().len(),1);let w=s.wires.lock().unwrap();let body:Value=serde_json::from_str(w[0].split("\r\n\r\n").nth(1).unwrap()).unwrap();assert_eq!(body["model"],"mock-model");assert!(body.to_string().contains("memory_restore"));
}
#[tokio::test]
async fn lower_capacity_allows_primary_to_modify_historical_content_without_truncation(){
 let(app,_,_,s,p,_,_,old)=fixture().await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":3}),"disabled").await;decide_answer(&s,"modify","Cel");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");assert_eq!(memory_view(&app,&p).await["version"]["content"],"Cel");assert!(s.wires.lock().unwrap()[0].contains(old["content"].as_str().unwrap()));
}
#[tokio::test]
async fn capacity_failure_gets_second_primary_proposal_and_decision_in_one_budget(){
 let(app,_,_,s,p,_,_,old)=fixture().await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":3}),"disabled").await;decide_answer(&s,"accept","");propose_answer(&s,"Cel");decide_answer(&s,"accept","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");assert_eq!(op["physicalCallCount"],3);assert_eq!(op["capacityAttemptCount"],2);let v=memory_view(&app,&p).await;assert_eq!(v["attempts"][0]["result"],"capacity_rejected");assert_eq!(v["attempts"][1]["result"],"accepted");assert!(s.wires.lock().unwrap()[1].contains("second and final"));
}
#[tokio::test]
async fn two_capacity_failures_and_two_bad_formats_leave_previous_version_intact(){
 for capacity in [true,false]{let(app,_,_,s,p,_,_,old)=fixture().await;let before=memory_view(&app,&p).await;if capacity{memory_settings(&app,&p,json!({"kind":"bounded","tokens":3}),"disabled").await;decide_answer(&s,"accept","");propose_answer(&s,"Still much too long");}else{answer(&s,json!({"garbage":true}));answer(&s,json!({"garbage":true}));}let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"failed");assert_eq!(op["error"]["code"],if capacity{"MEMORY_CAPACITY_EXCEEDED"}else{"MEMORY_FORMAT_INVALID"});assert_eq!(memory_view(&app,&p).await["version"],before["version"]);assert_eq!(s.wires.lock().unwrap().len(),2);}
}
#[tokio::test]
async fn malformed_decision_has_only_one_format_repair(){let(app,_,_,s,p,_,_,old)=fixture().await;answer(&s,json!({"wrong":true}));decide_answer(&s,"accept","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");assert_eq!(op["physicalCallCount"],2);assert_eq!(memory_view(&app,&p).await["formatRepairCount"],1);}
#[tokio::test]
async fn impossible_initial_decision_is_rejected_before_any_paid_call_or_write(){
 let(app,pool,_,s,p,_,_,old)=fixture().await;let v=memory_view(&app,&p).await;ok(&app,"memory.settings.set",json!({"scopeId":v["scopeId"],"capacityPolicy":{"kind":"bounded","tokens":300},"scribeMode":"disabled","primaryContextTokens":1024}),Some(&p),v["profileRevision"].as_i64()).await;let request=req(&app,&p,&old).await;let before=counts(&pool).await;let a=app.dispatch(request).await.response;assert_eq!(a["error"]["code"],"CONTEXT_BUDGET_EXCEEDED","{a}");assert!(s.wires.lock().unwrap().is_empty());assert_eq!(counts(&pool).await,before);
}
#[tokio::test]
async fn one_call_budget_can_accept_but_never_makes_an_unbudgeted_repair(){
 for repair in [false,true]{let(app,pool,_,s,p,_,_,old)=fixture().await;sqlx::query("UPDATE profiles SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.contextBudget.maxPhysicalCalls',1) WHERE id=?").bind(&p).execute(&pool).await.unwrap();if repair{answer(&s,json!({"invalid":true}));}else{decide_answer(&s,"accept","");}let op=restore(&app,&p,&old).await;assert_eq!(op["status"],if repair{"failed"}else{"succeeded"},"{op}");if repair{assert_eq!(op["error"]["code"],"OPERATION_BUDGET_EXCEEDED");}assert_eq!(s.wires.lock().unwrap().len(),1);assert_eq!(op["physicalCallCount"],1);}
}
#[tokio::test]
async fn stale_cas_foreign_source_current_version_and_corrupt_hash_never_dispatch(){
 let(app,pool,_,s,p,_,_,old)=fixture().await;let request=req(&app,&p,&old).await;let mut stale=request.clone();stale["expectedRevision"]=json!(1);assert_eq!(app.dispatch(stale).await.response["error"]["code"],"REVISION_CONFLICT");let v=memory_view(&app,&p).await;let mut current=request.clone();current["payload"]["versionId"]=v["version"]["id"].clone();assert_eq!(app.dispatch(current).await.response["error"]["code"],"POLICY_DENIED");let q=profile(&app,"Foreign").await;let qv=memory_view(&app,q["id"].as_str().unwrap()).await;let mut foreign=request.clone();foreign["profileId"]=q["id"].clone();foreign["payload"]["recordId"]=qv["record"]["id"].clone();foreign["expectedRevision"]=json!(1);assert_eq!(app.dispatch(foreign).await.response["ok"],false);
 sqlx::query("UPDATE memory_versions SET body=json_set(body,'$.content','TAMPERED') WHERE id=?").bind(old["id"].as_str().unwrap()).execute(&pool).await.unwrap();assert_eq!(app.dispatch(request).await.response["error"]["code"],"STORAGE_ERROR");assert!(s.wires.lock().unwrap().is_empty());
}
#[tokio::test]
async fn stop_closes_seed_proposal_without_late_activation(){
 let(app,_,_,s,p,_,_,old)=fixture().await;let before=memory_view(&app,&p).await;s.hold.store(true,Ordering::SeqCst);decide_answer(&s,"accept","");let a=app.dispatch(req(&app,&p,&old).await).await.response;wait_posts(&s,1).await;let op=ok(&app,"operation.cancel",json!({"operationId":a["result"]["operation"]["id"]}),Some(&p),None).await;assert_eq!(op["status"],"cancelled");s.release.add_permits(1);let after=memory_view(&app,&p).await;assert_eq!(after["version"],before["version"]);assert_eq!(after["proposals"][0]["state"],"failed");assert_eq!(after["record"],before["record"]);
}
#[tokio::test]
async fn excluding_record_during_restore_rejects_late_result(){
 let(app,_,_,s,p,_,_,old)=fixture().await;let before=memory_view(&app,&p).await;s.hold.store(true,Ordering::SeqCst);decide_answer(&s,"accept","");let a=app.dispatch(req(&app,&p,&old).await).await.response;wait_posts(&s,1).await;toggle(&app,&p,true).await;s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert_eq!(memory_view(&app,&p).await["version"],before["version"]);assert_eq!(memory_view(&app,&p).await["record"]["state"],"outdated");
}
#[tokio::test]
async fn historical_source_change_during_restore_rejects_late_result(){
 let(app,pool,_,s,p,_,_,old)=fixture().await;s.hold.store(true,Ordering::SeqCst);decide_answer(&s,"accept","");let a=app.dispatch(req(&app,&p,&old).await).await.response;wait_posts(&s,1).await;sqlx::query("UPDATE memory_versions SET body=json_set(body,'$.language','en') WHERE id=?").bind(old["id"].as_str().unwrap()).execute(&pool).await.unwrap();s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");
}
#[tokio::test]
async fn retry_checks_snapshot_before_second_post_and_does_not_overcount(){
 let(app,_,_,s,p,_,_,old)=fixture().await;s.hold.store(true,Ordering::SeqCst);s.responses.lock().unwrap().push_back((503,"{}".into()));let a=app.dispatch(req(&app,&p,&old).await).await.response;wait_posts(&s,1).await;toggle(&app,&p,true).await;s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert_eq!(op["physicalCallCount"],1);assert_eq!(s.wires.lock().unwrap().len(),1);
}
#[tokio::test]
async fn receipt_and_final_version_write_failures_are_atomic(){
 for final_write in [false,true]{let(app,pool,_,s,p,_,_,old)=fixture().await;let before=memory_view(&app,&p).await;if final_write{sqlx::query("CREATE TRIGGER reject_restore_version BEFORE INSERT ON memory_versions WHEN json_extract(NEW.body,'$.origin')='restore' BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();decide_answer(&s,"accept","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"failed");}else{let request=req(&app,&p,&old).await;let n=counts(&pool).await;sqlx::query("CREATE TRIGGER reject_restore_receipt BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT,'injected'); END").execute(&pool).await.unwrap();assert_eq!(app.dispatch(request).await.response["error"]["code"],"STORAGE_ERROR");assert_eq!(counts(&pool).await,n);assert!(s.wires.lock().unwrap().is_empty());}let after=memory_view(&app,&p).await;assert_eq!(after["record"],before["record"]);assert_eq!(after["version"],before["version"]);}
}
#[tokio::test]
async fn pending_restore_excludes_parallel_update_and_second_restore(){
 let(app,_,_,s,p,c,_,old)=fixture().await;s.hold.store(true,Ordering::SeqCst);decide_answer(&s,"accept","");let request=req(&app,&p,&old).await;let a=app.dispatch(request.clone()).await.response;wait_posts(&s,1).await;assert_eq!(app.dispatch(request.clone()).await.response,a);let mut other=request;other["requestId"]=json!(uuid::Uuid::new_v4().to_string());assert_eq!(app.dispatch(other).await.response["error"]["code"],"REVISION_CONFLICT");assert_eq!(app.dispatch(memory_request(&app,&p,&c).await).await.response["error"]["code"],"REVISION_CONFLICT");s.release.add_permits(1);assert_eq!(finished(&app,&p,&a["result"]["operation"]["id"]).await["status"],"succeeded");assert_eq!(s.wires.lock().unwrap().len(),1);
}
#[tokio::test]
async fn restoring_a_previously_restored_version_preserves_auditable_chain(){
 let(app,_,_,s,p,c,_,old)=fixture().await;decide_answer(&s,"accept","");assert_eq!(restore(&app,&p,&old).await["status"],"succeeded");let restored=memory_view(&app,&p).await["version"].clone();save_memory(&app,&s,&p,&c,"New checkpoint").await;decide_answer(&s,"accept","");assert_eq!(restore(&app,&p,&restored).await["status"],"succeeded");let v=memory_view(&app,&p).await;assert_eq!(v["version"]["versionNumber"],5);assert_eq!(v["version"]["restoredFromVersionId"],restored["id"]);assert_eq!(listing(&app,&p,v["record"]["id"].as_str().unwrap(),10,None).await["items"].as_array().unwrap().len(),5);
}

#[tokio::test]
async fn changed_credential_during_restore_invalidates_snapshot(){
 let(app,pool,_,s,p,_,conn,old)=fixture().await;let before=memory_view(&app,&p).await;s.hold.store(true,Ordering::SeqCst);decide_answer(&s,"accept","");let a=app.dispatch(req(&app,&p,&old).await).await.response;wait_posts(&s,1).await;ok(&app,"connection.credential.set",json!({"connectionId":conn["id"],"credential":{"secret":"changed-key"}}),Some(&p),Some(1)).await;s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert_eq!(op["physicalCallCount"],1);let body:String=sqlx::query_scalar("SELECT body FROM memory_versions WHERE profile_id=? AND id=?").bind(&p).bind(before["version"]["id"].as_str().unwrap()).fetch_one(&pool).await.unwrap();assert_eq!(serde_json::from_str::<Value>(&body).unwrap(),before["version"]);
}
#[tokio::test]
async fn retry_uses_same_restore_wire_and_budget_then_commits_once(){
 let(app,_,_,s,p,_,_,old)=fixture().await;s.responses.lock().unwrap().push_back((503,"{}".into()));decide_answer(&s,"accept","");let op=restore(&app,&p,&old).await;assert_eq!(op["status"],"succeeded","{op}");assert_eq!(op["physicalCallCount"],2);let w=s.wires.lock().unwrap();assert_eq!(w.len(),2);assert_eq!(w[0],w[1]);
}
#[tokio::test]
async fn file_restart_retains_restored_memory_and_interrupts_pending_restore_without_replay(){
 let path=std::env::temp_dir().join(format!("ai-home-restore-{}.sqlite",uuid::Uuid::new_v4()));let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let vault=Arc::new(MemoryVault::default());let app=Application::with_vault(store,Vault::with_backend(vault.clone()));let(app,pool,vault,s,p,c,_)=configure(app,pool,vault).await;
 chat(&app,&p,&c,"Źródło").await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"disabled").await;let old=save_memory(&app,&s,&p,&c,"Jeden").await;save_memory(&app,&s,&p,&c,"Dwa").await;decide_answer(&s,"accept","");let done=restore(&app,&p,&old).await;assert_eq!(done["status"],"succeeded");let version=memory_view(&app,&p).await["version"].clone();let body:String=sqlx::query_scalar("SELECT body FROM memory_runs WHERE operation_id=?").bind(done["id"].as_str().unwrap()).fetch_one(&pool).await.unwrap();let mut snapshot:Value=serde_json::from_str(&body).unwrap();let pending=uuid::Uuid::new_v4().to_string();let proposal_id=uuid::Uuid::new_v4().to_string();snapshot["restoreProposalId"]=json!(proposal_id);let mut op=done.clone();op["id"]=json!(pending);op["status"]=json!("queued");op["revision"]=json!(1);op["physicalCallCount"]=json!(0);op.as_object_mut().unwrap().remove("result");op.as_object_mut().unwrap().remove("finishedAt");assert!(schema::dto("Operation",&op));
 sqlx::query("INSERT INTO operations(profile_id,id,revision,body) VALUES(?,?,1,?)").bind(&p).bind(&pending).bind(op.to_string()).execute(&pool).await.unwrap();sqlx::query("INSERT INTO memory_runs(profile_id,operation_id,record_id,body) VALUES(?,?,?,?)").bind(&p).bind(&pending).bind(version["recordId"].as_str().unwrap()).bind(snapshot.to_string()).execute(&pool).await.unwrap();let mut proposal=memory_view(&app,&p).await["proposals"].as_array().and_then(|a|a.first()).cloned().unwrap_or(json!(null));if proposal.is_null(){let raw:String=sqlx::query_scalar("SELECT body FROM memory_proposals WHERE operation_id=?").bind(done["id"].as_str().unwrap()).fetch_one(&pool).await.unwrap();proposal=serde_json::from_str(&raw).unwrap();}proposal["id"]=json!(proposal_id);proposal["operationId"]=json!(pending);proposal["revision"]=json!(1);proposal["state"]=json!("pending");assert!(schema::dto("MemoryProposal",&proposal));sqlx::query("INSERT INTO memory_proposals(profile_id,id,revision,body) VALUES(?,?,1,?)").bind(&p).bind(&proposal_id).bind(proposal.to_string()).execute(&pool).await.unwrap();let posts=s.wires.lock().unwrap().len();drop(app);pool.close().await;
 let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault));let v=memory_view(&app,&p).await;assert_eq!(v["version"],version);assert_eq!(v["latestOperation"]["status"],"interrupted");assert_eq!(v["proposals"][0]["state"],"failed");let c=fresh(&app,&p,&c).await;assert_eq!(preflight(&app,&p,&c,"Restored context").await["workingContext"]["versionId"],version["id"]);assert_eq!(s.wires.lock().unwrap().len(),posts);drop(app);pool.close().await;let _=std::fs::remove_file(path);
}
