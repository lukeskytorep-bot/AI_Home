//! Profile-scoped conversation storage. No provider calls or synthetic messages.
use super::{bump, event, get, id, now, put, revision, Failure, Tx};
use crate::domain::schema;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
async fn changed(tx: &mut Tx<'_>, profile: &str) -> Result<(), Failure> {
    let seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?",
    )
    .bind(profile)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| "STORAGE_ERROR")?;
    event(
        tx,
        profile,
        "snapshot.required",
        &json!({"lastAvailableSequence":seq}),
    )
    .await
}
pub(super) async fn execute(
    tx: &mut Tx<'_>,
    command: &str,
    profile: &str,
    payload: &Value,
    expected: i64,
) -> Result<Value, Failure> {
    match command {
        "conversation.create" => {
            if payload["title"].as_str().unwrap().len() > 512 {
                return Err("VALIDATION_ERROR");
            }
            let v = json!({"schemaVersion":1,"id":id(),"profileId":profile,"revision":1,"createdAt":now(),"title":payload["title"],"language":payload["language"],"status":"open","draftText":""});
            if !schema::dto("Conversation", &v) {
                return Err("VALIDATION_ERROR");
            }
            put(tx, "conversations", &v, true, 0).await?;
            changed(tx, profile).await?;
            Ok(v)
        }
        "conversation.draft.set" => {
            let mut v = get(
                tx,
                "conversations",
                profile,
                payload["conversationId"].as_str().unwrap(),
            )
            .await?;
            if revision(&v)? != expected {
                return Err("REVISION_CONFLICT");
            }
            if v["status"] != "open" {
                return Err("POLICY_DENIED");
            }
            v["draftText"] = payload["text"].clone();
            bump(&mut v)?;
            put(tx, "conversations", &v, false, expected).await?;
            changed(tx, profile).await?;
            Ok(v)
        }
        "conversation.archive" => {
            let mut v=get(tx,"conversations",profile,payload["conversationId"].as_str().unwrap()).await?;
            if revision(&v)? != expected { return Err("REVISION_CONFLICT"); }
            let archived=payload["archived"].as_bool().ok_or("VALIDATION_ERROR")?;
            if archived != (v["status"] == "archived") {
                let old=revision(&v)?; bump(&mut v)?; v["status"]=json!(if archived{"archived"}else{"open"});
                put(tx,"conversations",&v,false,old).await?; changed(tx,profile).await?;
            }
            Ok(v)
        }
        "conversation.get" => {
            get(
                tx,
                "conversations",
                profile,
                payload["conversationId"].as_str().unwrap(),
            )
            .await
        }
        "conversation.list" | "conversation.archived.list" | "conversation.messages.list" => {
            let messages = command == "conversation.messages.list";
            let archived = command == "conversation.archived.list";
            let conversation = if messages {
                payload["conversationId"].as_str().unwrap()
            } else {
                ""
            };
            if messages {
                get(tx, "conversations", profile, conversation).await?;
            }
            let page = &payload["page"];
            let limit = page["limit"].as_i64().unwrap();
            let after = if let Some(token) = page["cursor"].as_str() {
                let row=sqlx::query("SELECT profile_id,conversation_id,kind,after_key,page_limit FROM conversation_cursors WHERE id=?").bind(token).fetch_optional(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?.ok_or("VALIDATION_ERROR")?;
                if row.get::<&str, _>("profile_id") != profile
                    || row.get::<&str, _>("conversation_id") != conversation
                    || row.get::<&str, _>("kind") != command
                    || row.get::<i64, _>("page_limit") != limit
                {
                    return Err("VALIDATION_ERROR");
                }
                row.get::<String, _>("after_key")
            } else {
                if messages { "0" } else { "" }.into()
            };
            let rows=if messages {
    sqlx::query("SELECT id,sequence FROM messages WHERE profile_id=? AND conversation_id=? AND sequence>? ORDER BY sequence LIMIT ?").bind(profile).bind(conversation).bind(after.parse::<i64>().map_err(|_|"STORAGE_ERROR")?).bind(limit+1).fetch_all(&mut **tx).await
   }else{
    sqlx::query("SELECT id FROM conversations WHERE profile_id=? AND id>? AND (json_extract(body,'$.status')='archived')=? ORDER BY id LIMIT ?").bind(profile).bind(&after).bind(archived).bind(limit+1).fetch_all(&mut **tx).await
   }.map_err(|_|"STORAGE_ERROR")?;
            let mut items = vec![];
            let mut last = String::new();
            for row in rows.iter().take(limit as usize) {
                let v = get(
                    tx,
                    if messages {
                        "messages"
                    } else {
                        "conversations"
                    },
                    profile,
                    row.get("id"),
                )
                .await?;
                if messages
                    && (v["conversationId"] != conversation
                        || v["sequence"].as_i64() != Some(row.get("sequence")))
                {
                    return Err("STORAGE_ERROR");
                }
                last = if messages {
                    row.get::<i64, _>("sequence").to_string()
                } else {
                    row.get::<String, _>("id")
                };
                items.push(v);
            }
            let cursor = if rows.len() > limit as usize {
                let token = format!(
                    "{:x}",
                    Sha256::digest(
                        json!([command, profile, conversation, limit, last])
                            .to_string()
                            .as_bytes()
                    )
                );
                sqlx::query("INSERT OR IGNORE INTO conversation_cursors(id,profile_id,conversation_id,kind,after_key,page_limit) VALUES(?,?,?,?,?,?)").bind(&token).bind(profile).bind(conversation).bind(command).bind(last).bind(limit).execute(&mut **tx).await.map_err(|_|"STORAGE_ERROR")?;
                token
            } else {
                String::new()
            };
            Ok(json!({"items":items,"nextCursor":cursor}))
        }
        _ => Err("UNKNOWN_COMMAND"),
    }
}
