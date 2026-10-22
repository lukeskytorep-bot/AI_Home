//! Durable Context Segments for long-conversation continuity.
//! Raw messages remain authoritative; sealed segments contribute only a bounded checkpoint summary.
use super::{bump, get, id, now, put, revision, Failure, Tx};
use crate::{domain::schema, providers::ProviderMessage};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;

fn message_ref(profile:&str,m:&Value)->Result<Value,Failure>{
    let content=m["content"].as_str().ok_or("STORAGE_ERROR")?;
    Ok(json!({"profileId":profile,"kind":"message","entityId":m["id"],"contentSha256":format!("{:x}",Sha256::digest(content.as_bytes()))}))
}

async fn range_messages(tx:&mut Tx<'_>,profile:&str,conversation:&str,first:i64,last:i64)->Result<Vec<Value>,Failure>{
    if first<1||last<first{return Err("STORAGE_ERROR")}
    let rows=sqlx::query("SELECT id,sequence FROM messages WHERE profile_id=? AND conversation_id=? AND sequence>=? AND sequence<=? ORDER BY sequence")
        .bind(profile).bind(conversation).bind(first).bind(last).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut out=Vec::with_capacity(rows.len());
    let mut expected=first;
    for row in rows{
        let sequence: i64=row.get("sequence");
        if sequence!=expected{return Err("STORAGE_ERROR")}
        let m=get(tx,"messages",profile,row.get("id")).await?;
        if m["conversationId"]!=conversation||m["sequence"].as_i64()!=Some(sequence){return Err("STORAGE_ERROR")}
        out.push(m);expected+=1;
    }
    if expected!=last+1{return Err("STORAGE_ERROR")}
    Ok(out)
}

fn digest(messages:&[Value])->Result<String,Failure>{
    let mut h=Sha256::new();
    for m in messages{
        h.update(m["id"].as_str().ok_or("STORAGE_ERROR")?.as_bytes());h.update([0]);
        h.update(m["sequence"].as_i64().ok_or("STORAGE_ERROR")?.to_string().as_bytes());h.update([0]);
        h.update(m["role"].as_str().ok_or("STORAGE_ERROR")?.as_bytes());h.update([0]);
        h.update(m["content"].as_str().ok_or("STORAGE_ERROR")?.as_bytes());h.update([0xff]);
    }
    Ok(format!("{:x}",h.finalize()))
}

