use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use sqlx::Row;
use tokio::sync::Mutex;
use crate::{domain::schema,kernel::Kernel,persistence::Store};
pub struct Application {store:Store,kernel:Kernel,gate:Mutex<()>,connections:crate::connections::Connections}
pub struct Dispatch {pub response:Value,pub event:Option<Value>}
fn error(r:&Value,code:&str)->Value{json!({"contractVersion":1,"requestId":r["requestId"].as_str().filter(|s|!s.is_empty()&&s.len()<=128).unwrap_or("invalid-request"),"ok":false,"error":{"code":code,"messageKey":format!("error.{}",code.to_lowercase()),"retryable":false}})}
fn success(r:&Value,result:Value)->Value{json!({"contractVersion":1,"requestId":r["requestId"],"ok":true,"result":result})}
fn embedded(s:&str)->Value{serde_json::from_str(s).expect("embedded JSON")}
fn pack(kind:&str,value:&Value)->Result<Vec<String>,&'static str>{
    if !schema::dto(if kind=="locale"{"LocalePack"}else{"ThemePack"},value){return Err("PACK_INVALID")}
    if kind=="locale"{
        let en=embedded(include_str!("../../src/locales/en.json"));let expected=en["strings"].as_object().unwrap();let supplied=value["strings"].as_object().unwrap();
        if supplied.keys().any(|k|!expected.contains_key(k)){return Err("PACK_INVALID")}
        return Ok(expected.keys().filter(|k|!supplied.contains_key(*k)).map(|k|format!("fallback.en:{k}")).collect())
    }
    Ok(vec![])
}
impl Application {
    pub fn new(store:Store)->Self{Self::with_vault(store,crate::vault::Vault::os())}
    pub fn with_vault(store:Store,vault:crate::vault::Vault)->Self{let mut kernel=Kernel::new();kernel.ready();let connections=crate::connections::Connections::new(store.pool.clone(),vault);Self{store,kernel,gate:Mutex::new(()),connections}}
    pub fn stop(&mut self){self.kernel.stop();}
    pub async fn dispatch(&self,r:Value)->Dispatch{
        let _guard=self.gate.lock().await;
        match self.execute(&r).await{Ok(d)=>d,Err(_)=>Dispatch{response:error(&r,"STORAGE_ERROR"),event:None}}
    }
    async fn execute(&self,r:&Value)->Result<Dispatch,sqlx::Error>{
        let reject=|code|Dispatch{response:error(r,code),event:None};
        if r["contractVersion"]!=1{return Ok(reject("UNSUPPORTED_CONTRACT_VERSION"))}
        if !schema::dto("CommandEnvelope",r){return Ok(reject("VALIDATION_ERROR"))}
        if crate::connections::supports(r["command"].as_str().unwrap()){return Ok(Dispatch{response:self.connections.dispatch(r.clone()).await,event:None})}
        let command=r["command"].as_str().unwrap();
        let catalog=embedded(include_str!("../../contracts/ipc.catalog.json"));let foundation=embedded(include_str!("../../contracts/foundation.ipc.json"));
        let implemented=["app.preferences.get","app.preferences.set","app.pack.validate","kernel.status.get","app.events.list"];
        if !implemented.contains(&command){return Ok(reject("UNKNOWN_COMMAND"))}
        if r.get("profileId").is_some(){return Ok(reject("VALIDATION_ERROR"))}
        let contract=catalog["commands"].get(command).or_else(||foundation["commands"].get(command)).unwrap();
        if !schema::valid(&r["payload"],&contract["payload"]){return Ok(reject("VALIDATION_ERROR"))}
        let result=match command{
            "app.preferences.get"=>self.store.preferences().await?,
            "kernel.status.get"=>self.kernel.status(self.store.sequence().await?),
            "app.events.list"=>self.store.events(r["payload"]["afterSequence"].as_i64().unwrap(),r["payload"]["limit"].as_i64().unwrap()).await?,
            "app.pack.validate"=>match pack(r["payload"]["kind"].as_str().unwrap(),&r["payload"]["pack"]){Ok(w)=>json!({"valid":true,"warnings":w}),Err(code)=>return Ok(reject(code))},
            "app.preferences.set"=>return self.set(r).await,
            _=>unreachable!()
        };
        Ok(Dispatch{response:success(r,result),event:None})
    }
    async fn set(&self,r:&Value)->Result<Dispatch,sqlx::Error>{
        let reject=|code|Dispatch{response:error(r,code),event:None};
        let Some(expected)=r["expectedRevision"].as_i64() else{return Ok(reject("VALIDATION_ERROR"))};
        let p=&r["payload"];
        if !["en","pl"].contains(&p["locale"].as_str().unwrap_or(""))||!["orbital","aurora"].contains(&p["themeId"].as_str().unwrap_or("")){return Ok(reject("VALIDATION_ERROR"))}
        let fingerprint=format!("{:x}",Sha256::digest(json!([r["command"],expected,p["locale"],p["themeId"],p["themeVariant"]]).to_string().as_bytes()));
        let mut tx=self.store.pool.begin().await?;
        if let Some(row)=sqlx::query("SELECT fingerprint,response FROM receipts WHERE request_id=?").bind(r["requestId"].as_str().unwrap()).fetch_optional(&mut *tx).await?{
            let response=if row.get::<&str,_>("fingerprint")==fingerprint{serde_json::from_str(row.get::<&str,_>("response")).map_err(|_|sqlx::Error::Protocol("invalid receipt".into()))?}else{error(r,"IDEMPOTENCY_CONFLICT")};
            if response["ok"]==true && (!schema::dto("AppPreferences",&response["result"])||response["requestId"]!=r["requestId"]||response["contractVersion"]!=1){return Err(sqlx::Error::Protocol("invalid receipt".into()))}
            return Ok(Dispatch{response,event:None})
        }
        let current:i64=sqlx::query_scalar("SELECT revision FROM preferences WHERE id=1").fetch_one(&mut *tx).await?;
        if current!=expected{return Ok(reject("REVISION_CONFLICT"))}
        let Some(revision)=current.checked_add(1).filter(|n|*n<=9_007_199_254_740_991) else{return Ok(reject("VALIDATION_ERROR"))};
        let preferences=json!({"schemaVersion":1,"revision":revision,"locale":p["locale"],"themeId":p["themeId"],"themeVariant":p["themeVariant"]});
        let update=sqlx::query("UPDATE preferences SET revision=?,body=? WHERE id=1 AND revision=?").bind(revision).bind(preferences.to_string()).bind(expected).execute(&mut *tx).await?;
        if update.rows_affected()!=1{return Ok(reject("REVISION_CONFLICT"))}
        let row=sqlx::query("INSERT INTO app_events(body) VALUES('')").execute(&mut *tx).await?;
        let sequence=row.last_insert_rowid();
        let event=json!({"contractVersion":1,"eventId":format!("{:x}",Sha256::digest(format!("{}:{sequence}",r["requestId"]).as_bytes())),"sequence":sequence,"eventType":"app.preferences.changed","occurredAt":chrono::Utc::now().to_rfc3339(),"payload":preferences});
        sqlx::query("UPDATE app_events SET body=? WHERE sequence=?").bind(event.to_string()).bind(sequence).execute(&mut *tx).await?;
        let response=success(r,preferences);
        sqlx::query("INSERT INTO receipts(request_id,fingerprint,response) VALUES(?,?,?)").bind(r["requestId"].as_str().unwrap()).bind(fingerprint).bind(response.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Dispatch{response,event:Some(event)})
    }
}

impl Drop for Application { fn drop(&mut self) { self.kernel.stop(); } }
