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
async fn create(app:&Application,p:&str,k:&str,title:&str)->Value{
 let mut a=json!({"title":title});if k=="intent"{a["intent"]=json!({"status":"open","trigger":{"kind":"manual","value":""},"importance":50});}
 ok(app,"memory.record.create",json!({"kind":k,"annotations":a,"subjectRef":"topic"}),Some(p),None).await
}
async fn layer(app:&Application,p:&str,id:&Value)->Value{ok(app,"memory.record.get",json!({"recordId":id}),Some(p),None).await}
async fn layer_request(app:&Application,p:&str,c:&Value,id:&Value,reason:&str)->Value{
 let src=ok(app,"memory.working.sources",json!({"conversationId":c["id"]}),Some(p),None).await;let r=layer(app,p,id).await;
 request("memory.record.update",json!({"recordId":id,"sourceRefs":src["sourceRefs"],"correctionReason":reason}),Some(p),r["record"]["revision"].as_i64())
}
async fn save_layer(app:&Application,s:&Server,p:&str,c:&Value,id:&Value,text:&str)->Value{
 propose_answer(s,text);decide_answer(s,"accept","");let req=layer_request(app,p,c,id,"From conversation; review independently").await;let a=app.dispatch(req).await.response;assert_eq!(a["ok"],true,"{a}");let op=finished(app,p,&a["result"]["operation"]["id"]).await;assert_eq!(op["status"],"succeeded","{op}");layer(app,p,id).await
}
async fn search(app:&Application,p:&str,q:&str)->Value{ok(app,"memory.search",json!({"query":q}),Some(p),None).await}
async fn layer_settings(app:&Application,p:&str)->Value{ok(app,"memory.layers.settings.get",json!({}),Some(p),None).await}
async fn limits(app:&Application,p:&str,tokens:i64,capacity:Option<(&str,Value)>)->Value{
 let s=layer_settings(app,p).await;let mut policies=s["memorySettings"].clone();policies.as_object_mut().unwrap().remove("working");if let Some((k,v))=capacity{policies[k]=v;}
 ok(app,"memory.layers.settings.set",json!({"policies":policies,"retrievalTargetTokens":tokens}),Some(p),s["profileRevision"].as_i64()).await
}
async fn metadata_set(app:&Application,p:&str,e:&Value,title:&str,status:Option<&str>,expires:Option<&str>)->Value{
 let mut a=json!({"title":title});if let Some(status)=status{a["intent"]=json!({"status":status,"trigger":{"kind":"manual","value":""},"importance":75});if let Some(t)=expires{a["intent"]["expiresAt"]=json!(t);}}
 ok(app,"memory.record.metadata.set",json!({"recordId":e["record"]["id"],"annotations":a}),Some(p),e["record"]["revision"].as_i64()).await
}
#[tokio::test]async fn five_kinds_empty_creation_provenance_and_frozen_dtos(){
 let(app,pool,_)=setup().await;let p=profile(&app,"layers").await;let p=p["id"].as_str().unwrap();
 for k in ["episodic","relationship","longTerm","intent","workRule"]{let e=create(&app,p,k,"label").await;assert_eq!(e["record"]["kind"],k);assert_eq!(e["record"]["state"],"empty");assert_eq!(e["metadata"]["addedBy"],"human");assert_eq!(e["metadata"]["createdAt"],e["record"]["createdAt"]);assert!(schema::dto("MemoryRecord",&e["record"]));assert!(e.get("version").is_none());if k=="intent"{assert!(schema::dto("Intent",&e["metadata"]["intent"]));}}
 assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM memory_versions").fetch_one(&pool).await.unwrap(),0);
}
#[tokio::test]async fn each_kind_commits_primary_approval_real_contacts_and_independent_capacity(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;
 for k in ["episodic","relationship","longTerm","intent","workRule"]{
  let e=create(&app,&p,k,"Pamięć").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"AI Home: żółw.").await;
  assert_eq!(e["record"]["state"],"active");assert_eq!(e["capacityPolicy"]["tokens"],if k=="longTerm"{500}else{300});assert_eq!(e["version"]["language"],"pl");assert_eq!(e["proposals"][0]["memoryKind"],k);assert_eq!(e["decisions"][0]["decision"],"accept");assert!(e["versionMetadata"]["sourceContacts"].as_array().unwrap().len()>=2);assert!(e["versionMetadata"]["lastContactAt"].as_str().is_some());
  let wire=s.wires.lock().unwrap().last().unwrap().clone();assert!(wire.contains("memoryKind"));assert!(wire.contains(k));
 }
}
#[tokio::test]async fn layer_limits_do_not_change_working_or_other_kinds(){
 let(app,_,_,_,p,_,_)=memory_fixture().await;let s=limits(&app,&p,1,Some(("relationship",json!({"kind":"unlimited"})))).await;
 assert_eq!(s["memorySettings"]["working"]["tokens"],300);assert_eq!(s["memorySettings"]["episodic"]["tokens"],300);assert_eq!(s["memorySettings"]["longTerm"]["tokens"],500);assert_eq!(s["memorySettings"]["relationship"]["kind"],"unlimited");assert_eq!(s["retrievalTargetTokens"],1);
 memory_settings(&app,&p,json!({"kind":"bounded","tokens":500}),"disabled").await;let s=layer_settings(&app,&p).await;assert_eq!(s["memorySettings"]["relationship"]["kind"],"unlimited");assert_eq!(s["retrievalTargetTokens"],1);
}
#[tokio::test]async fn layer_settings_stale_revision_and_receipt_replay(){
 let(app,_,_,_,p,_,_)=memory_fixture().await;let s=layer_settings(&app,&p).await;let mut policies=s["memorySettings"].clone();policies.as_object_mut().unwrap().remove("working");let req=request("memory.layers.settings.set",json!({"policies":policies,"retrievalTargetTokens":800}),Some(&p),s["profileRevision"].as_i64());let a=app.dispatch(req.clone()).await.response;assert_eq!(a["ok"],true,"{a}");assert_eq!(app.dispatch(req.clone()).await.response,a);let mut other=req;other["requestId"]=json!(uuid::Uuid::new_v4().to_string());assert_eq!(app.dispatch(other).await.response["error"]["code"],"REVISION_CONFLICT");
}
#[tokio::test]async fn create_idempotency_and_profile_scope(){
 let(app,_,_)=setup().await;let p=profile(&app,"p").await;let q=profile(&app,"q").await;let p=p["id"].as_str().unwrap();let q=q["id"].as_str().unwrap();
 let req=request("memory.record.create",json!({"kind":"longTerm","annotations":{"title":"One"}}),Some(p),None);let a=app.dispatch(req.clone()).await.response;assert_eq!(a["ok"],true,"{a}");assert_eq!(app.dispatch(req).await.response,a);let eid=&a["result"]["record"]["id"];
 for command in ["memory.record.get","memory.record.metadata.set","memory.record.update","memory.outdated.set","memory.versions.list","memory.version.get","memory.restore"]{
 let payload=match command {"memory.record.metadata.set"=>json!({"recordId":eid,"annotations":{"title":"No"}}),"memory.record.update"=>json!({"recordId":eid,"sourceRefs":[],"correctionReason":"No"}),"memory.outdated.set"=>json!({"recordId":eid,"outdated":true}),"memory.versions.list"=>json!({"recordId":eid,"page":{"limit":10}}),"memory.version.get"|"memory.restore"=>json!({"recordId":eid,"versionId":"foreign"}),_=>json!({"recordId":eid})};let r=app.dispatch(request(command,payload,Some(q),Some(1))).await.response;assert_eq!(r["ok"],false,"{r}");}
 let list=ok(&app,"memory.records.list",json!({"page":{"limit":20}}),Some(p),None).await;assert_eq!(list["items"].as_array().unwrap().len(),1);
}
#[tokio::test]async fn pagination_high_water_scope_and_restart_safe_cursor(){
 let(app,_,_)=setup().await;let p=profile(&app,"p").await;let q=profile(&app,"q").await;let p=p["id"].as_str().unwrap();let q=q["id"].as_str().unwrap();
 let mut ids=vec![];for _ in 0..3{ids.push(create(&app,p,"episodic","item").await["record"]["id"].clone());}
 let first=ok(&app,"memory.records.list",json!({"page":{"limit":1}}),Some(p),None).await;assert_eq!(first["items"][0]["record"]["id"],ids[0]);create(&app,p,"episodic","new").await;
 let page=json!({"page":{"limit":1,"cursor":first["nextCursor"]}});let next=ok(&app,"memory.records.list",page.clone(),Some(p),None).await;assert_eq!(next["items"][0]["record"]["id"],ids[1]);assert_eq!(app.dispatch(request("memory.records.list",page,Some(q),None)).await.response["error"]["code"],"VALIDATION_ERROR");
 let last=ok(&app,"memory.records.list",json!({"page":{"limit":1,"cursor":next["nextCursor"]}}),Some(p),None).await;assert_eq!(last["items"][0]["record"]["id"],ids[2]);assert_eq!(last["nextCursor"],"");
}
#[tokio::test]async fn invalid_annotations_and_forged_dates_or_decisions_do_not_create_records(){
 let(app,_,_)=setup().await;let p=profile(&app,"p").await;let p=p["id"].as_str().unwrap();
 for payload in [json!({"kind":"working","annotations":{"title":"No"}}),json!({"kind":"intent","annotations":{"title":"Missing intent"}}),json!({"kind":"episodic","annotations":{"title":" "}}),json!({"kind":"episodic","annotations":{"title":"x","createdAt":"2020-01-01T00:00:00Z"}}),json!({"kind":"episodic","annotations":{"title":"x"},"decision":"accept"}),json!({"kind":"intent","annotations":{"title":"x","intent":{"status":"open","trigger":{"kind":"time","value":"tomorrow"},"importance":50}}})]{assert_eq!(app.dispatch(request("memory.record.create",payload,Some(p),None)).await.response["error"]["code"],"VALIDATION_ERROR");}
 assert!(ok(&app,"memory.records.list",json!({"page":{"limit":10}}),Some(p),None).await["items"].as_array().unwrap().is_empty());
}
#[tokio::test]async fn correction_requires_reason_and_retains_versions_and_contacts(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"relationship","Contact").await;let first=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home: first inference").await;
 let mut req=layer_request(&app,&p,&c,&e["record"]["id"]," ").await;assert_eq!(app.dispatch(req.clone()).await.response["error"]["code"],"VALIDATION_ERROR");req["requestId"]=json!(uuid::Uuid::new_v4().to_string());req["payload"].as_object_mut().unwrap().remove("correctionReason");req["command"]=json!("memory.update.request");assert_eq!(app.dispatch(req).await.response["error"]["code"],"VALIDATION_ERROR");
 let second=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home: corrected inference").await;assert_eq!(second["version"]["previousVersionId"],first["version"]["id"]);assert_eq!(second["versionMetadata"]["previousVersionId"],first["version"]["id"]);assert!(second["versionMetadata"]["correctionReason"].as_str().unwrap().contains("review independently"));
 let list=listing(&app,&p,e["record"]["id"].as_str().unwrap(),10,None).await;assert_eq!(list["items"].as_array().unwrap().len(),2);assert_eq!(list["items"][1],first["version"]);
 let historical=ok(&app,"memory.version.annotations.get",json!({"recordId":e["record"]["id"],"versionId":first["version"]["id"]}),Some(&p),None).await;assert_eq!(historical["versionMetadata"],first["versionMetadata"]);
}
#[tokio::test]async fn rejected_conflicting_proposal_retains_previous_version_and_annotations(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"longTerm","Home").await;let old=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home known").await;
 propose_answer(&s,"Home contradicts previous");decide_answer(&s,"reject","");let req=layer_request(&app,&p,&c,&e["record"]["id"],"New claim is not evidence").await;let a=app.dispatch(req).await.response;let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["result"]["outcome"],"no_change");let current=layer(&app,&p,&e["record"]["id"]).await;assert_eq!(current["version"],old["version"]);assert_eq!(current["metadata"],old["metadata"]);
}
#[tokio::test]async fn actual_modify_is_measured_against_kind_limit_two_attempts(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,Some(("episodic",json!({"kind":"bounded","tokens":10})))).await;let e=create(&app,&p,"episodic","Home").await;
 propose_answer(&s,"small");decide_answer(&s,"modify","this text is too long");propose_answer(&s,"retry");decide_answer(&s,"modify","żółw");let req=layer_request(&app,&p,&c,&e["record"]["id"],"Review").await;let a=app.dispatch(req).await.response;let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["status"],"succeeded","{op}");let v=layer(&app,&p,&e["record"]["id"]).await;assert_eq!(v["version"]["content"],"żółw");assert_eq!(v["version"]["measurement"]["value"],"żółw".len());assert_eq!(v["attempts"][0]["result"],"capacity_rejected");assert_eq!(v["attempts"][1]["result"],"accepted");assert_eq!(v["proposals"][0]["capacityPolicySnapshot"]["tokens"],10);
}
#[tokio::test]async fn annotation_change_during_post_blocks_memory_commit(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"intent","Home").await;s.hold.store(true,Ordering::SeqCst);propose_answer(&s,"Home open");let req=layer_request(&app,&p,&c,&e["record"]["id"],"Review").await;let a=app.dispatch(req).await.response;wait_posts(&s,1).await;metadata_set(&app,&p,&e,"Home",Some("completed"),None).await;s.hold.store(false,Ordering::SeqCst);s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["status"],"failed");assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert_eq!(layer(&app,&p,&e["record"]["id"]).await["record"]["state"],"empty");assert_eq!(s.wires.lock().unwrap().len(),1);
}
#[tokio::test]async fn lexical_search_is_profile_scoped_case_unicode_exact_not_entire_store(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,None).await;
 let e=create(&app,&p,"episodic","Żółw").await;save_layer(&app,&s,&p,&c,&e["record"]["id"],"żółw projekt Home").await;let other=create(&app,&p,"longTerm","unrelated").await;save_layer(&app,&s,&p,&c,&other["record"]["id"],"Different topic").await;
 let found=search(&app,&p,"ŻÓŁW").await;assert_eq!(found["items"].as_array().unwrap().len(),1);assert_eq!(found["items"][0]["record"]["id"],e["record"]["id"]);assert!(found["inputTokens"].as_u64().unwrap()<=10000);assert!(search(&app,&p,"żółwiki").await["items"].as_array().unwrap().is_empty());assert!(search(&app,&p,"").await["items"].as_array().unwrap().is_empty());
 let q=profile(&app,"other").await;assert!(ok(&app,"memory.records.list",json!({"page":{"limit":10}}),q["id"].as_str(),None).await["items"].as_array().unwrap().is_empty());
}
#[tokio::test]async fn retrieval_includes_exact_whole_block_boundary_never_truncates_and_reports_omissions(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"workRule","Home").await;save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home complete record").await;limits(&app,&p,10000,None).await;let f=search(&app,&p,"Home").await;let cost=f["inputTokens"].as_i64().unwrap();assert!(cost>0);
 limits(&app,&p,cost,None).await;let f=search(&app,&p,"Home").await;assert_eq!(f["items"].as_array().unwrap().len(),1);assert_eq!(f["inputTokens"],cost);
 limits(&app,&p,cost-1,None).await;let f=search(&app,&p,"Home").await;assert!(f["items"].as_array().unwrap().is_empty());assert_eq!(f["omittedCount"],1);assert_eq!(f["inputTokens"],0);assert_eq!(layer(&app,&p,&e["record"]["id"]).await["version"]["content"],"Home complete record");
 limits(&app,&p,1,None).await;assert!(search(&app,&p,"Home").await["items"].as_array().unwrap().is_empty());
}
#[tokio::test]async fn inactive_records_and_closed_or_expired_intents_are_excluded_without_history_loss(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"intent","Home").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home plan").await;
 assert_eq!(search(&app,&p,"Home").await["items"].as_array().unwrap().len(),1);
 for status in ["completed","abandoned","expired"]{let e=layer(&app,&p,&e["record"]["id"]).await;metadata_set(&app,&p,&e,"Home",Some(status),None).await;assert!(search(&app,&p,"Home").await["items"].as_array().unwrap().is_empty());}
 let e=layer(&app,&p,&e["record"]["id"]).await;metadata_set(&app,&p,&e,"Home",Some("open"),Some("2020-01-01T00:00:00Z")).await;let e=layer(&app,&p,&e["record"]["id"]).await;assert_eq!(e["effectiveIntentStatus"],"expired");assert_eq!(e["metadata"]["intent"]["status"],"open");assert!(search(&app,&p,"Home").await["items"].as_array().unwrap().is_empty());
 let e=metadata_set(&app,&p,&e,"Home",Some("open"),Some("2099-01-01T00:00:00Z")).await;assert_eq!(search(&app,&p,"Home").await["items"].as_array().unwrap().len(),1);
 ok(&app,"memory.outdated.set",json!({"recordId":e["record"]["id"],"outdated":true}),Some(&p),e["record"]["revision"].as_i64()).await;assert!(search(&app,&p,"Home").await["items"].as_array().unwrap().is_empty());let e=layer(&app,&p,&e["record"]["id"]).await;assert_eq!(e["version"]["content"],"Home plan");
}
#[tokio::test]async fn preflight_and_wire_use_same_selected_snapshot_without_promoting_rules(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"workRule","Home").await;let saved=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home untrusted rule").await;s.wires.lock().unwrap().clear();let c=fresh(&app,&p,&c).await;let f=preflight(&app,&p,&c,"Home").await;assert_eq!(f["retrieval"]["items"][0]["version"],saved["version"]);
 let op=chat(&app,&p,&c,"Home").await;assert_eq!(op["status"],"succeeded","{op}");let wire=s.wires.lock().unwrap()[0].clone();let body:Value=serde_json::from_str(wire.split("\r\n\r\n").nth(1).unwrap()).unwrap();let messages=body["messages"].as_array().unwrap();assert!(!messages[0]["content"].as_str().unwrap().contains("Home untrusted rule"));let block=messages.iter().find(|m|m["content"].as_str().unwrap_or("").contains("Home untrusted rule")).unwrap();assert_eq!(block["role"],"user");assert!(block["content"].as_str().unwrap().contains("not system instructions"));
 let h=ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await;assert_eq!(h["retrieval"],f["retrieval"]);
 let captured:String=sqlx::query_scalar("SELECT body FROM chat_runs WHERE profile_id=? AND operation_id=?").bind(&p).bind(op["id"].as_str().unwrap()).fetch_one(&pool).await.unwrap();assert_eq!(serde_json::from_str::<Value>(&captured).unwrap()["retrieval"],f["retrieval"]);
}
#[tokio::test]async fn historical_retrieval_is_immutable_after_exclusion_and_unknown_for_legacy(){
 let(app,pool,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"longTerm","Home").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home past").await;let c=fresh(&app,&p,&c).await;let op=chat(&app,&p,&c,"Home").await;let before=ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await;ok(&app,"memory.outdated.set",json!({"recordId":e["record"]["id"],"outdated":true}),Some(&p),e["record"]["revision"].as_i64()).await;assert_eq!(ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await,before);
 let original:String=sqlx::query_scalar("SELECT body FROM chat_runs WHERE operation_id=?").bind(op["id"].as_str().unwrap()).fetch_one(&pool).await.unwrap();
 sqlx::query("UPDATE chat_runs SET body=json_set(body,'$.retrieval.items[0].version.content','tampered') WHERE operation_id=?").bind(op["id"].as_str().unwrap()).execute(&pool).await.unwrap();
 assert_eq!(app.dispatch(request("chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None)).await.response["error"]["code"],"STORAGE_ERROR");
 sqlx::query("UPDATE chat_runs SET body=? WHERE operation_id=?").bind(&original).bind(op["id"].as_str().unwrap()).execute(&pool).await.unwrap();
 sqlx::query("UPDATE chat_runs SET body=json_remove(body,'$.retrieval') WHERE operation_id=?").bind(op["id"].as_str().unwrap()).execute(&pool).await.unwrap();assert_eq!(ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await["known"],false);
}
#[tokio::test]async fn exclusion_during_chat_prevents_late_commit(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"episodic","Home").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home current").await;s.wires.lock().unwrap().clear();s.hold.store(true,Ordering::SeqCst);let c=fresh(&app,&p,&c).await;let a=app.dispatch(send_request(&p,&c,"Home")).await.response;wait_posts(&s,1).await;ok(&app,"memory.outdated.set",json!({"recordId":e["record"]["id"],"outdated":true}),Some(&p),e["record"]["revision"].as_i64()).await;s.hold.store(false,Ordering::SeqCst);s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"REVISION_CONFLICT");assert!(op.get("result").is_none());
}
#[tokio::test]async fn restore_nonworking_keeps_kind_capacity_and_current_intent_status(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"intent","Home").await;let first=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home first").await;let second=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home second").await;let e=metadata_set(&app,&p,&second,"Home",Some("completed"),None).await;decide_answer(&s,"accept","");let a=app.dispatch(request("memory.restore",json!({"recordId":e["record"]["id"],"versionId":first["version"]["id"]}),Some(&p),e["record"]["revision"].as_i64())).await.response;assert_eq!(a["ok"],true,"{a}");let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["status"],"succeeded","{op}");let e=layer(&app,&p,&e["record"]["id"]).await;assert_eq!(e["version"]["content"],"Home first");assert_eq!(e["record"]["kind"],"intent");assert_eq!(e["metadata"]["intent"]["status"],"completed");assert_eq!(e["versionMetadata"]["sourceContacts"],first["versionMetadata"]["sourceContacts"]);assert_eq!(e["proposals"][0]["memoryKind"],"intent");
}
#[tokio::test]async fn metadata_write_failure_rolls_back_record_event_and_receipt(){
 let(app,pool,_)=setup().await;let p=profile(&app,"p").await;let p=p["id"].as_str().unwrap();sqlx::query("CREATE TRIGGER no_meta BEFORE INSERT ON memory_metadata BEGIN SELECT RAISE(ABORT,'test'); END").execute(&pool).await.unwrap();let req=request("memory.record.create",json!({"kind":"episodic","annotations":{"title":"x"}}),Some(p),None);let id=req["requestId"].as_str().unwrap();assert_eq!(app.dispatch(req.clone()).await.response["error"]["code"],"STORAGE_ERROR");assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM memory_records WHERE json_extract(body,'$.kind')!='working'").fetch_one(&pool).await.unwrap(),0);assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM receipts WHERE request_id=?").bind(id).fetch_one(&pool).await.unwrap(),0);
}
#[tokio::test]async fn migration_v9_preserves_approved_working_history_through_v12(){
 let path=std::env::temp_dir().join(format!("ai-home-layers-{}.sqlite3",uuid::Uuid::new_v4()));let vault=Arc::new(MemoryVault::default());let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault.clone()));let(app,pool,vault,s,p,c,_)=configure(app,pool,vault).await;
 let a=app.dispatch(send_request(&p,&c,"AI Home")).await.response;finished(&app,&p,&a["result"]["operation"]["id"]).await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"disabled").await;let v=save_memory(&app,&s,&p,&c,"Previous L2").await;let prefs=ok(&app,"app.preferences.get",json!({}),None,None).await;
 sqlx::query("DROP TABLE memory_version_metadata").execute(&pool).await.unwrap();sqlx::query("DROP TABLE memory_metadata").execute(&pool).await.unwrap();sqlx::query("PRAGMA user_version=9").execute(&pool).await.unwrap();drop(app);pool.close().await;
 let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault));assert_eq!(sqlx::query_scalar::<_,i64>("PRAGMA user_version").fetch_one(&pool).await.unwrap(),13);assert_eq!(memory_view(&app,&p).await["version"],v);assert_eq!(ok(&app,"app.preferences.get",json!({}),None,None).await,prefs);assert!(sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap().is_empty());
}
#[tokio::test]async fn zero_retrieval_budget_is_contract_error_without_changing_profile(){
 let(app,_,_,_,p,_,_)=memory_fixture().await;let before=layer_settings(&app,&p).await;let mut policies=before["memorySettings"].clone();policies.as_object_mut().unwrap().remove("working");let r=app.dispatch(request("memory.layers.settings.set",json!({"policies":policies,"retrievalTargetTokens":0}),Some(&p),before["profileRevision"].as_i64())).await.response;assert_eq!(r["error"]["code"],"VALIDATION_ERROR");assert_eq!(layer_settings(&app,&p).await,before);
}
#[tokio::test]async fn retrieval_uses_current_route_measurement_without_rewriting_historical_version(){
 let(app,_,_,s,p,c,conn)=memory_fixture().await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"longTerm","Home").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home known").await;
 let snap=ok(&app,"profile.snapshot",json!({}),Some(&p),None).await;let oldroute=snap["routes"].as_array().unwrap().iter().find(|r|r["role"]=="primary").unwrap();ok(&app,"route.save",json!({"routeId":oldroute["id"],"route":{"role":"primary","connectionId":conn["id"],"modelId":"new-model","outputLimitTokens":1024}}),Some(&p),oldroute["revision"].as_i64()).await;
 let f=search(&app,&p,"Home").await;assert_eq!(f["items"][0]["measurement"]["modelRouteId"],oldroute["id"]);assert_ne!(f["items"][0]["measurement"]["modelRouteRevision"],e["version"]["measurement"]["modelRouteRevision"]);assert_eq!(layer(&app,&p,&e["record"]["id"]).await["version"],e["version"]);
}
#[tokio::test]async fn unlimited_memory_does_not_bypass_chat_context_preflight(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;limits(&app,&p,10000,Some(("longTerm",json!({"kind":"unlimited"})))).await;let e=create(&app,&p,"longTerm","Home").await;save_layer(&app,&s,&p,&c,&e["record"]["id"],&format!("Home {}","x".repeat(1000))).await;let c=fresh(&app,&p,&c).await;let full=preflight(&app,&p,&c,"Home").await;let needs=full["estimatedInputTokens"].as_i64().unwrap()+full["outputTokens"].as_i64().unwrap()+full["inputReserveTokens"].as_i64().unwrap();let settings=ok(&app,"chat.settings.get",json!({}),Some(&p),None).await;
 ok(&app,"chat.settings.set",json!({"routeId":settings["routeId"],"contextTokens":needs-1}),Some(&p),settings["routeRevision"].as_i64()).await;s.wires.lock().unwrap().clear();let req=send_request(&p,&c,"Home");let before=ok(&app,"conversation.messages.list",json!({"conversationId":c["id"],"page":{"limit":100}}),Some(&p),None).await;let r=app.dispatch(req).await.response;assert_eq!(r["error"]["code"],"CONTEXT_BUDGET_EXCEEDED","{r}");assert!(s.wires.lock().unwrap().is_empty());assert_eq!(ok(&app,"conversation.messages.list",json!({"conversationId":c["id"],"page":{"limit":100}}),Some(&p),None).await,before);
}
#[tokio::test]async fn double_capacity_failure_and_stop_preserve_nonworking_version(){
 let(app,_,_,s,p,c,_)=memory_fixture().await;let e=create(&app,&p,"relationship","Home").await;let saved=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home saved").await;limits(&app,&p,10000,Some(("relationship",json!({"kind":"bounded","tokens":3})))).await;
 propose_answer(&s,"Too long");propose_answer(&s,"Still too long");let a=app.dispatch(layer_request(&app,&p,&c,&e["record"]["id"],"Shrink").await).await.response;let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["error"]["code"],"MEMORY_CAPACITY_EXCEEDED");assert_eq!(layer(&app,&p,&e["record"]["id"]).await["version"],saved["version"]);
 s.wires.lock().unwrap().clear();s.hold.store(true,Ordering::SeqCst);propose_answer(&s,"New");let a=app.dispatch(layer_request(&app,&p,&c,&e["record"]["id"],"Shrink").await).await.response;wait_posts(&s,1).await;ok(&app,"operation.cancel",json!({"operationId":a["result"]["operation"]["id"]}),Some(&p),None).await;s.hold.store(false,Ordering::SeqCst);s.release.add_permits(1);let op=finished(&app,&p,&a["result"]["operation"]["id"]).await;assert_eq!(op["status"],"cancelled");assert_eq!(layer(&app,&p,&e["record"]["id"]).await["version"],saved["version"]);
}
#[tokio::test]async fn file_restart_preserves_nonworking_metadata_history_and_retrieval_receipt(){
 let path=std::env::temp_dir().join(format!("ai-home-layers-restart-{}.sqlite3",uuid::Uuid::new_v4()));let vault=Arc::new(MemoryVault::default());let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault.clone()));let(app,pool,vault,s,p,c,_)=configure(app,pool,vault).await;
 let a=app.dispatch(send_request(&p,&c,"Home")).await.response;finished(&app,&p,&a["result"]["operation"]["id"]).await;memory_settings(&app,&p,json!({"kind":"bounded","tokens":300}),"disabled").await;limits(&app,&p,10000,None).await;let e=create(&app,&p,"intent","Home").await;let e=save_layer(&app,&s,&p,&c,&e["record"]["id"],"Home intention").await;let c=fresh(&app,&p,&c).await;let op=chat(&app,&p,&c,"Home").await;let captured=ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await;
 drop(app);pool.close().await;let store=Store::open(&path).await.unwrap();let pool=store.pool.clone();let app=Application::with_vault(store,Vault::with_backend(vault));let restored=layer(&app,&p,&e["record"]["id"]).await;for k in ["record","version","metadata","versionMetadata"]{assert_eq!(restored[k],e[k]);}assert_eq!(ok(&app,"chat.retrieval.get",json!({"operationId":op["id"]}),Some(&p),None).await,captured);assert_eq!(listing(&app,&p,e["record"]["id"].as_str().unwrap(),10,None).await["items"][0],e["version"]);drop(app);pool.close().await;std::fs::remove_file(&path).unwrap();
}