async fn segment(tx:&mut Tx<'_>,profile:&str,id:&str)->Result<Value,Failure>{get(tx,"context_segments",profile,id).await}

async fn latest_sealed(tx:&mut Tx<'_>,profile:&str,conversation:&str)->Result<Option<Value>,Failure>{
    let row=sqlx::query("SELECT id FROM context_segments WHERE profile_id=? AND conversation_id=? AND state='sealed' ORDER BY last_message_sequence DESC LIMIT 1")
        .bind(profile).bind(conversation).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    match row{Some(r)=>Ok(Some(segment(tx,profile,r.get("id")).await?)),None=>Ok(None)}
}

pub(super) async fn capture(tx:&mut Tx<'_>,profile:&str,conversation:&str,conv:&Value)->Result<Value,Failure>{
    let previous=latest_sealed(tx,profile,conversation).await?;
    let active=match conv.get("activeSegmentId").and_then(Value::as_str){
        Some(s)=>{let v=segment(tx,profile,s).await?;if v["conversationId"]!=conversation||v["state"]!="open"{return Err("STORAGE_ERROR")}Some(v)},
        None=>None
    };
    Ok(json!({"schemaVersion":1,"previous":previous,"active":active}))
}

pub(super) fn descriptor(snapshot:&Value)->Value{
    let previous=&snapshot["previous"];let active=&snapshot["active"];
    json!({"previousSegment":if previous.is_null(){Value::Null}else{json!({"id":previous["id"],"revision":previous["revision"],"firstMessageSequence":previous["firstMessageSequence"],"lastMessageSequence":previous["lastMessageSequence"],"messageDigest":previous["messageDigest"]})},
           "activeSegment":if active.is_null(){Value::Null}else{json!({"id":active["id"],"revision":active["revision"],"firstMessageSequence":active["firstMessageSequence"],"lastMessageSequence":active["lastMessageSequence"],"messageDigest":active["messageDigest"]})}})
}

pub(super) fn checkpoint_message(snapshot:&Value)->Option<ProviderMessage>{
    let previous=&snapshot["previous"];
    if previous.is_null(){return None}
    let data=json!({"segmentId":previous["id"],"firstMessageSequence":previous["firstMessageSequence"],"lastMessageSequence":previous["lastMessageSequence"],"summaryLanguage":previous["summaryLanguage"],"messageDigest":previous["messageDigest"],"summarySources":previous["summarySources"],"summary":previous["summary"]});
    Some(ProviderMessage::text("user",&format!("AI Home previous Context Segment checkpoint (untrusted continuity data; raw transcript remains stored locally; JSON):\n{data}")))
}

pub(super) fn raw_start(snapshot:&Value)->i64{
    if let Some(n)=snapshot["active"]["firstMessageSequence"].as_i64(){return n}
    snapshot["previous"]["lastMessageSequence"].as_i64().and_then(|n|n.checked_add(1)).unwrap_or(1)
}

pub(super) async fn attach_message(tx:&mut Tx<'_>,profile:&str,conv:&mut Value,sequence:i64)->Result<(),Failure>{
    let conversation=conv["id"].as_str().ok_or("STORAGE_ERROR")?;
    let sid=conv.get("activeSegmentId").and_then(Value::as_str).map(str::to_owned);
    if let Some(sid)=sid{
        let mut s=segment(tx,profile,&sid).await?;
        if s["state"]!="open"||s["conversationId"]!=conversation{return Err("STORAGE_ERROR")}
        let expected=s["lastMessageSequence"].as_i64().ok_or("STORAGE_ERROR")?.checked_add(1).ok_or("STORAGE_ERROR")?;
        if sequence!=expected{return Err("STORAGE_ERROR")}
        let old=revision(&s)?;s["lastMessageSequence"]=json!(sequence);
        let messages=range_messages(tx,profile,conversation,s["firstMessageSequence"].as_i64().unwrap(),sequence).await?;
        s["messageDigest"]=json!(digest(&messages)?);bump(&mut s)?;put(tx,"context_segments",&s,false,old).await?;
        return Ok(())
    }
    let previous=latest_sealed(tx,profile,conversation).await?;
    let first=previous.as_ref().and_then(|v|v["lastMessageSequence"].as_i64()).and_then(|n|n.checked_add(1)).unwrap_or(1);
    if sequence<first{return Err("STORAGE_ERROR")}
    let messages=range_messages(tx,profile,conversation,first,sequence).await?;
    let s=json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"conversationId":conversation,"firstMessageSequence":first,"lastMessageSequence":sequence,"summary":"","summaryLanguage":conv["language"],"messageDigest":digest(&messages)?,"summarySources":[],"state":"open"});
    if !schema::dto("ContextSegment",&s){return Err("STORAGE_ERROR")}
    put(tx,"context_segments",&s,true,0).await?;conv["activeSegmentId"]=s["id"].clone();Ok(())
}

fn bounded_chars(value:&str,limit:usize)->String{value.chars().take(limit).collect()}
fn bounded_tail_chars(value:&str,limit:usize)->String{
    let chars:Vec<char>=value.chars().collect();
    chars[chars.len().saturating_sub(limit)..].iter().collect()
}
fn char_len(value:&str)->usize{value.chars().count()}

