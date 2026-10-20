use serde_json::{json,Value};
use sqlx::{sqlite::{SqliteConnectOptions,SqlitePoolOptions},Row,SqlitePool};
use std::path::Path;
use crate::domain::schema;
pub struct Store {pub pool:SqlitePool}
impl Store {
    pub async fn open(path:&Path)->Result<Self,sqlx::Error>{
        let options=SqliteConnectOptions::new().filename(path).create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal).busy_timeout(std::time::Duration::from_secs(5));
        Self::connect(options).await
    }
    pub async fn memory()->Result<Self,sqlx::Error>{Self::connect(SqliteConnectOptions::new().in_memory(true)).await}
    async fn connect(options:SqliteConnectOptions)->Result<Self,sqlx::Error>{
        let pool=SqlitePoolOptions::new().max_connections(1).connect_with(options).await?;
        let mut tx=pool.begin().await?;
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut *tx).await?;
        if version>7{return Err(sqlx::Error::Protocol("unsupported storage version".into()))}
        sqlx::query("CREATE TABLE IF NOT EXISTS preferences (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, body TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS receipts (request_id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, response TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS app_events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL)").execute(&mut *tx).await?;
        let d:Value=serde_json::from_str(include_str!("../../contracts/defaults.json")).unwrap();
        sqlx::query("INSERT OR IGNORE INTO preferences(id,revision,body) VALUES(1,1,?)").bind(d["appPreferences"].to_string()).execute(&mut *tx).await?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS message_documents (profile_id TEXT NOT NULL, message_id TEXT NOT NULL, ordinal INTEGER NOT NULL, attachment_id TEXT NOT NULL, material_id TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,message_id,ordinal), UNIQUE(profile_id,attachment_id), FOREIGN KEY(profile_id,message_id) REFERENCES messages(profile_id,id), FOREIGN KEY(profile_id,attachment_id) REFERENCES attachments(profile_id,id), FOREIGN KEY(profile_id,material_id) REFERENCES import_materials(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS import_blobs (profile_id TEXT NOT NULL REFERENCES profiles(id), id TEXT NOT NULL, sha256 TEXT NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS import_materials (profile_id TEXT NOT NULL, id TEXT NOT NULL, conversation_id TEXT NOT NULL, operation_id TEXT NOT NULL, original_blob_id TEXT NOT NULL, extracted_blob_id TEXT, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id), UNIQUE(profile_id,id,conversation_id), FOREIGN KEY(profile_id,conversation_id) REFERENCES conversations(profile_id,id), FOREIGN KEY(profile_id,operation_id) REFERENCES operations(profile_id,id), FOREIGN KEY(profile_id,original_blob_id) REFERENCES import_blobs(profile_id,id), FOREIGN KEY(profile_id,extracted_blob_id) REFERENCES import_blobs(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS attachments (profile_id TEXT NOT NULL, id TEXT NOT NULL, conversation_id TEXT NOT NULL, material_id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id), FOREIGN KEY(profile_id,conversation_id) REFERENCES conversations(profile_id,id), FOREIGN KEY(profile_id,material_id,conversation_id) REFERENCES import_materials(profile_id,id,conversation_id))",
            "CREATE TABLE IF NOT EXISTS chat_policies (profile_id TEXT NOT NULL, route_id TEXT NOT NULL, route_revision INTEGER NOT NULL, context_tokens INTEGER NOT NULL CHECK(context_tokens>=1024), PRIMARY KEY(profile_id,route_id), FOREIGN KEY(profile_id,route_id) REFERENCES routes(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS chat_runs (profile_id TEXT NOT NULL, operation_id TEXT NOT NULL, conversation_id TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,operation_id), FOREIGN KEY(profile_id,operation_id) REFERENCES operations(profile_id,id), FOREIGN KEY(profile_id,conversation_id) REFERENCES conversations(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS chat_replay (profile_id TEXT NOT NULL, message_id TEXT NOT NULL, scope TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,message_id), FOREIGN KEY(profile_id,message_id) REFERENCES messages(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS conversations (profile_id TEXT NOT NULL REFERENCES profiles(id), id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS messages (profile_id TEXT NOT NULL, id TEXT NOT NULL, conversation_id TEXT NOT NULL, sequence INTEGER NOT NULL CHECK(sequence>0), revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id), UNIQUE(profile_id,conversation_id,sequence), FOREIGN KEY(profile_id,conversation_id) REFERENCES conversations(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS conversation_cursors (id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, conversation_id TEXT NOT NULL, kind TEXT NOT NULL, after_key TEXT NOT NULL, page_limit INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS installation_hmac (id INTEGER PRIMARY KEY CHECK(id=1), credential_ref TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS profile_cursors (id TEXT PRIMARY KEY, after_id TEXT NOT NULL, page_limit INTEGER NOT NULL)",
            "CREATE TABLE IF NOT EXISTS profiles (id TEXT PRIMARY KEY, revision INTEGER NOT NULL, body TEXT NOT NULL)",
            "CREATE TABLE IF NOT EXISTS connections (profile_id TEXT NOT NULL REFERENCES profiles(id), id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS routes (profile_id TEXT NOT NULL, id TEXT NOT NULL, connection_id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id), FOREIGN KEY(profile_id) REFERENCES profiles(id), FOREIGN KEY(profile_id,connection_id) REFERENCES connections(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS operations (profile_id TEXT NOT NULL REFERENCES profiles(id), id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS connection_models (profile_id TEXT NOT NULL, connection_id TEXT NOT NULL, connection_revision INTEGER NOT NULL, credential_revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,connection_id), FOREIGN KEY(profile_id,connection_id) REFERENCES connections(profile_id,id))",
            "CREATE TABLE IF NOT EXISTS profile_events (profile_id TEXT NOT NULL REFERENCES profiles(id), sequence INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(profile_id,sequence))",
        ] {sqlx::query(statement).execute(&mut *tx).await?;}
        // A restarted process cannot resume a network request. Keep a durable safe error.
        let pending=sqlx::query("SELECT profile_id,id,revision,body FROM operations WHERE json_extract(body,'$.status') IN ('queued','running')").fetch_all(&mut *tx).await?;
        for row in pending {
            let mut op:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|sqlx::Error::Protocol("invalid stored operation".into()))?;
            if !schema::dto("Operation",&op)||op["profileId"]!=row.get::<&str,_>("profile_id")||op["id"]!=row.get::<&str,_>("id")||op["revision"].as_i64()!=Some(row.get::<i64,_>("revision")){return Err(sqlx::Error::Protocol("invalid stored operation".into()))}
            op["status"]=json!("interrupted");op["revision"]=json!(op["revision"].as_i64().unwrap_or(0)+1);
            op["finishedAt"]=json!(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true));
            op["error"]=json!({"code":"OPERATION_INTERRUPTED","messageKey":"error.operation_interrupted","retryable":true,"operationId":op["id"]});
            if !schema::dto("Operation",&op){return Err(sqlx::Error::Protocol("invalid stored operation".into()))}
            sqlx::query("UPDATE operations SET revision=?,body=? WHERE profile_id=? AND id=?").bind(op["revision"].as_i64().unwrap()).bind(op.to_string()).bind(row.get::<&str,_>("profile_id")).bind(row.get::<&str,_>("id")).execute(&mut *tx).await?;
            let profile=row.get::<&str,_>("profile_id");
            let sequence:i64=sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0)+1 FROM profile_events WHERE profile_id=?").bind(profile).fetch_one(&mut *tx).await?;
            let event=serde_json::json!({"contractVersion":1,"eventId":uuid::Uuid::new_v4().to_string(),"profileId":profile,"operationId":op["id"],"sequence":sequence,"eventType":"operation.changed","occurredAt":op["finishedAt"],"payload":op});
            sqlx::query("INSERT INTO profile_events(profile_id,sequence,body) VALUES(?,?,?)").bind(profile).bind(sequence).bind(event.to_string()).execute(&mut *tx).await?;

        }
        // Imported originals survive a crash; interrupted extraction is never presented as ready.
        sqlx::query("UPDATE import_materials SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.parseState','failed','$.errorCode','OPERATION_INTERRUPTED') WHERE json_extract(body,'$.parseState')='pending'").execute(&mut *tx).await?;
        sqlx::query("UPDATE attachments SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.state','failed') WHERE EXISTS(SELECT 1 FROM import_materials m WHERE m.profile_id=attachments.profile_id AND m.id=attachments.material_id AND json_extract(m.body,'$.errorCode')='OPERATION_INTERRUPTED') AND json_extract(body,'$.state')='staged'").execute(&mut *tx).await?;
        sqlx::query("PRAGMA user_version=7").execute(&mut *tx).await?;tx.commit().await?;
        let store=Self{pool};store.preferences().await?;Ok(store)
    }
    pub async fn preferences(&self)->Result<Value,sqlx::Error>{
        let row=sqlx::query("SELECT revision,body FROM preferences WHERE id=1").fetch_one(&self.pool).await?;
        let p:Value=serde_json::from_str(row.get::<&str,_>("body")).map_err(|_|sqlx::Error::Protocol("invalid stored preferences".into()))?;
        if !schema::dto("AppPreferences",&p)||p["revision"].as_i64()!=Some(row.get("revision"))||!["en","pl"].contains(&p["locale"].as_str().unwrap_or(""))||!["orbital","aurora"].contains(&p["themeId"].as_str().unwrap_or("")){return Err(sqlx::Error::Protocol("invalid stored preferences".into()))}
        Ok(p)
    }
    pub async fn sequence(&self)->Result<i64,sqlx::Error>{sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM app_events").fetch_one(&self.pool).await}
    pub async fn events(&self,after:i64,limit:i64)->Result<Value,sqlx::Error>{
        let rows=sqlx::query("SELECT body FROM app_events WHERE sequence>? ORDER BY sequence LIMIT ?").bind(after).bind(limit).fetch_all(&self.pool).await?;
        let items:Vec<Value>=rows.iter().map(|r|serde_json::from_str(r.get::<&str,_>("body"))).collect::<Result<_,_>>().map_err(|_|sqlx::Error::Protocol("invalid event".into()))?;
        if items.iter().any(|v|!schema::dto("AppEvent",v)){return Err(sqlx::Error::Protocol("invalid event".into()))}
        let sequence=self.sequence().await?;
        let last=items.last().and_then(|e|e["sequence"].as_i64()).unwrap_or(after);
        Ok(json!({"items":items,"lastSequence":sequence,"hasMore":last<sequence}))
    }
}
