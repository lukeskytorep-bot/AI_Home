//! Profile-scoped, content-safe activity history projected from durable operation events.
use super::{Failure,Tx};
use crate::domain::schema;
use serde_json::{json,Value};
use sqlx::Row;

fn activity_status(op:&Value)->&'static str{
    match op["status"].as_str().unwrap_or(""){
        "queued"|"running"=>"running",
        "succeeded"=>"success",
        "cancelled"=>"cancelled",
        "no_action"=>"no_action",
        _=>"error",
    }
}
fn project(event:&Value)->Result<Option<Value>,Failure>{
    if event["eventType"]!="operation.changed"{return Ok(None)}
    let op=&event["payload"];
    if op.get("tombstone").and_then(Value::as_bool)==Some(true)
        && op.get("reason").and_then(Value::as_str)==Some("permanent_deletion")
    {return Ok(None)}
    let operation_id=op["id"].as_str().ok_or("STORAGE_ERROR")?;
    let profile=event["profileId"].as_str().ok_or("STORAGE_ERROR")?;
    if op["profileId"]!=profile{return Err("STORAGE_ERROR")}
    let mut metadata=json!({});
    if let Some(n)=op["capacityAttemptCount"].as_i64().filter(|n|matches!(n,1|2)){metadata["attemptNumber"]=json!(n);}
    if let Some(code)=op.get("error").and_then(|e|e.get("code")).and_then(Value::as_str){metadata["errorCode"]=json!(code);}
    if let Some(record)=op.get("recordRefs").and_then(Value::as_array).and_then(|r|r.first()).and_then(|r|r.get("recordId")).and_then(Value::as_str){metadata["recordId"]=json!(record);}
    let activity=json!({
        "schemaVersion":1,
        "id":event["eventId"],
        "profileId":profile,
        "revision":op["revision"],
        "createdAt":event["occurredAt"],
        "operationId":operation_id,
        "sequence":event["sequence"],
        "code":format!("operation.{}",op["kind"].as_str().unwrap_or("unknown")),
        "status":activity_status(op),
        "metadata":metadata
    });
    if !schema::dto("Activity",&activity){return Err("STORAGE_ERROR")}
    Ok(Some(activity))
}
pub(super) async fn list(tx:&mut Tx<'_>,profile:&str,payload:&Value)->Result<Value,Failure>{
    let mut cursor=payload["afterSequence"].as_i64().ok_or("VALIDATION_ERROR")?;
    let limit=payload["limit"].as_i64().ok_or("VALIDATION_ERROR")? as usize;
    let mut items=Vec::new();
    loop{
        let rows=sqlx::query("SELECT sequence,body FROM profile_events WHERE profile_id=? AND sequence>? ORDER BY sequence LIMIT 100")
            .bind(profile).bind(cursor).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        if rows.is_empty(){break}
        let count=rows.len();
        for row in rows{
            cursor=row.get::<i64,_>("sequence");
            let event:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|"STORAGE_ERROR")?;
            if let Some(a)=project(&event)?{items.push(a);if items.len()>=limit{return Ok(json!({"items":items,"lastSequence":cursor}))}}
        }
        if count<100{break}
    }
    Ok(json!({"items":items,"lastSequence":cursor}))
}