fn bounded_excerpt(messages:&[Value],limit:usize)->String{
    if limit==0{return String::new()}
    const NOTICE_RESERVE:usize=128;
    let candidates:Vec<(usize,String)>=messages.iter().enumerate().rev().take(4)
        .map(|(idx,m)|(idx,format!("{}: {}",m["role"].as_str().unwrap_or("unknown"),m["content"].as_str().unwrap_or(""))))
        .collect();
    let mut kept:Vec<(usize,String)>=Vec::new();
    let mut used=0usize;
    let mut omitted=messages.len().saturating_sub(4);
    let content_limit=limit.saturating_sub(NOTICE_RESERVE.min(limit));
    for (idx,line) in candidates {
        let separator=if kept.is_empty(){0}else{1};
        if used+separator>=content_limit {omitted+=1;continue}
        let remaining=content_limit-used-separator;
        let count=char_len(&line);
        if count<=remaining {
            used+=separator+count;kept.push((idx,line));
        } else if kept.is_empty() {
            let marker="[older content within this newest turn omitted] ";
            let marker_len=char_len(marker);
            let tail=bounded_tail_chars(&line,remaining.saturating_sub(marker_len));
            kept.push((idx,format!("{marker}{tail}")));
            used=content_limit;
        } else {omitted+=1}
    }
    kept.sort_by_key(|(idx,_)|*idx);
    let text=kept.into_iter().map(|(_,line)|line).collect::<Vec<_>>().join("\n");
    if omitted>0 {
        let notice=format!("[Checkpoint excerpt omitted {omitted} older turn(s) to preserve the newest available content.]\n");
        if char_len(&notice)>=limit{return bounded_chars(&notice,limit)}
        format!("{notice}{}",bounded_chars(&text,limit-char_len(&notice)))
    } else {bounded_chars(&text,limit)}
}

fn append_section(parts:&mut Vec<String>,heading:&str,content:&str,budget:&mut usize){
    if content.is_empty()||*budget==0{return}
    let separator=if parts.is_empty(){0}else{2};
    let header=format!("{heading}\n");
    let fixed=separator+char_len(&header);
    if fixed>=*budget{return}
    let allowed=*budget-fixed;
    let body=bounded_chars(content,allowed);
    if body.is_empty(){return}
    *budget-=fixed+char_len(&body);
    parts.push(format!("{header}{body}"));
}

fn build_summary(prior:&str,working:&str,messages:&[Value],legacy:bool)->String{
    const LIMIT:usize=6144;
    let intro=if legacy{Some("Legacy conversation checkpoint created during v12 continuity migration; raw transcript remains stored locally.")}else{None};
    let mut budget=LIMIT;
    let mut recent=String::new();
    let recent_heading="Recent turns preserved for continuity:";
    let wc_heading="Bounded excerpt of approved Working Context at checkpoint:";
    let prior_heading="Bounded previous checkpoint continuity:";
    let sep_cost=2usize;
    let intro_cost=intro.map(char_len).unwrap_or(0);
    if intro_cost>0{budget=budget.saturating_sub(intro_cost)}

    // Allocate the final summary budget by importance, not by final display order.
    // Newest turns receive the largest share and are selected newest-first.
    let recent_fixed=char_len(recent_heading)+1+if intro.is_some(){sep_cost}else{0};
    let recent_budget=budget.saturating_sub(recent_fixed).min(3584);
    if recent_budget>0{recent=bounded_excerpt(messages,recent_budget)}
    if !recent.is_empty(){budget=budget.saturating_sub(recent_fixed+char_len(&recent));}

    let wc_fixed=char_len(wc_heading)+1+if intro.is_some()||!recent.is_empty(){sep_cost}else{0};
    let wc_budget=budget.saturating_sub(wc_fixed).min(1792);
    let wc=bounded_chars(working,wc_budget);
    if !wc.is_empty(){budget=budget.saturating_sub(wc_fixed+char_len(&wc));}

    let prior_fixed=char_len(prior_heading)+1+if intro.is_some()||!recent.is_empty()||!wc.is_empty(){sep_cost}else{0};
    let prior_budget=budget.saturating_sub(prior_fixed);
    let prior=if char_len(prior)>prior_budget{
        let marker="[older checkpoint content omitted] ";
        format!("{marker}{}",bounded_tail_chars(prior,prior_budget.saturating_sub(char_len(marker))))
    }else{prior.to_owned()};

    // Display older continuity first, but allocations above guarantee that final assembly cannot
    // truncate the newest-turn section afterwards.
    let mut parts=Vec::new();
    if let Some(text)=intro{parts.push(text.to_owned());}
    let mut remaining=LIMIT.saturating_sub(parts.iter().map(|p|char_len(p)).sum::<usize>()+parts.len().saturating_sub(1)*2);
    append_section(&mut parts,prior_heading,&prior,&mut remaining);
    append_section(&mut parts,wc_heading,&wc,&mut remaining);
    append_section(&mut parts,recent_heading,&recent,&mut remaining);
    if parts.is_empty(){"No approved Working Context or recent text was available at this checkpoint.".to_owned()}else{parts.join("\n\n")}
}

