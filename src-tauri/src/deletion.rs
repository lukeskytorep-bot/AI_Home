use serde_json::{json,Value};
use sqlx::Row;
use crate::connections::{Failure,Tx};

fn response_result<'a>(response:&'a Value)->Option<&'a Value>{
    (response["ok"]==true).then(||response.get("result")).flatten()
}
fn has_profile_id(v:&Value,profile:&str)->bool{
    match v{
        Value::Object(m)=>{
            if m.get("profileId").and_then(Value::as_str)==Some(profile){return true}
            m.values().any(|x|has_profile_id(x,profile))
        }
        Value::Array(a)=>a.iter().any(|x|has_profile_id(x,profile)),
        _=>false
    }
}
fn result_is_profile(v:&Value,profile:&str)->bool{
    v.get("id").and_then(Value::as_str)==Some(profile)
        && v.get("displayName").is_some()
        && v.get("status").is_some()
}
fn has_conversation_id(v:&Value,cid:&str)->bool{
    match v{
        Value::Object(m)=>{
            if m.get("conversationId").and_then(Value::as_str)==Some(cid){return true}
            m.values().any(|x|has_conversation_id(x,cid))
        }
        Value::Array(a)=>a.iter().any(|x|has_conversation_id(x,cid)),
        _=>false
    }
}
fn result_is_conversation(v:&Value,profile:&str,cid:&str)->bool{
    v.get("profileId").and_then(Value::as_str)==Some(profile)
        && v.get("id").and_then(Value::as_str)==Some(cid)
        && v.get("title").is_some()
        && v.get("language").is_some()
}
fn tombstone_response(request_id:&str)->Value{
    json!({"contractVersion":1,"requestId":request_id,"ok":false,
        "error":{"code":"POLICY_DENIED","messageKey":"error.permanently_deleted","retryable":false}})
}
async fn tombstone_matching_receipts<F>(tx:&mut Tx<'_>,mut matches:F)->Result<(),Failure>
where F:FnMut(&Value)->bool{
    let rows=sqlx::query("SELECT request_id,response FROM receipts").fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    for row in rows{
        let request_id=row.get::<String,_>("request_id");
        let response:Value=serde_json::from_str(row.get::<&str,_>("response")).map_err(|_|"STORAGE_ERROR")?;
        if matches(&response){
            sqlx::query("UPDATE receipts SET response=? WHERE request_id=?")
                .bind(tombstone_response(&request_id).to_string()).bind(request_id)
                .execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        }
    }
    Ok(())
}
pub(crate) async fn tombstone_profile_receipts(tx:&mut Tx<'_>,profile:&str)->Result<(),Failure>{
    tombstone_matching_receipts(tx,|response|{
        let Some(result)=response_result(response) else{return false};
        has_profile_id(result,profile)||result_is_profile(result,profile)
    }).await
}
pub(crate) async fn tombstone_conversation_receipts(tx:&mut Tx<'_>,profile:&str,cid:&str)->Result<(),Failure>{
    tombstone_matching_receipts(tx,|response|{
        let Some(result)=response_result(response) else{return false};
        has_conversation_id(result,cid)||result_is_conversation(result,profile,cid)
    }).await
}
pub(crate) async fn redact_conversation_events(tx:&mut Tx<'_>,profile:&str,cid:&str)->Result<(),Failure>{
    let rows=sqlx::query("SELECT sequence,body FROM profile_events WHERE profile_id=? ORDER BY sequence")
        .bind(profile).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    for row in rows{
        let sequence=row.get::<i64,_>("sequence");
        let mut body:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR")?;
        let event_type=body.get("eventType").and_then(Value::as_str).unwrap_or("");
        let payload=body.get("payload").cloned().unwrap_or(Value::Null);
        let direct_conversation_event=event_type.starts_with("conversation.")
            && payload.get("id").and_then(Value::as_str)==Some(cid);
        if has_conversation_id(&payload,cid)||direct_conversation_event{
            body["payload"]=json!({"tombstone":true,"reason":"permanent_deletion"});
            sqlx::query("UPDATE profile_events SET body=? WHERE profile_id=? AND sequence=?")
                .bind(body.to_string()).bind(profile).bind(sequence)
                .execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        }
    }
    Ok(())
}


pub(crate) async fn redact_conversation_memory_runs(tx:&mut Tx<'_>,profile:&str,cid:&str)->Result<(),Failure>{
    let rows=sqlx::query("SELECT operation_id,body FROM memory_runs WHERE profile_id=? ORDER BY rowid")
        .bind(profile).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    for row in rows{
        let operation_id=row.get::<String,_>("operation_id");
        let mut body:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR")?;
        let direct=body.get("conversationId").and_then(Value::as_str)==Some(cid);
        let nested=body.get("messages").and_then(Value::as_array).map(|messages|messages.iter().any(|m|m.get("conversationId").and_then(Value::as_str)==Some(cid))).unwrap_or(false);
        if direct||nested{
            body["messages"]=json!([]);
            body["sources"]=json!([]);
            body["conversationId"]=Value::Null;
            body["conversationLanguage"]=json!("und");
            body["sourceTombstone"]=json!({"reason":"permanent_deletion","resumable":false});
            sqlx::query("UPDATE memory_runs SET body=? WHERE profile_id=? AND operation_id=?")
                .bind(body.to_string()).bind(profile).bind(operation_id)
                .execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        }
    }
    Ok(())
}

pub(crate) async fn redact_experimental_source_refs(tx:&mut Tx<'_>,profile:&str,source_kind:&str,source_id:&str)->Result<(),Failure>{
    for table in ["experimental_technical_checkpoints","experimental_semantic_checkpoints","experimental_private_reflections","experimental_self_records"] {
        let q=format!("SELECT id,body FROM {table} WHERE profile_id=? ORDER BY rowid");
        let rows=sqlx::query(&q).bind(profile).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        for row in rows {
            let id=row.get::<String,_>("id");
            let mut body:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR")?;
            let mut changed=false;
            if let Some(refs)=body.get_mut("sourceRefs").and_then(Value::as_array_mut) {
                for r in refs {
                    if r.get("sourceKind").and_then(Value::as_str)==Some(source_kind) && r.get("sourceId").and_then(Value::as_str)==Some(source_id) {
                        r["availability"]=json!("permanently_deleted");
                        changed=true;
                    }
                }
            }
            if changed {
                let q=format!("UPDATE {table} SET revision=revision+1,body=json_set(?,'$.revision',revision+1) WHERE profile_id=? AND id=?");
                sqlx::query(&q).bind(body.to_string()).bind(profile).bind(id).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::persistence::Store;
    use serde_json::json;

    #[tokio::test]
    async fn experimental_checkpoint_source_refs_are_tombstoned_on_source_deletion(){
        let store=Store::memory().await.unwrap();
        sqlx::query("INSERT INTO profiles(id,revision,body) VALUES('p',1,'{}')").execute(&store.pool).await.unwrap();
        sqlx::query("INSERT INTO experimental_instances(profile_id,id,revision,body) VALUES('p','exp',1,'{}')").execute(&store.pool).await.unwrap();
        sqlx::query("INSERT INTO experimental_work_episodes(profile_id,instance_id,id,revision,body) VALUES('p','exp','ep',1,'{}')").execute(&store.pool).await.unwrap();
        let source=json!({"sourceKind":"conversation","sourceId":"c1","sourceRevision":1,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","availability":"available"});
        let tech=json!({"schemaVersion":1,"id":"t1","instanceId":"exp","episodeId":"ep","revision":1,"parentCheckpointId":"","sourceEventSequence":0,"status":"active","decisionRefs":[],"actionRefs":[],"sourceRefs":[source.clone()],"createdAt":"2026-10-25T12:00:00Z"});
        sqlx::query("INSERT INTO experimental_technical_checkpoints(profile_id,instance_id,episode_id,id,revision,body) VALUES('p','exp','ep','t1',1,?)").bind(tech.to_string()).execute(&store.pool).await.unwrap();
        let sem=json!({"schemaVersion":1,"id":"s1","instanceId":"exp","episodeId":"ep","revision":1,"parentCheckpointId":"","technicalCheckpointId":"t1","proposer":"primary","primaryDisposition":"accepted","sourceEventSequence":0,"sourceRefs":[source],"content":{"focus":"","currentIntention":"","findings":[],"openQuestions":[],"nextSuggestedStep":""},"createdAt":"2026-10-25T12:00:00Z"});
        sqlx::query("INSERT INTO experimental_semantic_checkpoints(profile_id,instance_id,episode_id,technical_checkpoint_id,id,revision,body) VALUES('p','exp','ep','t1','s1',1,?)").bind(sem.to_string()).execute(&store.pool).await.unwrap();
        sqlx::query("INSERT INTO experimental_reflection_runs(profile_id,instance_id,id,revision,body) VALUES('p','exp','run',1,'{}')").execute(&store.pool).await.unwrap();
        let private=json!({"revision":1,"sourceRefs":[sem["sourceRefs"][0].clone()],"contentSha256":"unchanged","summary":"explicit private output"});
        sqlx::query("INSERT INTO experimental_private_reflections(profile_id,instance_id,run_id,id,revision,body) VALUES('p','exp','run','reflection',1,?)").bind(private.to_string()).execute(&store.pool).await.unwrap();
        sqlx::query("INSERT INTO experimental_self_reviews(profile_id,instance_id,reflection_id,candidate_index,id,revision,body) VALUES('p','exp','reflection',0,'review',1,'{}')").execute(&store.pool).await.unwrap();
        sqlx::query("INSERT INTO experimental_self_records(profile_id,instance_id,id,review_id,revision,body) VALUES('p','exp','record','review',1,?)").bind(private.to_string()).execute(&store.pool).await.unwrap();
        let mut tx=store.pool.begin().await.unwrap();redact_experimental_source_refs(&mut tx,"p","conversation","c1").await.unwrap();tx.commit().await.unwrap();
        for table in ["experimental_technical_checkpoints","experimental_semantic_checkpoints","experimental_private_reflections","experimental_self_records"]{
            let availability:String=sqlx::query_scalar(&format!("SELECT json_extract(body,'$.sourceRefs[0].availability') FROM {table} WHERE profile_id='p'")).fetch_one(&store.pool).await.unwrap();
            assert_eq!(availability,"permanently_deleted");
        }
    }
}
