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