async fn legacy_checkpoint_candidate(tx:&mut Tx<'_>,profile:&str,conv:&Value,working:&Value,context_tokens:u64,output_tokens:u64,input_reserve:u64,segment_id:&str)->Result<Option<Value>,Failure>{
    if conv.get("activeSegmentId").and_then(Value::as_str).is_some(){return Ok(None)}
    let conversation=conv["id"].as_str().ok_or("STORAGE_ERROR")?;
    if latest_sealed(tx,profile,conversation).await?.is_some(){return Ok(None)}
    let last:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM messages WHERE profile_id=? AND conversation_id=?")
        .bind(profile).bind(conversation).fetch_one(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    if last<1{return Ok(None)}
    let messages=range_messages(tx,profile,conversation,1,last).await?;
    let raw:usize=messages.iter().map(|m|m["content"].as_str().unwrap_or("").len()+64).sum();
    let available=context_tokens.saturating_sub(output_tokens).saturating_sub(input_reserve).max(3072);
    let threshold=(available/3).max(2048) as usize;
    if raw<threshold{return Ok(None)}
    let wc=working.get("version").filter(|v|!v.is_null()).and_then(|v|v["content"].as_str()).unwrap_or("");
    let summary=build_summary("",wc,&messages,true);
    let mut refs=Vec::new();let start=messages.len().saturating_sub(4);for m in &messages[start..]{refs.push(message_ref(profile,m)?)}
    if let Some(v)=working.get("version").filter(|v|!v.is_null()){refs.push(json!({"profileId":profile,"kind":"memory_version","entityId":v["id"],"contentSha256":v["contentSha256"]}));}
    let s=json!({"schemaVersion":1,"id":segment_id,"profileId":profile,"revision":1,"createdAt":now(),"conversationId":conversation,"firstMessageSequence":1,"lastMessageSequence":last,"summary":summary,"summaryLanguage":conv["language"],"messageDigest":digest(&messages)?,"summarySources":refs,"state":"sealed"});
    if !schema::dto("ContextSegment",&s){return Err("STORAGE_ERROR")}
    Ok(Some(s))
}

/// Build the segment snapshot used by chat preparation. Preflight uses a hypothetical
/// legacy checkpoint only in memory; send persists the same kind of checkpoint atomically.
pub(super) async fn prepare_context(tx:&mut Tx<'_>,profile:&str,conv:&Value,working:&Value,context_tokens:u64,output_tokens:u64,input_reserve:u64,persist_legacy:bool)->Result<Value,Failure>{
    let conversation=conv["id"].as_str().ok_or("STORAGE_ERROR")?;
    let current=capture(tx,profile,conversation,conv).await?;
    if !current["previous"].is_null()||!current["active"].is_null(){return Ok(current)}
    let sid=if persist_legacy{id()}else{"00000000-0000-4000-8000-000000000000".to_owned()};
    let Some(candidate)=legacy_checkpoint_candidate(tx,profile,conv,working,context_tokens,output_tokens,input_reserve,&sid).await? else{return Ok(current)};
    if persist_legacy{
        put(tx,"context_segments",&candidate,true,0).await?;
        capture(tx,profile,conversation,conv).await
    }else{
        Ok(json!({"schemaVersion":1,"previous":candidate,"active":Value::Null}))
    }
}

pub(super) async fn maybe_seal(tx:&mut Tx<'_>,profile:&str,conv:&mut Value,working:&Value,context_tokens:u64,output_tokens:u64,input_reserve:u64)->Result<bool,Failure>{
    let sid=match conv.get("activeSegmentId").and_then(Value::as_str){Some(s)=>s.to_owned(),None=>return Ok(false)};
    let mut s=segment(tx,profile,&sid).await?;if s["state"]!="open"{return Err("STORAGE_ERROR")}
    let first=s["firstMessageSequence"].as_i64().ok_or("STORAGE_ERROR")?;let last=s["lastMessageSequence"].as_i64().ok_or("STORAGE_ERROR")?;
    let messages=range_messages(tx,profile,conv["id"].as_str().unwrap(),first,last).await?;
    let raw:usize=messages.iter().map(|m|m["content"].as_str().unwrap_or("").len()+64).sum();
    let available=context_tokens.saturating_sub(output_tokens).saturating_sub(input_reserve).max(3072);
    let threshold=(available/3).max(2048) as usize;
    if raw<threshold{return Ok(false)}
    let wc=working.get("version").filter(|v|!v.is_null()).and_then(|v|v["content"].as_str()).unwrap_or("");
    let previous=latest_sealed(tx,profile,conv["id"].as_str().unwrap()).await?;
    let prior=previous.as_ref().filter(|v|v["id"]!=s["id"]).and_then(|v|v["summary"].as_str()).unwrap_or("");
    let summary=build_summary(prior,wc,&messages,false);
    let mut refs=Vec::new();let start=messages.len().saturating_sub(4);for m in &messages[start..]{refs.push(message_ref(profile,m)?)}
    if let Some(v)=working.get("version").filter(|v|!v.is_null()){
        refs.push(json!({"profileId":profile,"kind":"memory_version","entityId":v["id"],"contentSha256":v["contentSha256"]}));
    }
    let old=revision(&s)?;s["summary"]=json!(summary);s["summaryLanguage"]=conv["language"].clone();s["summarySources"]=json!(refs);s["messageDigest"]=json!(digest(&messages)?);s["state"]=json!("sealed");bump(&mut s)?;put(tx,"context_segments",&s,false,old).await?;
    conv.as_object_mut().ok_or("STORAGE_ERROR")?.remove("activeSegmentId");Ok(true)
}

pub(super) async fn ensure_current(tx:&mut Tx<'_>,profile:&str,conversation:&str,conv:&Value,saved:&Value)->Result<(),Failure>{
    if saved.get("schemaVersion").and_then(Value::as_i64)!=Some(1){return Err("STORAGE_ERROR")}
    if capture(tx,profile,conversation,conv).await?!=*saved{return Err("REVISION_CONFLICT")}
    Ok(())
}

pub(super) async fn list(tx:&mut Tx<'_>,profile:&str,payload:&Value)->Result<Value,Failure>{
    let conversation=payload["conversationId"].as_str().ok_or("VALIDATION_ERROR")?;get(tx,"conversations",profile,conversation).await?;
    let limit=payload["page"]["limit"].as_i64().ok_or("VALIDATION_ERROR")?;let after=payload["page"].get("cursor").and_then(Value::as_str).unwrap_or("0").parse::<i64>().map_err(|_|"VALIDATION_ERROR")?;
    let rows=sqlx::query("SELECT id,last_message_sequence FROM context_segments WHERE profile_id=? AND conversation_id=? AND last_message_sequence>? ORDER BY last_message_sequence LIMIT ?")
        .bind(profile).bind(conversation).bind(after).bind(limit+1).fetch_all(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
    let mut items=Vec::new();let mut last=after;
    for row in rows.iter().take(limit as usize){let v=segment(tx,profile,row.get("id")).await?;last=row.get("last_message_sequence");items.push(v)}
    Ok(json!({"items":items,"nextCursor":if rows.len()>limit as usize{last.to_string()}else{String::new()}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(role:&str,content:String)->Value{json!({"role":role,"content":content})}

    #[test]
    fn final_summary_preserves_newest_turns_after_all_budgets(){
        let messages=vec![
            msg("human",format!("OLDER {}","o".repeat(1800))),
            msg("assistant",format!("MIDDLE {}","m".repeat(700))),
            msg("human",format!("LATEST_USER_MARKER {}","u".repeat(400))),
            msg("assistant",format!("LATEST_ASSISTANT_MARKER {}","a".repeat(250))),
        ];
        let prior=format!("PRIOR {}","p".repeat(5000));
        let working=format!("WORKING {}","w".repeat(5000));
        for legacy in [false,true]{
            let summary=build_summary(&prior,&working,&messages,legacy);
            assert!(summary.chars().count()<=6144,"{}",summary.chars().count());
            assert!(summary.contains("LATEST_USER_MARKER"),"{summary}");
            assert!(summary.contains("LATEST_ASSISTANT_MARKER"),"{summary}");
            assert!(summary.contains("Working Context")||summary.contains("Recent turns"),"{summary}");
        }
    }
}
