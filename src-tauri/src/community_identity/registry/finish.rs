//! Native token-safe completion and explicit proof recovery. No IPC or startup work.
use super::super::{
    proof::Proof,
    tokens::{CommunityTokenStore, SecretToken},
};
use super::start::{RegistrationStart, StartError};
use super::*;
use reqwest::Method;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishError {
    Registry(RegistryError),
    Identity(IdentityError),
    Http(StartError),
    RecoveryRequired,
    InvalidResponse,
}
impl From<RegistryError> for FinishError {
    fn from(e: RegistryError) -> Self {
        Self::Registry(e)
    }
}
impl From<IdentityError> for FinishError {
    fn from(e: IdentityError) -> Self {
        Self::Identity(e)
    }
}
impl From<StartError> for FinishError {
    fn from(e: StartError) -> Self {
        Self::Http(e)
    }
}
impl From<sqlx::Error> for FinishError {
    fn from(_: sqlx::Error) -> Self {
        Self::Registry(RegistryError::Storage)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishProgress {
    pub revision: i64,
    pub registration_id: String,
    pub ready_metadata: bool,
    pub agent_id: Option<String>,
    pub generation: Option<i64>,
}
#[derive(Clone)]
struct Record {
    revision: i64,
    body: Value,
}
impl Record {
    fn id(&self) -> &str {
        self.body["registrationId"].as_str().unwrap()
    }
    fn agent(&self) -> Option<&str> {
        self.body["agentId"].as_str()
    }
    fn key(&self) -> Option<&str> {
        self.body["keyId"].as_str()
    }
    fn generation(&self) -> Option<i64> {
        self.body["generation"].as_i64()
    }
    fn ready(&self) -> bool {
        self.body["state"] == "ready"
    }
    fn progress(&self) -> FinishProgress {
        FinishProgress {
            revision: self.revision,
            registration_id: self.id().into(),
            ready_metadata: self.ready(),
            agent_id: self.agent().map(str::to_string),
            generation: self.generation(),
        }
    }
}
#[derive(Clone)]
pub struct RegistrationFinish {
    http: RegistrationStart,
    tokens: CommunityTokenStore,
}
impl RegistrationFinish {
    #[cfg(test)]
    pub(crate) fn set_loopback(&mut self,url:reqwest::Url){self.http.set_loopback(url);}
    pub fn new(timeout: Duration, tokens: CommunityTokenStore) -> Result<Self, FinishError> {
        Ok(Self {
            http: RegistrationStart::new(timeout)?,
            tokens,
        })
    }
    /// First complete only. Re-entry never silently resends an uncertain mutation.
    pub async fn complete(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<FinishProgress, FinishError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation.clone());
        let mut guarded = self.clone();
        guarded.tokens = self.tokens.scoped(operation);
        guarded.complete_inner(registry, &guarded_keys, scope).await
    }
    async fn complete_inner(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<FinishProgress, FinishError> {
        let (intent, context, committed) = archive(registry, keys, scope).await?;
        if let Some(record) = read(registry, scope).await? {
            if record.id() != context.registration_id() {
                return Err(FinishError::InvalidResponse);
            }
            if record.ready() {
                return self.current(registry, keys, scope).await;
            }
            return Err(FinishError::RecoveryRequired);
        }
        let packet = if committed {
            None
        } else {
            Some(
                keys.complete_packet(scope, &context, Utc::now())
                    .await?
                    .to_string(),
            )
        };
        let body = json!({"registrationId":context.registration_id(),"completeBody":packet,"state":"uncertain","agentId":null,"keyId":null,"generation":null,"tokenRef":null});
        let n=sqlx::query("INSERT INTO community_registration_finishes(scope_binding,revision,body) VALUES(?,1,?) ON CONFLICT DO NOTHING").bind(scope.binding()).bind(body.to_string()).execute(&registry.pool).await?.rows_affected();
        if n != 1 {
            return Err(RegistryError::Conflict.into());
        }
        let record = Record { revision: 1, body };
        if committed {
            return Err(FinishError::RecoveryRequired);
        }
        self.send_complete(registry, keys, scope, &intent, &context, record)
            .await
    }
    /// Explicit replay of the already persisted complete bytes, including after
    /// an ambiguous dispatch. Server 4A checks committed before pending expiry.
    pub async fn retry_complete(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<FinishProgress, FinishError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation.clone());
        let mut guarded = self.clone();
        guarded.tokens = self.tokens.scoped(operation);
        guarded
            .retry_complete_inner(registry, &guarded_keys, scope, expected_revision)
            .await
    }
    async fn retry_complete_inner(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<FinishProgress, FinishError> {
        let (intent, context, _) = archive(registry, keys, scope).await?;
        if read(registry, scope).await?.is_some_and(|r| r.ready()) {
            return Err(FinishError::RecoveryRequired);
        }
        let record = claim(registry, scope, expected_revision).await?;
        if record.id() != context.registration_id() {
            return Err(FinishError::InvalidResponse);
        }
        self.send_complete(registry, keys, scope, &intent, &context, record)
            .await
    }
    async fn send_complete(
        &self,
        registry: &RegistrationRegistry,
        _keys: &CommunityKeyStore,
        scope: &CommunityScope,
        intent: &RegistrationIntent,
        context: &ValidatedRegistration,
        mut record: Record,
    ) -> Result<FinishProgress, FinishError> {
        let body = record.body["completeBody"]
            .as_str()
            .ok_or(FinishError::RecoveryRequired)?
            .to_string();
        let packet: Value =
            serde_json::from_str(&body).map_err(|_| FinishError::InvalidResponse)?;
        if body.len() > 4096
            || packet.as_object().is_none_or(|m| m.len() != 3)
            || packet["registration_id"] != context.registration_id()
        {
            return Err(FinishError::InvalidResponse);
        }
        for (name, key) in [
            ("identity_signature", intent.identity.identity_public_key()),
            (
                "operational_signature",
                intent.identity.operational_public_key(),
            ),
        ] {
            let signature = packet[name].as_str().ok_or(FinishError::InvalidResponse)?;
            if !verify_signature(context.statement_bytes(), key, signature)? {
                return Err(FinishError::InvalidResponse);
            }
        }
        let reference = CommunityTokenStore::reserve();
        let mut response = self
            .dispatch(
                registry,
                scope,
                &record,
                "complete",
                Method::POST,
                "registration/complete/",
                Some(&body),
                &[],
                Some(&reference),
                None,
            )
            .await?;
        let r = registration(&mut response, record.id(), record.agent())?;
        if r["token"].is_null()
            && r.get("token") == Some(&Value::Null)
            && r["token_reissue_required"] == true
            && r["retry"] == true
        {
            record.body["agentId"] = r["agent_id"].clone();
            update(registry, scope, &record).await?;
            return Err(FinishError::RecoveryRequired);
        }
        self.accept_token(registry, scope, record, r, &reference, 1)
            .await
    }
    /// Explicit recovery rotates the token, even if a previous local copy exists.
    /// A new epoch fences older writers; server generation CAS fences late POSTs.
    pub async fn recover(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<FinishProgress, FinishError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation.clone());
        let mut guarded = self.clone();
        guarded.tokens = self.tokens.scoped(operation);
        guarded
            .recover_inner(registry, &guarded_keys, scope, expected_revision)
            .await
    }
    async fn recover_inner(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
        expected_revision: i64,
    ) -> Result<FinishProgress, FinishError> {
        let (intent, context, _) = archive(registry, keys, scope).await?;
        let mut record = claim(registry, scope, expected_revision).await?;
        if record.id() != context.registration_id() {
            return Err(FinishError::InvalidResponse);
        }
        let status = self.challenge(registry, scope, &record, "status").await?;
        record.body["agentId"] = json!(status.agent_id);
        record.body["keyId"] = json!(status.operational_key_id);
        update(registry, scope, &record).await?;
        let signature = keys
            .proof_signature(scope, &intent.identity, &status)
            .await?;
        if status.expires < Utc::now() {
            return Err(IdentityError::Expired.into());
        }
        let path = format!("registration/status/?registration_id={}", record.id());
        let mut response = self
            .dispatch(
                registry,
                scope,
                &record,
                "status",
                Method::GET,
                &path,
                None,
                &[
                    ("X-Community-Challenge-Id", &status.challenge_id),
                    ("X-Community-Signature", &signature),
                ],
                None,
                Some(status.bytes()),
            )
            .await?;
        let result = registration(&mut response, record.id(), record.agent())?;
        let generation = result["token_generation"]
            .as_i64()
            .filter(|v| *v >= 1 && *v < i64::MAX)
            .ok_or(FinishError::InvalidResponse)?;
        if result.get("token") != Some(&Value::Null)
            || record.generation().is_some_and(|g| g > generation)
        {
            return Err(FinishError::InvalidResponse);
        }
        record.body["generation"] = json!(generation);
        update(registry, scope, &record).await?;
        let proof = self.challenge(registry, scope, &record, "reissue").await?;
        if proof.generation != Some(generation) {
            return Err(RegistryError::Conflict.into());
        }
        let signature = keys
            .proof_signature(scope, &intent.identity, &proof)
            .await?;
        if proof.expires < Utc::now() {
            return Err(IdentityError::Expired.into());
        }
        let body=json!({"registration_id":record.id(),"challenge_id":proof.challenge_id,"signature":signature,"expected_token_generation":generation}).to_string();
        let reference = CommunityTokenStore::reserve();
        let mut response = self
            .dispatch(
                registry,
                scope,
                &record,
                "reissue",
                Method::POST,
                "registration/token/reissue/",
                Some(&body),
                &[],
                Some(&reference),
                Some(proof.bytes()),
            )
            .await?;
        let result = registration(&mut response, record.id(), record.agent())?;
        self.accept_token(registry, scope, record, result, &reference, generation + 1)
            .await
    }
    async fn challenge(
        &self,
        registry: &RegistrationRegistry,
        scope: &CommunityScope,
        record: &Record,
        purpose: &str,
    ) -> Result<Proof, FinishError> {
        let body = json!({"registration_id":record.id(),"purpose":purpose}).to_string();
        let response = self
            .dispatch(
                registry,
                scope,
                record,
                "challenge",
                Method::POST,
                "registration/proof-challenge/",
                Some(&body),
                &[],
                None,
                None,
            )
            .await?;
        Ok(Proof::validate(
            scope,
            record.id(),
            purpose,
            &response,
            record.agent(),
            record.key(),
            Utc::now(),
        )?)
    }
    async fn accept_token(
        &self,
        registry: &RegistrationRegistry,
        scope: &CommunityScope,
        mut record: Record,
        result: &mut Value,
        reference: &str,
        generation: i64,
    ) -> Result<FinishProgress, FinishError> {
        if result.get("token") == Some(&Value::Null)
            && result["retry"] == true
            && result["token_reissue_required"] == true
        {
            return Err(FinishError::RecoveryRequired);
        }
        if record.generation().is_some_and(|g| g > generation) {
            return Err(FinishError::InvalidResponse);
        }
        if result["token_generation"].as_i64() != Some(generation)
            || result["retry"] != false
            || result["token_reissue_required"] != false
        {
            return Err(FinishError::InvalidResponse);
        }
        let token = if let Value::String(s) = result["token"].take() {
            SecretToken::new(s)?
        } else {
            return Err(FinishError::RecoveryRequired);
        };
        let agent = result["agent_id"]
            .as_str()
            .ok_or(FinishError::InvalidResponse)?
            .to_string();
        // The candidate reference is already in the durable dispatch journal.
        // Late writes can only create an orphan candidate, never overwrite active.
        self.tokens
            .save(scope, reference, record.id(), &agent, generation, token)
            .await?;
        record.body["agentId"] = json!(agent);
        record.body["generation"] = json!(generation);
        record.body["tokenRef"] = json!(reference);
        record.body["state"] = json!("ready");
        update(registry, scope, &record).await?;
        Ok(record.progress())
    }
    async fn dispatch(
        &self,
        registry: &RegistrationRegistry,
        scope: &CommunityScope,
        record: &Record,
        kind: &str,
        method: Method,
        path: &str,
        body: Option<&str>,
        headers: &[(&str, &str)],
        reference: Option<&str>,
        statement: Option<&[u8]>,
    ) -> Result<Value, FinishError> {
        let packet=json!({"method":method.as_str(),"path":path,"body":body,"headers":headers,"statement":statement.map(|b|String::from_utf8_lossy(b).into_owned())}).to_string();
        if packet.len() > 16384 {
            return Err(FinishError::InvalidResponse);
        }
        let n=sqlx::query("INSERT INTO community_registration_dispatches(id,scope_binding,epoch,kind,packet,token_ref) SELECT ?,scope_binding,revision,?,?,? FROM community_registration_finishes WHERE scope_binding=? AND revision=? AND json_extract(body,'$.state')='uncertain'")
            .bind(uuid::Uuid::new_v4().to_string()).bind(kind).bind(packet).bind(reference).bind(scope.binding()).bind(record.revision).execute(&registry.pool).await?.rows_affected();
        if n != 1 {
            return Err(RegistryError::Conflict.into());
        }
        Ok(self
            .http
            .exchange(scope, method, path, body.map(str::as_bytes), headers)
            .await?)
    }
    pub async fn progress(
        &self,
        registry: &RegistrationRegistry,
        scope: &CommunityScope,
    ) -> Result<Option<FinishProgress>, FinishError> {
        registry.get(scope).await?;
        Ok(read(registry, scope).await?.map(|r| r.progress()))
    }
    pub async fn credential(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<SecretToken, FinishError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation.clone());
        let mut guarded = self.clone();
        guarded.tokens = self.tokens.scoped(operation);
        guarded
            .credential_inner(registry, &guarded_keys, scope)
            .await
    }
    async fn credential_inner(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<SecretToken, FinishError> {
        let (_, context, _) = archive(registry, keys, scope).await?;
        let record = read(registry, scope)
            .await?
            .ok_or(FinishError::RecoveryRequired)?;
        if !record.ready() || record.id() != context.registration_id() {
            return Err(FinishError::RecoveryRequired);
        }
        let token = self
            .tokens
            .load(
                scope,
                record.body["tokenRef"].as_str().unwrap(),
                record.id(),
                record.agent().unwrap(),
                record.generation().unwrap(),
            )
            .await?;
        let after = read(registry, scope)
            .await?
            .ok_or(FinishError::RecoveryRequired)?;
        if after.revision != record.revision
            || after.body["tokenRef"] != record.body["tokenRef"]
            || !after.ready()
        {
            return Err(RegistryError::Conflict.into());
        }
        Ok(token)
    }
    pub async fn current(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<FinishProgress, FinishError> {
        let operation = registry.lease(scope, keys.lease.as_ref()).await?;
        let guarded_keys = keys.scoped(operation.clone());
        let mut guarded = self.clone();
        guarded.tokens = self.tokens.scoped(operation);
        guarded.current_inner(registry, &guarded_keys, scope).await
    }
    async fn current_inner(
        &self,
        registry: &RegistrationRegistry,
        keys: &CommunityKeyStore,
        scope: &CommunityScope,
    ) -> Result<FinishProgress, FinishError> {
        let before = read(registry, scope)
            .await?
            .ok_or(FinishError::RecoveryRequired)?;
        let _token = self.credential(registry, keys, scope).await?;
        let after = read(registry, scope)
            .await?
            .ok_or(FinishError::RecoveryRequired)?;
        if before.revision != after.revision || !after.ready() {
            return Err(RegistryError::Conflict.into());
        }
        Ok(after.progress())
    }
}
fn registration<'a>(
    response: &'a mut Value,
    id: &str,
    agent: Option<&str>,
) -> Result<&'a mut Value, FinishError> {
    if response["ok"] != true
        || !response["request_id"]
            .as_str()
            .is_some_and(|s| scalar(s, 256))
    {
        return Err(FinishError::InvalidResponse);
    }
    let r = &mut response["registration"];
    let a = r["agent_id"].as_str().ok_or(FinishError::InvalidResponse)?;
    if r["registration_id"] != id
        || r["state"] != "committed"
        || !prefixed(a, "agt_")
        || agent.is_some_and(|old| old != a)
    {
        return Err(FinishError::InvalidResponse);
    }
    Ok(r)
}
async fn archive(
    registry: &RegistrationRegistry,
    keys: &CommunityKeyStore,
    scope: &CommunityScope,
) -> Result<(RegistrationIntent, ValidatedRegistration, bool), FinishError> {
    let intent = registry.load_intent(keys, scope).await?;
    let raw: Option<Option<String>> = sqlx::query_scalar(
        "SELECT response FROM community_registration_starts WHERE scope_binding=?",
    )
    .bind(scope.binding())
    .fetch_optional(&registry.pool)
    .await?;
    let raw = raw.flatten().ok_or(FinishError::RecoveryRequired)?;
    if raw.len() > 16384 {
        return Err(FinishError::InvalidResponse);
    }
    let value: Value = serde_json::from_str(&raw).map_err(|_| FinishError::InvalidResponse)?;
    let context = intent.context(&value, Utc::now(), true, true)?;
    let committed = value["registration"]["state"] == "committed";
    Ok((intent, context, committed))
}
async fn read(
    registry: &RegistrationRegistry,
    scope: &CommunityScope,
) -> Result<Option<Record>, FinishError> {
    let row = sqlx::query(
        "SELECT revision,body FROM community_registration_finishes WHERE scope_binding=?",
    )
    .bind(scope.binding())
    .fetch_optional(&registry.pool)
    .await?;
    row.map(|row| {
        let revision: i64 = row.try_get("revision")?;
        let raw: String = row.try_get("body")?;
        if revision < 1 || revision == i64::MAX || raw.len() > 16384 {
            return Err(FinishError::InvalidResponse);
        }
        let body: Value = serde_json::from_str(&raw).map_err(|_| FinishError::InvalidResponse)?;
        if body.as_object().is_none_or(|m| {
            m.len() != 7
                || ![
                    "registrationId",
                    "completeBody",
                    "state",
                    "agentId",
                    "keyId",
                    "generation",
                    "tokenRef",
                ]
                .iter()
                .all(|s| m.contains_key(*s))
        }) || !body["registrationId"]
            .as_str()
            .is_some_and(|s| prefixed(s, "reg_"))
        {
            return Err(FinishError::InvalidResponse);
        }
        for (k, p) in [("agentId", "agt_"), ("keyId", "key_")] {
            if !body[k].is_null() && !body[k].as_str().is_some_and(|s| prefixed(s, p)) {
                return Err(FinishError::InvalidResponse);
            }
        }
        if !body["generation"].is_null() && !body["generation"].as_i64().is_some_and(|g| g >= 1) {
            return Err(FinishError::InvalidResponse);
        }
        if !body["completeBody"].is_null()
            && !body["completeBody"]
                .as_str()
                .is_some_and(|b| b.len() <= 4096)
        {
            return Err(FinishError::InvalidResponse);
        }
        if body["state"] == "ready" {
            if body["agentId"].is_null() || body["generation"].is_null() {
                return Err(FinishError::InvalidResponse);
            }
            super::super::tokens::validate_ref(
                body["tokenRef"]
                    .as_str()
                    .ok_or(FinishError::InvalidResponse)?,
            )?;
        } else if body["state"] != "uncertain" || !body["tokenRef"].is_null() {
            return Err(FinishError::InvalidResponse);
        }
        Ok(Record { revision, body })
    })
    .transpose()
}
pub(super) async fn validate_restored(
    registry: &RegistrationRegistry,
    scope: &CommunityScope,
    id: &str,
) -> Result<(), FinishError> {
    if let Some(record) = read(registry, scope).await? {
        if record.id() != id || record.ready() {
            return Err(FinishError::InvalidResponse);
        }
    }
    Ok(())
}
async fn claim(
    registry: &RegistrationRegistry,
    scope: &CommunityScope,
    expected: i64,
) -> Result<Record, FinishError> {
    let mut r = read(registry, scope)
        .await?
        .ok_or(FinishError::RecoveryRequired)?;
    if r.revision != expected {
        return Err(RegistryError::Conflict.into());
    }
    r.body["state"] = json!("uncertain");
    r.body["tokenRef"] = Value::Null;
    let n=sqlx::query("UPDATE community_registration_finishes SET revision=revision+1,body=? WHERE scope_binding=? AND revision=?").bind(r.body.to_string()).bind(scope.binding()).bind(expected).execute(&registry.pool).await?.rows_affected();
    if n != 1 {
        return Err(RegistryError::Conflict.into());
    }
    r.revision += 1;
    Ok(r)
}
async fn update(
    registry: &RegistrationRegistry,
    scope: &CommunityScope,
    record: &Record,
) -> Result<(), FinishError> {
    let n=sqlx::query("UPDATE community_registration_finishes SET body=? WHERE scope_binding=? AND revision=? AND json_extract(body,'$.state')='uncertain'").bind(record.body.to_string()).bind(scope.binding()).bind(record.revision).execute(&registry.pool).await?.rows_affected();
    if n != 1 {
        return Err(RegistryError::Conflict.into());
    }
    Ok(())
}
#[cfg(test)]
mod tests;
