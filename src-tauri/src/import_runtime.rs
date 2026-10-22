//! Native import lifecycle, original blobs and profile/conversation-scoped reads.
use super::{event, get, id, now, put, revision, terminal, Connections, Failure, Tx};
use crate::{
    domain::schema,
    imports::{self, SelectedFile},
    providers::transport::CallBudget,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use sqlx::Row;
use tokio::sync::OwnedSemaphorePermit;
pub(super) struct ImportJob {
    pub token: String,
    files: Vec<(String, SelectedFile)>,
    permit: OwnedSemaphorePermit,
}
fn validate(v: &Value, name: &str) -> Result<(), Failure> {
    if schema::dto(name, v) {
        Ok(())
    } else {
        Err("STORAGE_ERROR")
    }
}
pub(super) async fn material(tx: &mut Tx<'_>, profile: &str, mid: &str) -> Result<Value, Failure> {
    if let Some(row)=sqlx::query("SELECT revision,body,conversation_id,original_blob_id,extracted_blob_id FROM import_materials WHERE profile_id=? AND id=?")
        .bind(profile).bind(mid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")? {
        let v: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
        validate(&v, "ImportedMaterial")?;
        if v["profileId"] != profile || v["id"] != mid || v["revision"].as_i64() != Some(row.get("revision"))
            || v["conversationId"] != row.get::<&str, _>("conversation_id")
            || v["originalBlobId"] != row.get::<&str, _>("original_blob_id")
            || v["extractedBlobId"].as_str() != row.get::<Option<&str>, _>("extracted_blob_id") {return Err("STORAGE_ERROR");}
        return Ok(v);
    }
    let row=sqlx::query("SELECT revision,body,original_blob_id,extracted_blob_id FROM profile_import_materials WHERE profile_id=? AND id=?")
        .bind(profile).bind(mid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.ok_or("NOT_FOUND")?;
    let v: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
    validate(&v, "ImportedMaterial")?;
    if v["profileId"] != profile || v["id"] != mid || v["revision"].as_i64() != Some(row.get("revision"))
        || v.get("conversationId").is_some()
        || v["originalBlobId"] != row.get::<&str, _>("original_blob_id")
        || v["extractedBlobId"].as_str() != row.get::<Option<&str>, _>("extracted_blob_id") {return Err("STORAGE_ERROR");}
    Ok(v)
}
async fn save_material(tx: &mut Tx<'_>, v: &Value) -> Result<(), Failure> {
    validate(v, "ImportedMaterial")?;
    let table=if v.get("conversationId").is_some(){"import_materials"}else{"profile_import_materials"};
    let sql=format!("UPDATE {table} SET revision=?,body=?,extracted_blob_id=? WHERE profile_id=? AND id=?");
    let changed=sqlx::query(&sql).bind(v["revision"].as_i64().unwrap()).bind(v.to_string()).bind(v["extractedBlobId"].as_str()).bind(v["profileId"].as_str().unwrap()).bind(v["id"].as_str().unwrap()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.rows_affected();
    if changed!=1{return Err("STORAGE_ERROR")}
    Ok(())
}
async fn attachments_failed(tx: &mut Tx<'_>, profile: &str, mid: &str) -> Result<(), Failure> {
    let rows = sqlx::query("SELECT body FROM attachments WHERE profile_id=? AND material_id=?")
        .bind(profile)
        .bind(mid)
        .fetch_all(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    for row in rows {
        let mut a: Value = serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
        validate(&a, "Attachment")?;
        a["revision"] = json!(a["revision"].as_i64().unwrap() + 1);
        a["state"] = json!("failed");
        validate(&a, "Attachment")?;
        sqlx::query("UPDATE attachments SET revision=?,body=? WHERE profile_id=? AND id=?")
            .bind(a["revision"].as_i64().unwrap())
            .bind(a.to_string())
            .bind(profile)
            .bind(a["id"].as_str().unwrap())
            .execute(&mut **tx)
            .await
            .map_err(|_| "STORAGE_ERROR")?;
    }
    Ok(())
}
pub(super) async fn blob(
    tx: &mut Tx<'_>,
    profile: &str,
    bid: &str,
    expected: &str,
) -> Result<Vec<u8>, Failure> {
    let row = sqlx::query("SELECT bytes,sha256 FROM import_blobs WHERE profile_id=? AND id=?")
        .bind(profile)
        .bind(bid)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?
        .ok_or("STORAGE_ERROR")?;
    let b: Vec<u8> = row.get("bytes");
    if imports::digest(&b) != expected || row.get::<&str, _>("sha256") != expected {
        return Err("STORAGE_ERROR");
    };
    Ok(b)
}
async fn insert_blob(
    tx: &mut Tx<'_>,
    profile: &str,
    bid: &str,
    sha: &str,
    bytes: &[u8],
) -> Result<(), Failure> {
    sqlx::query("INSERT INTO import_blobs(profile_id,id,sha256,bytes) VALUES(?,?,?,?)")
        .bind(profile)
        .bind(bid)
        .bind(sha)
        .bind(bytes)
        .execute(&mut **tx)
        .await
        .map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}
pub(super) async fn accept(
    tx: &mut Tx<'_>,
    service: &Connections,
    profile: &str,
    payload: &Value,
) -> Result<(ImportJob, Value), Failure> {
    let purpose=payload["purpose"].as_str().ok_or("VALIDATION_ERROR")?;
    if !matches!(purpose,"conversation"|"knowledge"|"guidance"|"memory_import"){return Err("VALIDATION_ERROR")}
    if payload["author"]["assertedBy"] != "importer"
        || payload["author"]["verified"] != false
        || payload["author"]["label"].as_str().unwrap().len() > 256
    {
        return Err("VALIDATION_ERROR");
    }
    if purpose=="guidance" {
        let p=get(tx,"profiles","",profile).await?;let rid=p["primaryRouteId"].as_str().ok_or("MODEL_CAPABILITY_UNKNOWN")?;let _=get(tx,"routes",profile,rid).await?;
    }
    let conversation=if purpose=="conversation"{
        let cid=payload["conversationId"].as_str().ok_or("VALIDATION_ERROR")?;
        let c=get(tx,"conversations",profile,cid).await?;
        if c["status"]!="open"{return Err("POLICY_DENIED")}
        Some(cid)
    }else{
        if payload.get("conversationId").is_some(){return Err("VALIDATION_ERROR")}
        None
    };
    let token = payload["selectionToken"].as_str().unwrap();
    let snapshot = service.imports.snapshot(token, profile, conversation).await?;
    if purpose!="conversation" && snapshot.files.iter().any(|f|imports::image::is_image(&f.name)){return Err("VALIDATION_ERROR")}
    let permit = service.imports.worker.clone().try_acquire_owned().map_err(|_| "POLICY_DENIED")?;
    let opid = id();
    let time = now();
    let mut files = vec![];
    let mut op = json!({"schemaVersion":1,"id":opid,"profileId":profile,"revision":1,"createdAt":time,"kind":"import","status":"queued","capacityAttemptCount":0,"physicalCallCount":0,"limits":{"maxPhysicalCalls":1,"maxElapsedMs":30000},"recordRefs":[]});
    put(tx, "operations", &op, true, 0).await?;
    for file in snapshot.files {
        let mid = id();
        let bid = id();
        let lower = file.name.to_lowercase();
        let is_image=imports::image::is_image(&file.name);
        let mime = if is_image {if lower.ends_with(".png"){"image/png"}else if lower.ends_with(".webp"){"image/webp"}else if lower.ends_with(".gif"){"image/gif"}else{"image/jpeg"}} else if lower.ends_with(".pdf") {
            "application/pdf"
        } else if lower.ends_with(".docx") {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        } else if lower.ends_with(".md") {
            "text/markdown"
        } else {
            "text/plain"
        };
        let mut m = json!({"schemaVersion":1,"id":mid,"profileId":profile,"revision":1,"createdAt":time,"fileName":file.name,"mimeType":mime,"originalBlobId":bid,"originalSha256":file.sha256,"originalSizeBytes":file.bytes.len(),"purpose":purpose,"author":payload["author"],"importer":{"kind":"human","localActorId":"local-user"},"importedAt":time,"mediaKind":if is_image{"image"}else{"document"},"parseState":"pending"});
        if let Some(cid)=conversation{m["conversationId"]=json!(cid)}
        validate(&m, "ImportedMaterial")?;
        insert_blob(tx, profile, &bid, &file.sha256, &file.bytes).await?;
        if let Some(cid)=conversation{
            sqlx::query("INSERT INTO import_materials(profile_id,id,conversation_id,operation_id,original_blob_id,revision,body) VALUES(?,?,?,?,?,1,?)")
                .bind(profile).bind(&mid).bind(cid).bind(&opid).bind(&bid).bind(m.to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
            let aid=id();
            let a = json!({"schemaVersion":1,"id":aid,"profileId":profile,"revision":1,"createdAt":time,"conversationId":cid,"materialId":mid,"kind":if is_image{"image"}else{"document"},"state":"staged"});
            validate(&a, "Attachment")?;
            sqlx::query("INSERT INTO attachments(profile_id,id,conversation_id,material_id,revision,body) VALUES(?,?,?,?,1,?)")
                .bind(profile).bind(&aid).bind(cid).bind(&mid).bind(a.to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        }else{
            sqlx::query("INSERT INTO profile_import_materials(profile_id,id,operation_id,original_blob_id,revision,body) VALUES(?,?,?,?,1,?)")
                .bind(profile).bind(&mid).bind(&opid).bind(&bid).bind(m.to_string()).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
        }
        op["recordRefs"].as_array_mut().unwrap().push(json!({"profileId":profile,"recordKind":"material","recordId":mid}));
        files.push((mid, file));
    }
    put(tx, "operations", &op, false, 1).await?;
    event(tx, profile, "operation.changed", &op).await?;
    Ok((ImportJob { token: token.into(), files, permit }, op))
}

pub(super) async fn read(
    tx: &mut Tx<'_>,
    command: &str,
    profile: &str,
    p: &Value,
    expected: i64,
) -> Result<Value, Failure> {
    if command == "material.get" {
        return material(tx, profile, p["materialId"].as_str().unwrap()).await;
    }
    let cid = p["conversationId"].as_str().unwrap();
    get(tx, "conversations", profile, cid).await?;
    match command {
        "conversation.import.latest" => {
            let body:Option<String>=sqlx::query_scalar("SELECT o.body FROM operations o WHERE o.profile_id=? AND json_extract(o.body,'$.kind')='import' AND EXISTS(SELECT 1 FROM import_materials m WHERE m.profile_id=o.profile_id AND m.operation_id=o.id AND m.conversation_id=?) ORDER BY json_extract(o.body,'$.createdAt') DESC,o.rowid DESC LIMIT 1").bind(profile).bind(cid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
            if let Some(body) = body {
                let op: Value = serde_json::from_str(&body).map_err(|_| "STORAGE_ERROR")?;
                validate(&op, "Operation")?;
                Ok(json!({"found":true,"operation":op}))
            } else {
                Ok(json!({"found":false}))
            }
        }
        "conversation.materials.list" => {
            let limit = p["limit"].as_i64().unwrap();
            let rows=sqlx::query("SELECT id,material_id,revision,body FROM attachments WHERE profile_id=? AND conversation_id=? AND id>? ORDER BY id LIMIT ?").bind(profile).bind(cid).bind(p["afterId"].as_str().unwrap_or("")).bind(limit+1).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
            let mut items = vec![];
            for row in rows.iter().take(limit as usize) {
                let a: Value =
                    serde_json::from_str(row.get("body")).map_err(|_| "STORAGE_ERROR")?;
                validate(&a, "Attachment")?;
                if a["id"] != row.get::<&str, _>("id")
                    || a["materialId"] != row.get::<&str, _>("material_id")
                    || a["profileId"] != profile
                    || a["conversationId"] != cid
                    || a["revision"].as_i64() != Some(row.get("revision"))
                {
                    return Err("STORAGE_ERROR");
                }
                let m = material(tx, profile, row.get("material_id")).await?;
                if m["conversationId"] != cid {
                    return Err("STORAGE_ERROR");
                };
                items.push(json!({"attachment":a,"material":m}));
            }
            let next = if rows.len() > limit as usize {
                items.last().unwrap()["attachment"]["id"].as_str().unwrap()
            } else {
                ""
            };
            Ok(json!({"items":items,"nextAfterId":next}))
        }
        "attachment.detach" => {
            let aid = p["attachmentId"].as_str().unwrap();
            let row=sqlx::query("SELECT revision,material_id,body FROM attachments WHERE profile_id=? AND conversation_id=? AND id=?").bind(profile).bind(cid).bind(aid).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.ok_or("NOT_FOUND")?;
            if row.get::<i64, _>("revision") != expected {
                return Err("REVISION_CONFLICT");
            };
            let a:Value=serde_json::from_str(row.get("body")).map_err(|_|"STORAGE_ERROR")?;
            validate(&a,"Attachment")?;
            if a["state"]=="bound" || a.get("messageId").is_some(){return Err("POLICY_DENIED");}
            let m = material(tx, profile, row.get("material_id")).await?;
            if m["parseState"] == "pending" {
                return Err("POLICY_DENIED");
            }
            sqlx::query("DELETE FROM attachments WHERE profile_id=? AND id=?")
                .bind(profile)
                .bind(aid)
                .execute(&mut **tx)
                .await
                .map_err(|_| "STORAGE_ERROR")?;
            Ok(json!({"removed":true}))
        }
        "material.original.read" | "material.text.read" => {
            let m = material(tx, profile, p["materialId"].as_str().unwrap()).await?;
            if m["conversationId"] != cid {
                return Err("NOT_FOUND");
            }
            let original = command == "material.original.read";
            if !original && (m["parseState"] != "ready"||m["mediaKind"]!="document") {
                return Err("IMPORT_UNREADABLE");
            }
            let bid = m[if original {
                "originalBlobId"
            } else {
                "extractedBlobId"
            }]
            .as_str()
            .ok_or("STORAGE_ERROR")?;
            let sha = m[if original {
                "originalSha256"
            } else {
                "extractedSha256"
            }]
            .as_str()
            .ok_or("STORAGE_ERROR")?;
            let b = blob(tx, profile, bid, sha).await?;
            if original && m["originalSizeBytes"].as_u64() != Some(b.len() as u64) {
                return Err("STORAGE_ERROR");
            }
            let offset = p["offset"].as_u64().unwrap() as usize;
            let limit = p["limit"].as_u64().unwrap() as usize;
            if offset > b.len() {
                return Err("VALIDATION_ERROR");
            };
            let mut end = offset.saturating_add(limit).min(b.len());
            if original {
                return Ok(
                    json!({"dataBase64":STANDARD.encode(&b[offset..end]),"nextOffset":if end<b.len(){end}else{0},"totalBytes":b.len(),"fileName":m["fileName"],"mimeType":m["mimeType"],"sha256":sha}),
                );
            }
            let text = std::str::from_utf8(&b).map_err(|_| "STORAGE_ERROR")?;
            if !text.is_char_boundary(offset) {
                return Err("VALIDATION_ERROR");
            };
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            if end == offset && offset < b.len() {
                return Err("VALIDATION_ERROR");
            }
            Ok(
                json!({"text":&text[offset..end],"nextOffset":if end<b.len(){end}else{0},"totalBytes":b.len()}),
            )
        }
        _ => Err("UNKNOWN_COMMAND"),
    }
}
type Outcome = (String, Result<imports::ParsedMaterial, String>);
pub(super) async fn run(
    service: &Connections,
    job: ImportJob,
    op: Value,
    cancel: CallBudget,
) -> Result<(), Failure> {
    let profile = op["profileId"].as_str().unwrap();
    let opid = op["id"].as_str().unwrap();
    {
        let _guard = service.gate.lock().await;
        let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
        let mut current = get(&mut tx, "operations", profile, opid).await?;
        if current["status"] != "queued" {
            drop(tx);
            drop(_guard);
            return failed(service, profile, opid, "OPERATION_CANCELLED").await;
        }
        current["status"] = json!("running");
        current["revision"] = json!(2);
        put(&mut tx, "operations", &current, false, 1).await?;
        event(&mut tx, profile, "operation.changed", &current).await?;
        tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    }
    let expected = job.files.len();
    let worker_cancel = cancel.clone();
    let task = tokio::task::spawn_blocking(move || {
        let _permit = job.permit;
        let mut outcomes = vec![];
        for (mid, f) in job.files {
            if worker_cancel.ensure_active().is_err() {
                break;
            }
            outcomes.push((mid, imports::parse_material(&f.name, &f.bytes)));
        }
        outcomes
    });
    let outcomes = tokio::select! {_ =cancel.cancelled()=>return failed(service,profile,opid,"OPERATION_CANCELLED").await,
    result=tokio::time::timeout(std::time::Duration::from_secs(30),task)=>match result{Ok(Ok(v))=>v,_=>return failed(service,profile,opid,"IMPORT_UNREADABLE").await}};
    if outcomes.len() != expected || cancel.ensure_active().is_err() {
        return failed(service, profile, opid, "OPERATION_BUDGET_EXCEEDED").await;
    }
    commit(service, profile, opid, outcomes).await
}
async fn commit(
    service: &Connections,
    profile: &str,
    opid: &str,
    outcomes: Vec<Outcome>,
) -> Result<(), Failure> {
    let _guard = service.gate.lock().await;
    let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
    let mut op = get(&mut tx, "operations", profile, opid).await?;
    if !["queued", "running"].contains(&op["status"].as_str().unwrap()) {
        drop(tx);drop(_guard);return failed(service, profile, opid, "OPERATION_CANCELLED").await;
    }
    let p = get(&mut tx, "profiles", "", profile).await?;
    if p["status"] == "archived" {drop(tx);drop(_guard);return failed(service, profile, opid, "POLICY_DENIED").await;}
    let mut mids = vec![];let mut aids = vec![];let mut knowledge_ids=vec![];let mut guidance_ids=vec![];let mut ready = 0;
    for (mid, outcome) in outcomes {
        let mut m = material(&mut tx, profile, &mid).await?;
        if let Some(cid)=m["conversationId"].as_str(){
            let c=get(&mut tx,"conversations",profile,cid).await?;
            if c["status"]!="open"{drop(tx);drop(_guard);return failed(service,profile,opid,"POLICY_DENIED").await;}
        }
        m["revision"] = json!(m["revision"].as_i64().unwrap() + 1);
        match outcome {
            Ok(imports::ParsedMaterial::Image(parsed)) => {
                if m["purpose"]!="conversation" {return Err("VALIDATION_ERROR")}
                m["mimeType"]=json!(parsed.mime);m["parseState"]=json!("ready");
                m["imageMetadata"]=json!({"format":parsed.format,"width":parsed.width,"height":parsed.height});ready+=1;
            }
            Ok(imports::ParsedMaterial::Document(parsed)) => {
                if m["originalSizeBytes"].as_u64() != Some(parsed.size_bytes as u64) {return Err("STORAGE_ERROR")};
                m["fileName"] = json!(parsed.display_name);m["mimeType"] = json!(parsed.mime_type);
                let bid = id();insert_blob(&mut tx,profile,&bid,&parsed.content_hash,parsed.content.as_bytes()).await?;
                m["parseState"] = json!("ready");m["extractedBlobId"] = json!(bid);m["extractedSha256"] = json!(parsed.content_hash);m["importMethod"] = json!(parsed.import_method);ready += 1;
            }
            Err(message) => {
                m["parseState"] = json!("unreadable");m["errorCode"] = json!(imports::parse_error(&message));attachments_failed(&mut tx, profile, &mid).await?;
            }
        }
        save_material(&mut tx, &m).await?;
        if m["parseState"]=="ready" && m["mediaKind"]=="document" {
            match m["purpose"].as_str().unwrap() {
                "knowledge" => {
                    let kid=id();let k=json!({"schemaVersion":1,"id":kid,"profileId":profile,"revision":1,"createdAt":now(),"materialId":mid,"enabled":true,"indexRevision":1,"sourceLanguage":"und"});
                    validate(&k,"KnowledgeSource")?;
                    sqlx::query("INSERT INTO knowledge_sources(profile_id,id,material_id,revision,body) VALUES(?,?,?,?,?)").bind(profile).bind(&kid).bind(&mid).bind(1i64).bind(k.to_string()).execute(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                    knowledge_ids.push(kid);
                }
                "guidance" => {
                    let route_id=p["primaryRouteId"].as_str().ok_or("MODEL_CAPABILITY_UNKNOWN")?;
                    let route=get(&mut tx,"routes",profile,route_id).await?;
                    let bytes=blob(&mut tx,profile,m["extractedBlobId"].as_str().ok_or("STORAGE_ERROR")?,m["extractedSha256"].as_str().ok_or("STORAGE_ERROR")?).await?;
                    let order:i64=sqlx::query_scalar("SELECT COALESCE(MAX(json_extract(body,'$.order')),-1)+1 FROM persistent_guidance WHERE profile_id=?").bind(profile).fetch_one(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                    let gid=id();let g=json!({"schemaVersion":1,"id":gid,"profileId":profile,"revision":1,"createdAt":now(),"materialId":mid,"enabled":true,"order":order,"language":"und","contentSha256":m["extractedSha256"],"tokenMeasurement":{"value":bytes.len(),"method":"estimated","estimatorId":"utf8_bytes_v1","modelRouteId":route["id"],"modelRouteRevision":route["revision"]}});
                    validate(&g,"PersistentGuidance")?;
                    sqlx::query("INSERT INTO persistent_guidance(profile_id,id,material_id,revision,body) VALUES(?,?,?,?,?)").bind(profile).bind(&gid).bind(&mid).bind(1i64).bind(g.to_string()).execute(&mut *tx).await.map_err(|_|"STORAGE_ERROR")?;
                    guidance_ids.push(gid);
                }
                _=>{}
            }
        }
        mids.push(mid.clone());
        let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM attachments WHERE profile_id=? AND material_id=? ORDER BY id").bind(profile).bind(&mid).fetch_all(&mut *tx).await.map_err(|_| "STORAGE_ERROR")?;
        aids.extend(ids);
    }
    let previous = revision(&op)?;
    if ready > 0 {
        op["revision"] = json!(previous + 1);op["finishedAt"] = json!(now());op["status"] = json!("succeeded");
        op["result"] = json!({"kind":"import","materialIds":mids,"attachmentIds":aids,"knowledgeSourceIds":knowledge_ids,"guidanceIds":guidance_ids,"proposalIds":[]});
    } else {terminal(&mut op, "failed", "IMPORT_UNREADABLE")?;}
    put(&mut tx, "operations", &op, false, previous).await?;event(&mut tx, profile, "operation.changed", &op).await?;
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;Ok(())
}
pub(super) async fn failed(
    service: &Connections,
    profile: &str,
    opid: &str,
    code: Failure,
) -> Result<(), Failure> {
    let _guard = service.gate.lock().await;
    let mut tx = service.pool.begin().await.map_err(|_| "STORAGE_ERROR")?;
    let mut op = get(&mut tx, "operations", profile, opid).await?;
    cancel_pending(&mut tx, profile, opid, code).await?;
    if ["queued", "running"].contains(&op["status"].as_str().unwrap()) {
        let previous = revision(&op)?;
        terminal(&mut op, "failed", code)?;
        put(&mut tx, "operations", &op, false, previous).await?;
        event(&mut tx, profile, "operation.changed", &op).await?;
    }
    tx.commit().await.map_err(|_| "STORAGE_ERROR")?;
    Ok(())
}

pub(super) async fn cancel_pending(
    tx: &mut Tx<'_>,
    profile: &str,
    opid: &str,
    code: Failure,
) -> Result<(), Failure> {
    let mut ids:Vec<String>=sqlx::query_scalar("SELECT id FROM import_materials WHERE profile_id=? AND operation_id=? AND json_extract(body,'$.parseState')='pending'").bind(profile).bind(opid).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut profile_ids:Vec<String>=sqlx::query_scalar("SELECT id FROM profile_import_materials WHERE profile_id=? AND operation_id=? AND json_extract(body,'$.parseState')='pending'").bind(profile).bind(opid).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    ids.append(&mut profile_ids);
    for mid in ids {
        let mut m = material(tx, profile, &mid).await?;
        m["revision"] = json!(m["revision"].as_i64().unwrap() + 1);m["parseState"] = json!("failed");m["errorCode"] = json!(code);
        save_material(tx, &m).await?;attachments_failed(tx, profile, &mid).await?;
    }
    Ok(())
}
