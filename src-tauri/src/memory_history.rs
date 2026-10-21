//! Immutable, profile-scoped memory history and CAS exclusion. Never creates an approval.
use super::{bump, event, get, put, revision, Failure, Tx};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

pub(super) async fn approved(tx:&mut Tx<'_>,profile:&str,record:&Value,version_id:&str)->Result<(Value,Value),Failure>{
    let version = get(tx, "memory_versions", profile, version_id).await?;
    let content = version["content"].as_str().ok_or("STORAGE_ERROR")?;
    if content.len()>8*1024*1024 { return Err("CONTEXT_BUDGET_EXCEEDED"); }
    let digest = format!("{:x}", Sha256::digest(content.as_bytes()));
    if version["recordId"] != record["id"] || version["contentSha256"] != digest { return Err("STORAGE_ERROR"); }
    let decision = get(tx,"memory_decisions",profile,version["decisionReceiptId"].as_str().ok_or("STORAGE_ERROR")?).await?;
    let operation = get(tx,"operations",profile,decision["operationId"].as_str().ok_or("STORAGE_ERROR")?).await?;
    if !matches!(decision["decision"].as_str(),Some("accept" | "modify"))
        || decision["candidateSha256"] != version["contentSha256"]
        || decision["candidateMeasurement"] != version["measurement"]
        || decision["primaryRouteId"] != version["measurement"]["modelRouteId"]
        || decision["routeRevision"] != version["measurement"]["modelRouteRevision"]
        || operation["status"] != "succeeded" || operation["result"]["versionId"] != version["id"]
        || operation["result"]["recordId"] != record["id"] { return Err("STORAGE_ERROR"); }
    if version["sources"].as_array().ok_or("STORAGE_ERROR")?.iter().any(|s|s["profileId"]!=profile){return Err("STORAGE_ERROR");}
    match version["origin"].as_str(){
        Some("agent_decision") if operation["kind"]=="memory_update" && operation["result"]["outcome"]=="committed"=>{},
        Some("restore") if operation["kind"]=="restore" && operation["result"]["kind"]=="restore" && operation["result"]["restoredFromVersionId"]==version["restoredFromVersionId"]=>{
            let source=get(tx,"memory_versions",profile,version["restoredFromVersionId"].as_str().ok_or("STORAGE_ERROR")?).await?;
            if source["recordId"]!=record["id"]||source["versionNumber"].as_u64().ok_or("STORAGE_ERROR")?>=version["versionNumber"].as_u64().ok_or("STORAGE_ERROR")?||source["contentSha256"]!=format!("{:x}",Sha256::digest(source["content"].as_str().unwrap().as_bytes())){return Err("STORAGE_ERROR");}
            let reference=json!({"profileId":profile,"kind":"memory_version","entityId":source["id"],"contentSha256":source["contentSha256"]});
            if !version["sources"].as_array().unwrap().contains(&reference){return Err("STORAGE_ERROR");}
        },_=>return Err("STORAGE_ERROR")
    }
    Ok((version,decision))
}
async fn working_record(tx:&mut Tx<'_>,profile:&str,payload:&Value)->Result<Value,Failure>{
    let r=get(tx,"memory_records",profile,payload["recordId"].as_str().ok_or("VALIDATION_ERROR")?).await?;
    if r["kind"]!="working" {return Err("POLICY_DENIED");}
    Ok(r)
}
pub(super) async fn read(tx:&mut Tx<'_>,command:&str,profile:&str,payload:&Value)->Result<Value,Failure>{
    let record=working_record(tx,profile,payload).await?;
    if command=="memory.version.get" {
        let (version,decision)=approved(tx,profile,&record,payload["versionId"].as_str().ok_or("VALIDATION_ERROR")?).await?;
        return Ok(json!({"version":version,"decision":decision}));
    }
    let record_id=record["id"].as_str().unwrap();let limit=payload["page"]["limit"].as_i64().ok_or("VALIDATION_ERROR")?;
    let (head,before)=if let Some(cursor)=payload["page"]["cursor"].as_str(){
        if cursor.len()>2048{return Err("VALIDATION_ERROR");}
        let decoded=URL_SAFE_NO_PAD.decode(cursor).map_err(|_|"VALIDATION_ERROR")?;
        let v:Value=serde_json::from_slice(&decoded).map_err(|_|"VALIDATION_ERROR")?;
        let a=v.as_array().ok_or("VALIDATION_ERROR")?;
        if a.len()!=6||a[0]!="ai-home-memory-history:v1"||a[1]!=profile||a[2]!=record_id||a[3]!=limit{return Err("VALIDATION_ERROR");}
        let head=a[4].as_i64().filter(|n|*n>0).ok_or("VALIDATION_ERROR")?;
        let before=a[5].as_i64().filter(|n|*n>0&&*n<=head).ok_or("VALIDATION_ERROR")?;
        (head,before)
    }else{
        let head:i64=sqlx::query_scalar("SELECT COALESCE(MAX(rowid),0) FROM memory_versions WHERE profile_id=? AND record_id=?")
            .bind(profile).bind(record_id).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        (head,0)
    };
    // A high-water mark fixes this pagination traversal while later versions are appended.
    let rows=sqlx::query("SELECT id,rowid AS history_key FROM memory_versions WHERE profile_id=? AND record_id=? AND rowid<=? AND (?=0 OR rowid<?) ORDER BY rowid DESC LIMIT ?")
        .bind(profile).bind(record_id).bind(head).bind(before).bind(before).bind(limit+1).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut items=vec![];let mut last=0;
    for row in rows.iter().take(limit as usize){
        let (version,_)=approved(tx,profile,&record,row.get("id")).await?;items.push(version);last=row.get::<i64,_>("history_key");
    }
    // This self-contained cursor is an offset, not an authorization credential. Each read scopes SQL anew.
    let next=if rows.len()>limit as usize {URL_SAFE_NO_PAD.encode(json!(["ai-home-memory-history:v1",profile,record_id,limit,head,last]).to_string())}else{String::new()};
    Ok(json!({"items":items,"nextCursor":next}))
}
pub(super) async fn outdated(tx:&mut Tx<'_>,profile:&str,payload:&Value,expected:i64)->Result<Value,Failure>{
    let mut record=working_record(tx,profile,payload).await?;
    if revision(&record)?!=expected{return Err("REVISION_CONFLICT");}
    if !matches!(record["state"].as_str(),Some("active"|"outdated"))||record["activeVersionId"].as_str().is_none(){return Err("POLICY_DENIED");}
    let target=if payload["outdated"]==true{"outdated"}else{"active"};
    // Withdrawal never approves or mutates content. Re-enabling verifies the same already committed version.
    if target=="active" {approved(tx,profile,&record,record["activeVersionId"].as_str().unwrap()).await?;}
    if record["state"]==target{return Ok(record);}
    record["state"]=json!(target);bump(&mut record)?;put(tx,"memory_records",&record,false,expected).await?;
    event(tx,profile,"memory.changed",&record).await?;
    Ok(record)
}
