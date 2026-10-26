use base64::{engine::general_purpose::STANDARD,Engine as _};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::Row;
use crate::persistence::Store;

async fn count(store:&Store,sql:&str)->Result<i64,sqlx::Error>{sqlx::query_scalar(sql).fetch_one(&store.pool).await}

pub async fn snapshot(store:&Store)->Result<Value,sqlx::Error>{
    let prefs=store.preferences().await?;
    let profiles=count(store,"SELECT COUNT(*) FROM profiles").await?;
    let conversations=count(store,"SELECT COUNT(*) FROM conversations").await?;
    let memory=count(store,"SELECT COUNT(*) FROM memory_records").await?;
    let operations=count(store,"SELECT COUNT(*) FROM operations").await?;
    let failed=count(store,"SELECT COUNT(*) FROM operations WHERE json_extract(body,'$.status') IN ('failed','interrupted')").await?;
    let rows=sqlx::query("SELECT json_extract(body,'$.error.code') code,COUNT(*) n FROM operations WHERE json_extract(body,'$.error.code') IS NOT NULL GROUP BY code ORDER BY n DESC,code LIMIT 20").fetch_all(&store.pool).await?;
    let recent=rows.into_iter().filter_map(|r|{let code:Option<String>=r.try_get("code").ok();let n:Option<i64>=r.try_get("n").ok();code.zip(n).map(|(code,count)|json!({"code":code,"count":count}))}).collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion":1,
        "generatedAt":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis,true),
        "appVersion":env!("CARGO_PKG_VERSION"),
        "storageSchemaVersion":15,
        "mode":"standard",
        "stateProvider":"null",
        "preferences":{"locale":prefs["locale"],"themeId":prefs["themeId"],"themeVariant":prefs["themeVariant"]},
        "counts":{"profiles":profiles,"conversations":conversations,"memoryRecords":memory,"operations":operations,"failedOperations":failed},
        "recentErrors":recent,
        "contentIncluded":false
    }))
}

pub async fn export(store:&Store)->Result<Value,sqlx::Error>{
    let value=snapshot(store).await?;
    let bytes=serde_json::to_vec_pretty(&json!({
        "diagnosticsVersion":1,
        "privacy":{"contentIncluded":false,"credentialsIncluded":false,"recordIdentifiersIncluded":false,"providerPayloadsIncluded":false},
        "snapshot":value
    })).map_err(|_|sqlx::Error::Protocol("diagnostics serialization failed".into()))?;
    let sha=format!("{:x}",Sha256::digest(&bytes));
    Ok(json!({"fileName":"ai-home-diagnostics.json","mimeType":"application/json","dataBase64":STANDARD.encode(&bytes),"sizeBytes":bytes.len(),"sha256":sha,"contentIncluded":false}))
}
