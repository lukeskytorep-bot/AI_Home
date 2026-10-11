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
        if version>1{return Err(sqlx::Error::Protocol("unsupported storage version".into()))}
        sqlx::query("CREATE TABLE IF NOT EXISTS preferences (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL, body TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS receipts (request_id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, response TEXT NOT NULL)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS app_events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT NOT NULL)").execute(&mut *tx).await?;
        let d:Value=serde_json::from_str(include_str!("../../contracts/defaults.json")).unwrap();
        sqlx::query("INSERT OR IGNORE INTO preferences(id,revision,body) VALUES(1,1,?)").bind(d["appPreferences"].to_string()).execute(&mut *tx).await?;
        sqlx::query("PRAGMA user_version=1").execute(&mut *tx).await?;tx.commit().await?;
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
