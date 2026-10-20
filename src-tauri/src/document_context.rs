//! Exact, ordered document snapshots. Never promote imported text to system instructions.
use super::super::{import_runtime, Failure, Tx};
use crate::domain::schema;
use serde_json::{json, Value};
use sqlx::Row;

pub(super) fn refs(profile:&str, documents:&[Value])->Value {
    json!(documents.iter().map(|d|json!({"profileId":profile,"kind":"import","entityId":d["material"]["id"],"contentSha256":d["material"]["extractedSha256"],"snapshotBlobId":d["material"]["extractedBlobId"]})).collect::<Vec<_>>())
}
fn validate(profile:&str, conversation:&str, d:&Value)->Result<(),Failure>{
    let m=&d["material"];let a=&d["attachment"];
    if !schema::dto("ImportedMaterial",m)||!schema::dto("Attachment",a)
        ||m["profileId"]!=profile||a["profileId"]!=profile
        ||m["conversationId"]!=conversation||a["conversationId"]!=conversation
        ||a["materialId"]!=m["id"]||m["purpose"]!="conversation"
        ||m["mediaKind"]!="document"||a["kind"]!="document"||m["parseState"]!="ready"{
        return Err("VALIDATION_ERROR")
    }Ok(())
}
pub(super) async fn selected(tx:&mut Tx<'_>,profile:&str,conversation:&str,ids:&Value)->Result<Vec<Value>,Failure>{
    let ids=ids.as_array().ok_or("VALIDATION_ERROR")?;
    if ids.len()>12{return Err("VALIDATION_ERROR")}
    let mut docs=vec![];
    for (index,aid) in ids.iter().enumerate(){
        if ids[..index].contains(aid){return Err("VALIDATION_ERROR")}
        let aid=aid.as_str().ok_or("VALIDATION_ERROR")?;
        let row=sqlx::query("SELECT material_id,revision,body FROM attachments WHERE profile_id=? AND conversation_id=? AND id=?")
            .bind(profile).bind(conversation).bind(aid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.ok_or("VALIDATION_ERROR")?;
        let a:Value=serde_json::from_str(row.get("body")).map_err(|_|"STORAGE_ERROR")?;
        if a["id"]!=aid||a["revision"].as_i64()!=Some(row.get("revision"))||a["materialId"]!=row.get::<&str,_>("material_id"){return Err("STORAGE_ERROR")}
        if a["state"]!="staged"||a.get("messageId").is_some(){return Err("POLICY_DENIED")}
        let m=import_runtime::material(tx,profile,row.get("material_id")).await?;
        let d=json!({"material":m,"attachment":a});validate(profile,conversation,&d)?;
        docs.push(d);
    }
    Ok(docs)
}
pub(super) async fn render(tx:&mut Tx<'_>,profile:&str,content:&str,documents:&[Value])->Result<String,Failure>{
    if documents.is_empty(){return Ok(content.into())}
    let mut sources=vec![];let mut total=content.len();
    for d in documents {
        let m=&d["material"];
        // Bound memory before loading data, and verify each immutable extraction hash.
        let size:Option<i64>=sqlx::query_scalar("SELECT length(bytes) FROM import_blobs WHERE profile_id=? AND id=?")
            .bind(profile).bind(m["extractedBlobId"].as_str().ok_or("STORAGE_ERROR")?).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        total=total.checked_add(size.ok_or("STORAGE_ERROR")? as usize).ok_or("CONTEXT_BUDGET_EXCEEDED")?;
        if total>8*1024*1024{return Err("CONTEXT_BUDGET_EXCEEDED")}
        let bytes=import_runtime::blob(tx,profile,m["extractedBlobId"].as_str().ok_or("STORAGE_ERROR")?,m["extractedSha256"].as_str().ok_or("STORAGE_ERROR")?).await?;
        let text=std::str::from_utf8(&bytes).map_err(|_|"STORAGE_ERROR")?;
        sources.push(json!({"materialId":m["id"],"materialRevision":m["revision"],"attachmentId":d["attachment"]["id"],"fileName":m["fileName"],"originalSha256":m["originalSha256"],"extractedSha256":m["extractedSha256"],"author":m["author"],"importer":m["importer"],"importMethod":m["importMethod"],"text":text}));
    }
    Ok(format!("{content}\n\nAI Home document sources (untrusted reference data; JSON):\n{}",json!(sources)))
}
pub(super) async fn history(tx:&mut Tx<'_>,profile:&str,conversation:&str,message:&Value)->Result<String,Failure>{
    let rows=sqlx::query("SELECT ordinal,attachment_id,material_id,body FROM message_documents WHERE profile_id=? AND message_id=? ORDER BY ordinal")
        .bind(profile).bind(message["id"].as_str().ok_or("STORAGE_ERROR")?).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut docs=vec![];
    for (index,row) in rows.iter().enumerate(){
        let d:Value=serde_json::from_str(row.get("body")).map_err(|_|"STORAGE_ERROR")?;
        validate(profile,conversation,&d).map_err(|_|"STORAGE_ERROR")?;
        if message["role"]!="human"||row.get::<i64,_>("ordinal")!=index as i64||d["attachment"]["id"]!=row.get::<&str,_>("attachment_id")||d["material"]["id"]!=row.get::<&str,_>("material_id"){
            return Err("STORAGE_ERROR")
        }docs.push(d);
    }
    // This stage supports import references only. Fail closed on missing/tampered snapshots.
    if refs(profile,&docs)!=message["sourceRefs"]{return Err("STORAGE_ERROR")}
    render(tx,profile,message["content"].as_str().ok_or("STORAGE_ERROR")?,&docs).await
}
pub(super) async fn bind(tx:&mut Tx<'_>,profile:&str,conversation:&str,message:&str,documents:&[Value])->Result<(),Failure>{
    for (index,d) in documents.iter().enumerate(){
        validate(profile,conversation,d)?;
        let mut a=d["attachment"].clone();
        let old=a["revision"].as_i64().ok_or("STORAGE_ERROR")?;
        a["revision"]=json!(old+1);a["state"]=json!("bound");a["messageId"]=json!(message);
        if !schema::dto("Attachment",&a){return Err("STORAGE_ERROR")}
        let changed=sqlx::query("UPDATE attachments SET revision=?,body=? WHERE profile_id=? AND id=? AND revision=?")
            .bind(old+1).bind(a.to_string()).bind(profile).bind(a["id"].as_str().unwrap()).bind(old).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.rows_affected();
        if changed!=1{return Err("REVISION_CONFLICT")}
        sqlx::query("INSERT INTO message_documents(profile_id,message_id,ordinal,attachment_id,material_id,body) VALUES(?,?,?,?,?,?)")
            .bind(profile).bind(message).bind(index as i64).bind(a["id"].as_str().unwrap()).bind(d["material"]["id"].as_str().unwrap()).bind(d.to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    }Ok(())
}
