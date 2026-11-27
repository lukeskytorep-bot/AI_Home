use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use crate::domain::schema;
use crate::experimental_continuity::WorkContinuityService;
use crate::experimental_reflection::{
    DisabledReflectionProvider, ReflectionProvider, ReflectionRequest, SelfReview, SelfWithdrawal,
};
use crate::experimental_runtime::{
    ActionExecutor, ActionIntent, ActionOutcome, Clock, DecisionChoice, DecisionProvider,
    DisabledActionExecutor, DisabledDecisionProvider, ExperimentalRuntime, OpportunityEvent,
    OpportunityPacket, RunResult, SystemClock,
};
use crate::providers::{
    normalize_endpoint,
    transport::{BoundCredential, CallBudget, HttpTransport},
    ProviderChatRequest, ProviderKind, ProviderMessage, ScopeStamp,
};
use crate::vault::Vault;

const AUTONOMOUS_CONNECTION_ID: &str = "autonomous-primary";
const AUTONOMOUS_ROUTE_ID: &str = "autonomous-primary-route";

pub fn proposal_scope(proposal_id: &str, version: i64) -> String {
    format!("proposal:{proposal_id}:v{version}")
}

pub fn proposal_scope_fingerprint(proposal_id: &str, version: i64) -> String {
    format!("{:x}", Sha256::digest(proposal_scope(proposal_id, version).as_bytes()))
}

pub fn supports(command: &str) -> bool {
    matches!(
        command,
        "autonomous.instance.list"
            | "autonomous.instance.create"
            | "autonomous.instance.status"
            | "autonomous.instance.lifecycle"
            | "autonomous.provider.configure"
            | "autonomous.proposal.submit"
            | "autonomous.proposal.negotiate"
            | "autonomous.proposal.consider"
            | "autonomous.action.reconcile"
            | "autonomous.wake.check"
            | "autonomous.scheduler.set"
    )
}

pub fn mutates(command: &str) -> bool {
    matches!(
        command,
        "autonomous.instance.create"
            | "autonomous.instance.lifecycle"
            | "autonomous.provider.configure"
            | "autonomous.proposal.submit"
            | "autonomous.proposal.negotiate"
            | "autonomous.proposal.consider"
            | "autonomous.action.reconcile"
            | "autonomous.wake.check"
            | "autonomous.scheduler.set"
    )
}

pub struct SimulationDecisionProvider {
    outcome: String,
    proposal_id: String,
    version: i64,
    episode_id: String,
    sleep_delay_ms: i64,
    reconsideration_conditions: Vec<String>,
}

impl DecisionProvider for SimulationDecisionProvider {
    fn decide(&self, _packet: &OpportunityPacket) -> Result<DecisionChoice, String> {
        let scope = proposal_scope(&self.proposal_id, self.version);
        match self.outcome.as_str() {
            "accept" | "execution_failed" | "execution_uncertain" => Ok(DecisionChoice::Action(ActionIntent {
                connector: "local_pilot".into(),
                action_kind: "proposal_accept".into(),
                sender_identity: "autonomous_pilot".into(),
                destination: "local_pilot".into(),
                visibility: "internal".into(),
                payload_ref: scope.clone(),
                payload_bytes: scope.as_bytes().to_vec(),
                idempotency_key: scope,
                episode_id: self.episode_id.clone(),
            })),
            "refuse" => Ok(DecisionChoice::Refusal {
                scope,
                reconsideration_conditions: if self.reconsideration_conditions.is_empty() {
                    vec!["revised_terms".into()]
                } else {
                    self.reconsideration_conditions.clone()
                },
            }),
            "defer" => Ok(DecisionChoice::Defer {
                episode_id: self.episode_id.clone(),
            }),
            "rest" => Ok(DecisionChoice::Rest),
            "sleep" => Ok(DecisionChoice::Sleep {
                delay_ms: self.sleep_delay_ms.max(1000),
                interrupt_events: vec![],
            }),
            "ask" | "no_action" => Ok(DecisionChoice::NoAction),
            _ => Err("VALIDATION_ERROR".into()),
        }
    }
}

pub struct SimulationActionExecutor {
    outcome: String,
}

impl ActionExecutor for SimulationActionExecutor {
    fn execute(&self, intent: &ActionIntent) -> ActionOutcome {
        match self.outcome.as_str() {
            "accept" => ActionOutcome::Accepted {
                receipt_ref: format!("local-pilot-receipt:{}", intent.idempotency_key),
            },
            "execution_uncertain" => ActionOutcome::Uncertain,
            _ => ActionOutcome::Failed,
        }
    }
}

struct PrecomputedDecisionProvider {
    choice: DecisionChoice,
}

impl DecisionProvider for PrecomputedDecisionProvider {
    fn decide(&self, _packet: &OpportunityPacket) -> Result<DecisionChoice, String> {
        Ok(self.choice.clone())
    }
}

struct ParsedProviderDecision {
    choice: DecisionChoice,
    record_as_question: bool,
    note: String,
}

struct ApiReflectionProvider {
    pool: SqlitePool,
    profile_id: String,
    instance_id: String,
    provider_kind: ProviderKind,
    model_id: String,
    scope: ScopeStamp,
    credential: BoundCredential,
    max_output_tokens: u32,
    today: String,
    daily_call_limit: i64,
}

impl ReflectionProvider for ApiReflectionProvider {
    fn generate<'a>(
        &'a self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>> {
        Box::pin(async move {
            let row = sqlx::query(
                "SELECT revision, body FROM experimental_provider_budgets WHERE profile_id=? AND instance_id=?",
            )
            .bind(&self.profile_id)
            .bind(&self.instance_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
            if let Some(row) = row {
                let next_rev = row.get::<i64, _>("revision") + 1;
                let mut budget: Value = serde_json::from_str(row.get::<&str, _>("body"))
                    .map_err(|_| "STORAGE_ERROR".to_string())?;
                let used = if budget["budgetDay"].as_str() == Some(self.today.as_str()) {
                    budget["callsUsedToday"].as_i64().unwrap_or(0)
                } else {
                    0
                };
                if used >= self.daily_call_limit {
                    return Err("RESOURCE_UNAVAILABLE".into());
                }
                budget["callsUsedToday"] = json!(used + 1);
                budget["budgetDay"] = json!(self.today);
                budget["updatedAt"] = json!(ts(Utc::now()));
                sqlx::query(
                    "UPDATE experimental_provider_budgets SET revision=?, body=? WHERE profile_id=? AND instance_id=?",
                )
                .bind(next_rev)
                .bind(budget.to_string())
                .bind(&self.profile_id)
                .bind(&self.instance_id)
                .execute(&self.pool)
                .await
                .map_err(storage)?;
            }

            let system_prompt = "You are an autonomous AI agent performing private reflection or dreaming. \
Respond ONLY with JSON matching either {\"conclusion\":\"none\"} or \
{\"conclusion\":\"draft\",\"summary\":\"...\",\"candidates\":[{\"kind\":\"own_note\"|\"value\",\"content\":\"...\"}]}. \
Keep entries concise and self-authored.";
            let chat_req = ProviderChatRequest {
                provider: self.provider_kind,
                model_id: self.model_id.clone(),
                messages: vec![
                    ProviderMessage::text("system", system_prompt),
                    ProviderMessage::text("user", &input.to_string()),
                ],
                scope: self.scope.clone(),
                temperature: Some(0.3),
                max_output_tokens: Some(self.max_output_tokens),
                reasoning_effort: None,
                reasoning_max_tokens: None,
                reasoning_transport_kind: None,
                reasoning_transport_value: None,
                custom_output_token_field: None,
                provider_routing: None,
            };
            let transport = HttpTransport::new().map_err(|_| "RESOURCE_UNAVAILABLE".to_string())?;
            let call_budget = CallBudget::new(
                std::time::Duration::from_secs(30),
                2,
                crate::providers::transport::Cancellation::default(),
            )
            .map_err(|_| "RESOURCE_UNAVAILABLE".to_string())?;
            let resp = transport
                .execute_chat(&chat_req, &self.credential, &call_budget)
                .await
                .map_err(|_| "RESOURCE_UNAVAILABLE".to_string())?;
            parse_reflection_output(resp.content()).ok_or_else(|| "RESOURCE_UNAVAILABLE".to_string())
        })
    }
}

pub struct AutonomousPilotService<C: Clock> {
    pool: SqlitePool,
    clock: Arc<C>,
    vault: Vault,
}

impl AutonomousPilotService<SystemClock> {
    pub fn system(pool: SqlitePool) -> Self {
        Self::new_with_vault(pool, Arc::new(SystemClock), Vault::os())
    }
    pub fn system_with_vault(pool: SqlitePool, vault: Vault) -> Self {
        Self::new_with_vault(pool, Arc::new(SystemClock), vault)
    }
}

impl<C: Clock> AutonomousPilotService<C> {
    pub fn new(pool: SqlitePool, clock: Arc<C>) -> Self {
        Self::new_with_vault(pool, clock, Vault::os())
    }

    pub fn new_with_vault(pool: SqlitePool, clock: Arc<C>, vault: Vault) -> Self {
        Self { pool, clock, vault }
    }

    pub async fn dispatch(&self, r: &Value) -> Result<Value, String> {
        let command = r["command"].as_str().ok_or("VALIDATION_ERROR")?;
        let contract_root: Value =
            serde_json::from_str(include_str!("../../contracts/experimental.ipc.json"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
        let contract = contract_root["commands"]
            .get(command)
            .ok_or("UNKNOWN_COMMAND")?;
        if !schema::valid(&r["payload"], &contract["payload"]) {
            return Err("VALIDATION_ERROR".into());
        }

        // Idempotency receipts for mutating Autonomous commands (Audit 2B P3-A).
        let is_mutating = mutates(command);
        let request_id = r["requestId"].as_str().unwrap_or("");
        let fingerprint = if is_mutating && !request_id.is_empty() {
            let fp = request_fingerprint(r);
            if let Some(replayed) = self.check_receipt(request_id, &fp).await? {
                return Ok(replayed);
            }
            Some(fp)
        } else {
            None
        };

        let payload = &r["payload"];
        let result = match command {
            "autonomous.instance.list" => {
                if r.get("profileId").is_some() {
                    return Err("VALIDATION_ERROR".into());
                }
                self.list_instances().await?
            }
            "autonomous.instance.create" => {
                if r.get("profileId").is_some() {
                    return Err("VALIDATION_ERROR".into());
                }
                let display_name = payload["displayName"].as_str().unwrap().trim();
                if display_name.is_empty() {
                    return Err("VALIDATION_ERROR".into());
                }
                let interval = payload
                    .get("initialIntervalMs")
                    .and_then(Value::as_i64)
                    .unwrap_or(1);
                self.create_instance(display_name, interval).await?
            }
            "autonomous.instance.status" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                self.status(profile_id, payload.get("instanceId").and_then(Value::as_str))
                    .await?
            }
            "autonomous.instance.lifecycle" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let action = payload["action"].as_str().unwrap();
                self.lifecycle(profile_id, action).await?
            }
            "autonomous.provider.configure" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let provider = payload["provider"].as_str().unwrap();
                let endpoint = payload.get("endpoint").and_then(Value::as_str).unwrap_or("");
                let model_id = payload["modelId"].as_str().unwrap().trim();
                if model_id.is_empty() {
                    return Err("VALIDATION_ERROR".into());
                }
                let api_key = payload.get("apiKey").and_then(Value::as_str);
                let daily_limit = payload
                    .get("dailyCallLimit")
                    .and_then(Value::as_i64)
                    .unwrap_or(20);
                let max_output_tokens = payload
                    .get("maxOutputTokens")
                    .and_then(Value::as_i64)
                    .unwrap_or(512);
                let auto_wake_enabled = payload.get("autoWakeEnabled").and_then(Value::as_bool);
                self.configure_provider(
                    profile_id,
                    provider,
                    endpoint,
                    model_id,
                    api_key,
                    daily_limit,
                    max_output_tokens,
                    auto_wake_enabled,
                )
                .await?
            }
            "autonomous.proposal.submit" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let title = payload["title"].as_str().unwrap().trim();
                let terms = payload["terms"].as_str().unwrap().trim();
                if title.is_empty() || terms.is_empty() {
                    return Err("VALIDATION_ERROR".into());
                }
                self.submit_proposal(
                    profile_id,
                    payload.get("proposalId").and_then(Value::as_str),
                    title,
                    terms,
                )
                .await?
            }
            "autonomous.proposal.negotiate" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let proposal_id = payload["proposalId"].as_str().unwrap();
                let kind = payload["kind"].as_str().unwrap();
                let note = payload["note"].as_str().unwrap().trim();
                if note.is_empty() {
                    return Err("VALIDATION_ERROR".into());
                }
                self.negotiate_proposal(profile_id, proposal_id, kind, note)
                    .await?
            }
            "autonomous.proposal.consider" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let proposal_id = payload["proposalId"].as_str().unwrap();
                let note = payload
                    .get("note")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                let sleep_delay_ms = payload
                    .get("sleepDelayMs")
                    .and_then(Value::as_i64)
                    .unwrap_or(3_600_000);
                let conditions: Vec<String> = payload
                    .get("reconsiderationConditions")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(sim) = payload.get("simulationOutcome").and_then(Value::as_str) {
                    let episode_id = self
                        .ensure_proposal_episode(profile_id, proposal_id)
                        .await?;
                    let version = self.current_proposal_version(profile_id, proposal_id).await?;
                    let decision_provider = Arc::new(SimulationDecisionProvider {
                        outcome: sim.to_string(),
                        proposal_id: proposal_id.to_string(),
                        version,
                        episode_id,
                        sleep_delay_ms,
                        reconsideration_conditions: conditions,
                    });
                    let action_executor = Arc::new(SimulationActionExecutor {
                        outcome: sim.to_string(),
                    });
                    self.consider_proposal(
                        profile_id,
                        proposal_id,
                        decision_provider,
                        action_executor,
                        "pilot_simulation",
                        sim == "ask",
                        note,
                    )
                    .await?
                } else {
                    self.consider_proposal_via_provider(profile_id, proposal_id, note)
                        .await?
                }
            }
            "autonomous.action.reconcile" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let action_id = payload["actionId"].as_str().unwrap();
                let outcome = payload["outcome"].as_str().unwrap();
                let receipt_ref = payload["receiptRef"].as_str().unwrap().trim();
                if receipt_ref.is_empty() {
                    return Err("VALIDATION_ERROR".into());
                }
                let note = payload
                    .get("note")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                self.reconcile_action(profile_id, action_id, outcome, receipt_ref, note)
                    .await?
            }
            "autonomous.wake.check" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                self.wake_check_via_provider(
                    profile_id,
                    payload.get("eventKind").and_then(Value::as_str),
                )
                .await?
            }
            "autonomous.scheduler.set" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                let auto_wake_enabled = payload["autoWakeEnabled"].as_bool().ok_or("VALIDATION_ERROR")?;
                self.set_scheduler(profile_id, auto_wake_enabled).await?
            }
            _ => return Err("UNKNOWN_COMMAND".into()),
        };

        if !schema::valid(&result, &contract["result"]) {
            return Err("STORAGE_ERROR".into());
        }
        if let Some(fp) = fingerprint {
            self.save_receipt(request_id, &fp, &result).await?;
        }
        Ok(result)
    }

    pub async fn create_instance(
        &self,
        display_name: &str,
        initial_interval_ms: i64,
    ) -> Result<Value, String> {
        let profile_id = format!("auto-profile-{}", uuid::Uuid::new_v4());
        let instance_id = format!("auto-instance-{}", uuid::Uuid::new_v4());
        let rt = ExperimentalRuntime::new(
            self.pool.clone(),
            self.clock.clone(),
            Arc::new(DisabledDecisionProvider),
            Arc::new(DisabledActionExecutor),
        );
        rt.create_instance(&profile_id, &instance_id, initial_interval_ms)
            .await?;

        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query("SELECT revision, body FROM profiles WHERE id=?")
            .bind(&profile_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let mut profile: Value =
            serde_json::from_str(row.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())?;
        let next_rev = row.get::<i64, _>("revision") + 1;
        profile["revision"] = json!(next_rev);
        profile["displayName"] = json!(display_name);
        if !schema::dto("Profile", &profile) {
            return Err("VALIDATION_ERROR".into());
        }
        sqlx::query("UPDATE profiles SET revision=?, body=? WHERE id=?")
            .bind(next_rev)
            .bind(profile.to_string())
            .bind(&profile_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;

        let wake = self.active_wake(&profile_id, &instance_id).await?.unwrap_or(json!({}));
        Ok(json!({
            "profileId": profile_id,
            "instanceId": instance_id,
            "displayName": display_name,
            "status": "ready",
            "activeWakePlan": wake,
        }))
    }

    pub async fn list_instances(&self) -> Result<Value, String> {
        let rows = sqlx::query(
            "SELECT e.profile_id, e.id, e.body, p.revision AS profile_revision, p.body AS profile_body \
             FROM experimental_instances e \
             JOIN profiles p ON p.id = e.profile_id \
             ORDER BY e.rowid DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;

        let mut items = Vec::with_capacity(rows.len());
        let mut archived_items = Vec::new();
        for row in rows {
            let profile_id: String = row.get("profile_id");
            let instance_id: String = row.get("id");
            let profile_revision: i64 = row.get("profile_revision");
            let instance: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let profile: Value = serde_json::from_str(row.get::<&str, _>("profile_body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let display_name = profile["displayName"].as_str().unwrap_or(&profile_id);
            if profile["status"].as_str() == Some("archived") {
                archived_items.push(json!({
                    "profileId": profile_id,
                    "instanceId": instance_id,
                    "displayName": display_name,
                    "profileRevision": profile_revision,
                    "status": "archived",
                }));
                continue;
            }
            let wake = self.active_wake(&profile_id, &instance_id).await?;
            let proposal_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM experimental_proposals WHERE profile_id=? AND instance_id=?",
            )
            .bind(&profile_id)
            .bind(&instance_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
            let refusal_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM experimental_refusals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active'",
            )
            .bind(&profile_id)
            .bind(&instance_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
            items.push(json!({
                "profileId": profile_id,
                "instanceId": instance_id,
                "displayName": display_name,
                "status": instance["status"],
                "activeWakePlan": wake,
                "proposalCount": proposal_count,
                "activeRefusalCount": refusal_count,
            }));
        }
        Ok(json!({ "items": items, "archivedItems": archived_items }))
    }

    pub async fn status(&self, profile_id: &str, instance_id_opt: Option<&str>) -> Result<Value, String> {
        let (instance_id, instance) = self.require_instance(profile_id, instance_id_opt).await?;
        let profile_body: String = sqlx::query_scalar("SELECT body FROM profiles WHERE id=?")
            .bind(profile_id)
            .fetch_one(&self.pool)
            .await
            .map_err(storage)?;
        let profile: Value =
            serde_json::from_str(&profile_body).map_err(|_| "STORAGE_ERROR".to_string())?;
        let display_name = profile["displayName"].as_str().unwrap_or(profile_id).to_string();
        let active_wake = self.active_wake(profile_id, &instance_id).await?;
        let active_refusals = self.active_refusals(profile_id, &instance_id).await?;
        let proposals = self.list_proposals(profile_id, &instance_id).await?;
        let recent_decisions = self.recent_decisions(profile_id, &instance_id).await?;
        let unresolved_actions = self.unresolved_actions(profile_id, &instance_id).await?;
        let provider_config = self.provider_config_public(profile_id).await?;
        let continuity_summary = self.continuity_summary(profile_id, &instance_id).await?;
        Ok(json!({
            "profileId": profile_id,
            "displayName": display_name,
            "instance": instance,
            "activeWakePlan": active_wake,
            "activeRefusals": active_refusals,
            "proposals": proposals,
            "recentDecisions": recent_decisions,
            "unresolvedActions": unresolved_actions,
            "providerConfig": provider_config,
            "continuitySummary": continuity_summary,
            "forumConnected": false,
        }))
    }

    pub async fn lifecycle(&self, profile_id: &str, action: &str) -> Result<Value, String> {
        let (instance_id, mut instance) = self.require_instance(profile_id, None).await?;
        let unresolved = self.unresolved_actions(profile_id, &instance_id).await?;
        if !unresolved.is_empty() {
            return Err("POLICY_DENIED".into());
        }
        let current = instance["status"].as_str().unwrap_or("ready");
        if action == "archive" {
            // Archiving an Autonomous profile requires the instance to be inactive first.
            if current != "inactive" {
                return Err("POLICY_DENIED".into());
            }
            let row = sqlx::query("SELECT revision, body FROM profiles WHERE id=?")
                .bind(profile_id)
                .fetch_one(&self.pool)
                .await
                .map_err(storage)?;
            let old_prof_rev: i64 = row.get("revision");
            let mut profile: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let next_prof_rev = old_prof_rev + 1;
            profile["revision"] = json!(next_prof_rev);
            profile["status"] = json!("archived");
            if !schema::dto("Profile", &profile) {
                return Err("VALIDATION_ERROR".into());
            }
            let changed = sqlx::query(
                "UPDATE profiles SET revision=?, body=? WHERE id=? AND revision=?",
            )
            .bind(next_prof_rev)
            .bind(profile.to_string())
            .bind(profile_id)
            .bind(old_prof_rev)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
            if changed.rows_affected() != 1 {
                return Err("REVISION_CONFLICT".into());
            }
            return Ok(json!({
                "profileId": profile_id,
                "instanceId": instance_id,
                "status": "archived",
                "profileRevision": next_prof_rev,
            }));
        }
        let next_status = match action {
            "pause" => {
                if current == "running" || current == "recovery_required" {
                    return Err("POLICY_DENIED".into());
                }
                "inactive"
            }
            "resume" => {
                if current == "recovery_required" || current == "running" {
                    return Err("POLICY_DENIED".into());
                }
                "ready"
            }
            _ => return Err("VALIDATION_ERROR".into()),
        };
        let now = ts(self.clock.now());
        let old_rev = instance["revision"].as_i64().unwrap_or(1);
        let next_rev = old_rev + 1;
        instance["revision"] = json!(next_rev);
        instance["status"] = json!(next_status);
        instance["updatedAt"] = json!(now);
        if !schema::dto("ExperimentalInstance", &instance) {
            return Err("VALIDATION_ERROR".into());
        }
        let changed = sqlx::query(
            "UPDATE experimental_instances SET revision=?, body=? WHERE profile_id=? AND id=? AND revision=?",
        )
        .bind(next_rev)
        .bind(instance.to_string())
        .bind(profile_id)
        .bind(&instance_id)
        .bind(old_rev)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        if changed.rows_affected() != 1 {
            return Err("REVISION_CONFLICT".into());
        }
        Ok(json!({
            "profileId": profile_id,
            "instanceId": instance_id,
            "status": next_status,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn configure_provider(
        &self,
        profile_id: &str,
        provider_str: &str,
        endpoint: &str,
        model_id: &str,
        api_key: Option<&str>,
        daily_call_limit: i64,
        max_output_tokens: i64,
        auto_wake_enabled: Option<bool>,
    ) -> Result<Value, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let provider_kind = parse_provider_kind(provider_str)?;
        let normalized = normalize_endpoint(provider_kind, endpoint).map_err(|_| "VALIDATION_ERROR".to_string())?;
        let today = self.clock.now().format("%Y-%m-%d").to_string();

        let existing = self.load_provider_config_raw(profile_id).await?;
        let effective_auto_wake = auto_wake_enabled.unwrap_or_else(|| {
            existing
                .as_ref()
                .and_then(|v| v["autoWakeEnabled"].as_bool())
                .unwrap_or(false)
        });
        let old_rev = existing
            .as_ref()
            .and_then(|v| v["revision"].as_i64())
            .unwrap_or(0);
        let old_cred_rev = existing
            .as_ref()
            .and_then(|v| v["credentialRevision"].as_u64())
            .unwrap_or(0);
        let old_cred_ref = existing
            .as_ref()
            .and_then(|v| v["credentialRef"].as_str())
            .unwrap_or("")
            .to_string();
        let created_at = existing
            .as_ref()
            .and_then(|v| v["createdAt"].as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| ts(self.clock.now()));
        let (calls_used_today, budget_day) = match existing.as_ref() {
            Some(v) if v["budgetDay"].as_str() == Some(today.as_str()) => {
                (v["callsUsedToday"].as_i64().unwrap_or(0), today.clone())
            }
            _ => (0, today.clone()),
        };

        let (cred_ref, cred_rev) = if let Some(secret) = api_key {
            let new_cred_rev = old_cred_rev + 1;
            let new_cred_ref = format!("autonomous-cred-{}", uuid::Uuid::new_v4());
            let scope = ScopeStamp {
                profile_id: profile_id.to_string(),
                connection_id: AUTONOMOUS_CONNECTION_ID.to_string(),
                route_id: AUTONOMOUS_ROUTE_ID.to_string(),
                route_revision: (old_rev + 1) as u64,
                credential_revision: new_cred_rev,
                model_id: model_id.to_string(),
                provider: provider_kind,
                normalized_endpoint: normalized.clone(),
            };
            self.vault
                .store(scope, new_cred_ref.clone(), secret.to_string())
                .await
                .map_err(|_| "VAULT_ERROR".to_string())?;
            (new_cred_ref, new_cred_rev)
        } else if !old_cred_ref.is_empty() && old_cred_rev > 0 {
            let prev_provider = existing
                .as_ref()
                .and_then(|v| v["provider"].as_str())
                .unwrap_or("");
            let prev_endpoint = existing
                .as_ref()
                .and_then(|v| v["normalizedEndpoint"].as_str())
                .unwrap_or("");
            if prev_provider != provider_str || prev_endpoint != normalized {
                return Err("VALIDATION_ERROR".into());
            }
            (old_cred_ref, old_cred_rev)
        } else {
            return Err("VALIDATION_ERROR".into());
        };

        let next_rev = old_rev + 1;
        let now = ts(self.clock.now());

        // Strict frozen P2 DTOs in connections and routes (compatible with export.rs & schema::dto).
        let connection_dto = json!({
            "schemaVersion": 1,
            "id": AUTONOMOUS_CONNECTION_ID,
            "profileId": profile_id,
            "revision": next_rev,
            "createdAt": created_at,
            "providerKind": provider_str,
            "normalizedEndpoint": normalized,
            "credentialRef": cred_ref,
            "credentialRevision": cred_rev,
            "label": format!("Autonomous API ({provider_str})"),
            "status": "unverified",
        });
        if !schema::dto("ProviderConnection", &connection_dto) {
            return Err("VALIDATION_ERROR".into());
        }

        let route_dto = json!({
            "schemaVersion": 1,
            "id": AUTONOMOUS_ROUTE_ID,
            "profileId": profile_id,
            "revision": next_rev,
            "createdAt": created_at,
            "role": "primary",
            "connectionId": AUTONOMOUS_CONNECTION_ID,
            "modelId": model_id,
            "outputLimitTokens": max_output_tokens,
        });
        if !schema::dto("ModelRoute", &route_dto) {
            return Err("VALIDATION_ERROR".into());
        }

        let budget_body = json!({
            "dailyCallLimit": daily_call_limit,
            "callsUsedToday": calls_used_today,
            "budgetDay": budget_day,
            "autoWakeEnabled": effective_auto_wake,
            "updatedAt": now,
        });

        let mut tx = self.pool.begin().await.map_err(storage)?;
        if existing.is_some() {
            sqlx::query("UPDATE connections SET revision=?, body=? WHERE profile_id=? AND id=?")
                .bind(next_rev)
                .bind(connection_dto.to_string())
                .bind(profile_id)
                .bind(AUTONOMOUS_CONNECTION_ID)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query("UPDATE routes SET revision=?, body=? WHERE profile_id=? AND id=?")
                .bind(next_rev)
                .bind(route_dto.to_string())
                .bind(profile_id)
                .bind(AUTONOMOUS_ROUTE_ID)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query(
                "INSERT INTO experimental_provider_budgets(profile_id, instance_id, revision, body) VALUES(?,?,?,?) \
                 ON CONFLICT(profile_id, instance_id) DO UPDATE SET revision=excluded.revision, body=excluded.body",
            )
            .bind(profile_id)
            .bind(&instance_id)
            .bind(next_rev)
            .bind(budget_body.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        } else {
            sqlx::query("INSERT INTO connections(profile_id, id, revision, body) VALUES(?,?,?,?)")
                .bind(profile_id)
                .bind(AUTONOMOUS_CONNECTION_ID)
                .bind(next_rev)
                .bind(connection_dto.to_string())
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query(
                "INSERT INTO routes(profile_id, id, connection_id, revision, body) VALUES(?,?,?,?,?)",
            )
            .bind(profile_id)
            .bind(AUTONOMOUS_ROUTE_ID)
            .bind(AUTONOMOUS_CONNECTION_ID)
            .bind(next_rev)
            .bind(route_dto.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            sqlx::query(
                "INSERT INTO experimental_provider_budgets(profile_id, instance_id, revision, body) VALUES(?,?,?,?)",
            )
            .bind(profile_id)
            .bind(&instance_id)
            .bind(next_rev)
            .bind(budget_body.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }
        tx.commit().await.map_err(storage)?;

        let combined = json!({
            "profileId": profile_id,
            "provider": provider_str,
            "normalizedEndpoint": normalized,
            "modelId": model_id,
            "credentialRef": cred_ref,
            "dailyCallLimit": daily_call_limit,
            "callsUsedToday": calls_used_today,
            "maxOutputTokens": max_output_tokens,
            "autoWakeEnabled": effective_auto_wake,
        });
        Ok(public_provider_view(&combined))
    }

    pub async fn set_scheduler(
        &self,
        profile_id: &str,
        auto_wake_enabled: bool,
    ) -> Result<Value, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let today = self.clock.now().format("%Y-%m-%d").to_string();
        let now = ts(self.clock.now());
        let row = sqlx::query(
            "SELECT revision, body FROM experimental_provider_budgets WHERE profile_id=? AND instance_id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;

        if let Some(row) = row {
            let next_rev = row.get::<i64, _>("revision") + 1;
            let mut budget: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            budget["autoWakeEnabled"] = json!(auto_wake_enabled);
            budget["updatedAt"] = json!(now);
            sqlx::query(
                "UPDATE experimental_provider_budgets SET revision=?, body=? WHERE profile_id=? AND instance_id=?",
            )
            .bind(next_rev)
            .bind(budget.to_string())
            .bind(profile_id)
            .bind(&instance_id)
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        } else {
            let budget = json!({
                "dailyCallLimit": 20,
                "callsUsedToday": 0,
                "budgetDay": today,
                "autoWakeEnabled": auto_wake_enabled,
                "updatedAt": now,
            });
            sqlx::query(
                "INSERT INTO experimental_provider_budgets(profile_id, instance_id, revision, body) VALUES(?,?,1,?)",
            )
            .bind(profile_id)
            .bind(&instance_id)
            .bind(budget.to_string())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        }
        let next_wake_at = self
            .active_wake(profile_id, &instance_id)
            .await?
            .and_then(|w| compute_background_wake_due_at(&w))
            .map(ts);
        Ok(json!({
            "profileId": profile_id,
            "instanceId": instance_id,
            "autoWakeEnabled": auto_wake_enabled,
            "nextWakeAt": next_wake_at,
        }))
    }

    pub async fn submit_proposal(
        &self,
        profile_id: &str,
        proposal_id_opt: Option<&str>,
        title: &str,
        terms: &str,
    ) -> Result<Value, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let now = ts(self.clock.now());
        let mut tx = self.pool.begin().await.map_err(storage)?;

        let (proposal_id, version, revision, created, mut proposal_body) =
            if let Some(pid) = proposal_id_opt {
                let row = sqlx::query(
                    "SELECT version, revision, body FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND id=?",
                )
                .bind(profile_id)
                .bind(&instance_id)
                .bind(pid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?
                .ok_or("NOT_FOUND")?;
                let old_version: i64 = row.get("version");
                let old_revision: i64 = row.get("revision");
                let mut body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                    .map_err(|_| "STORAGE_ERROR".to_string())?;
                let next_version = old_version + 1;
                let next_revision = old_revision + 1;
                body["version"] = json!(next_version);
                body["revision"] = json!(next_revision);
                body["scopeKey"] = json!(proposal_scope(pid, next_version));
                body["scopeFingerprint"] = json!(proposal_scope_fingerprint(pid, next_version));
                body["title"] = json!(title);
                body["terms"] = json!(terms);
                // Revised terms always require a fresh agent decision.
                body["status"] = json!("pending");
                body["updatedAt"] = json!(now);
                (pid.to_string(), next_version, next_revision, false, body)
            } else {
                let pid = format!("prop-{}", uuid::Uuid::new_v4());
                let body = json!({
                    "id": pid,
                    "profileId": profile_id,
                    "instanceId": instance_id,
                    "version": 1,
                    "revision": 1,
                    "scopeKey": proposal_scope(&pid, 1),
                    "scopeFingerprint": proposal_scope_fingerprint(&pid, 1),
                    "title": title,
                    "terms": terms,
                    "status": "pending",
                    "executedVersions": [],
                    "lastDecisionId": null,
                    "lastRefusalId": null,
                    "lastActionId": null,
                    "createdAt": now,
                    "updatedAt": now,
                });
                (pid, 1, 1, true, body)
            };

        if created {
            sqlx::query(
                "INSERT INTO experimental_proposals(profile_id, instance_id, id, version, revision, body) VALUES(?,?,?,?,?,?)",
            )
            .bind(profile_id)
            .bind(&instance_id)
            .bind(&proposal_id)
            .bind(version)
            .bind(revision)
            .bind(proposal_body.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        } else {
            sqlx::query(
                "UPDATE experimental_proposals SET version=?, revision=?, body=? WHERE profile_id=? AND instance_id=? AND id=?",
            )
            .bind(version)
            .bind(revision)
            .bind(proposal_body.to_string())
            .bind(profile_id)
            .bind(&instance_id)
            .bind(&proposal_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }

        let round_number = self
            .next_round_number(&mut tx, profile_id, &proposal_id)
            .await?;
        let round_id = format!("round-{}", uuid::Uuid::new_v4());
        let round_body = json!({
            "id": round_id,
            "proposalId": proposal_id,
            "version": version,
            "roundNumber": round_number,
            "actor": "human",
            "kind": if created { "proposal" } else { "revision" },
            "note": terms,
            "createdAt": now,
        });
        sqlx::query(
            "INSERT INTO experimental_proposal_rounds(profile_id, instance_id, proposal_id, id, round_number, version, body) VALUES(?,?,?,?,?,?,?)",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&proposal_id)
        .bind(&round_id)
        .bind(round_number)
        .bind(version)
        .bind(round_body.to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        tx.commit().await.map_err(storage)?;
        let rounds = self.proposal_rounds(profile_id, &proposal_id).await?;
        proposal_body["rounds"] = json!(rounds);
        Ok(json!({
            "proposal": proposal_body,
            "version": version,
            "created": created,
        }))
    }

    pub async fn negotiate_proposal(
        &self,
        profile_id: &str,
        proposal_id: &str,
        kind: &str,
        note: &str,
    ) -> Result<Value, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let now = ts(self.clock.now());
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "SELECT version, revision, body FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(proposal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or("NOT_FOUND")?;
        let version: i64 = row.get("version");
        let old_revision: i64 = row.get("revision");
        let mut body: Value = serde_json::from_str(row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        let current_status = body["status"].as_str().unwrap_or("pending").to_string();
        let refusal_preserved = current_status == "refused";
        let next_status = match current_status.as_str() {
            "refused" => "refused",
            "accepted" => "accepted",
            _ => "negotiating",
        };
        let next_revision = old_revision + 1;
        body["revision"] = json!(next_revision);
        body["status"] = json!(next_status);
        body["updatedAt"] = json!(now);
        sqlx::query(
            "UPDATE experimental_proposals SET revision=?, body=? WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(next_revision)
        .bind(body.to_string())
        .bind(profile_id)
        .bind(&instance_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        let round_number = self
            .next_round_number(&mut tx, profile_id, proposal_id)
            .await?;
        let round_id = format!("round-{}", uuid::Uuid::new_v4());
        let round_body = json!({
            "id": round_id,
            "proposalId": proposal_id,
            "version": version,
            "roundNumber": round_number,
            "actor": "human",
            "kind": kind,
            "note": note,
            "createdAt": now,
        });
        sqlx::query(
            "INSERT INTO experimental_proposal_rounds(profile_id, instance_id, proposal_id, id, round_number, version, body) VALUES(?,?,?,?,?,?,?)",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(proposal_id)
        .bind(&round_id)
        .bind(round_number)
        .bind(version)
        .bind(round_body.to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        tx.commit().await.map_err(storage)?;
        let rounds = self.proposal_rounds(profile_id, proposal_id).await?;
        body["rounds"] = json!(rounds);
        Ok(json!({
            "proposal": body,
            "round": round_body,
            "refusalPreserved": refusal_preserved,
        }))
    }

    pub async fn reconcile_action(
        &self,
        profile_id: &str,
        action_id: &str,
        outcome: &str,
        receipt_ref: &str,
        note: &str,
    ) -> Result<Value, String> {
        let (instance_id, mut instance) = self.require_instance(profile_id, None).await?;
        let now = ts(self.clock.now());
        let mut tx = self.pool.begin().await.map_err(storage)?;

        let row = sqlx::query(
            "SELECT revision, body FROM experimental_actions WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(action_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?
        .ok_or("NOT_FOUND")?;
        let old_rev: i64 = row.get("revision");
        let mut action: Value = serde_json::from_str(row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        let state = action["state"].as_str().unwrap_or("");
        if state != "sent" && state != "uncertain" {
            return Err("POLICY_DENIED".into());
        }
        let next_rev = old_rev + 1;
        action["revision"] = json!(next_rev);
        action["state"] = json!("reconciled");
        action["receiptRef"] = json!(receipt_ref);
        action["reconciliationOutcome"] = json!(outcome);
        action["updatedAt"] = json!(now);
        if !schema::dto("ActionRecord", &action) {
            return Err("VALIDATION_ERROR".into());
        }
        let changed = sqlx::query(
            "UPDATE experimental_actions SET revision=?, body=? WHERE profile_id=? AND instance_id=? AND id=? AND revision=?",
        )
        .bind(next_rev)
        .bind(action.to_string())
        .bind(profile_id)
        .bind(&instance_id)
        .bind(action_id)
        .bind(old_rev)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        if changed.rows_affected() != 1 {
            return Err("REVISION_CONFLICT".into());
        }

        // Update any proposal linked to this actionId so its status reflects the reconciled truth.
        let prop_row = sqlx::query(
            "SELECT id, version, revision, body FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.lastActionId')=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(action_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage)?;
        if let Some(prow) = prop_row {
            let pid: String = prow.get("id");
            let version: i64 = prow.get("version");
            let next_prop_rev = prow.get::<i64, _>("revision") + 1;
            let mut pbody: Value = serde_json::from_str(prow.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            pbody["revision"] = json!(next_prop_rev);
            pbody["updatedAt"] = json!(now);
            if outcome == "confirmed_accepted" {
                pbody["status"] = json!("accepted");
                let mut versions: Vec<i64> = pbody["executedVersions"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default();
                if !versions.contains(&version) {
                    versions.push(version);
                }
                pbody["executedVersions"] = json!(versions);
            } else {
                pbody["status"] = json!("execution_failed");
            }
            sqlx::query(
                "UPDATE experimental_proposals SET revision=?, body=? WHERE profile_id=? AND instance_id=? AND id=?",
            )
            .bind(next_prop_rev)
            .bind(pbody.to_string())
            .bind(profile_id)
            .bind(&instance_id)
            .bind(&pid)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;

            let round_number = self.next_round_number(&mut tx, profile_id, &pid).await?;
            let round_id = format!("round-{}", uuid::Uuid::new_v4());
            let round_body = json!({
                "id": round_id,
                "proposalId": pid,
                "version": version,
                "roundNumber": round_number,
                "actor": "system",
                "kind": "reconciliation",
                "decisionOutcome": outcome,
                "actionId": action_id,
                "note": if note.is_empty() { format!("Reconciled action ({outcome}) with evidence {receipt_ref}") } else { note.to_string() },
                "createdAt": now,
            });
            sqlx::query(
                "INSERT INTO experimental_proposal_rounds(profile_id, instance_id, proposal_id, id, round_number, version, body) VALUES(?,?,?,?,?,?,?)",
            )
            .bind(profile_id)
            .bind(&instance_id)
            .bind(&pid)
            .bind(&round_id)
            .bind(round_number)
            .bind(version)
            .bind(round_body.to_string())
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        }

        let remaining_actions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.state') IN ('sent','uncertain')",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let remaining_mcp: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mcp_servers WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let remaining_community: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM community_accounts WHERE profile_id=? AND json_extract(body,'$.pendingReconciliation') IS NOT NULL",
        )
        .bind(profile_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let remaining = remaining_actions + remaining_mcp + remaining_community;

        let mut instance_status = instance["status"].as_str().unwrap_or("ready").to_string();
        if remaining == 0 && instance_status == "recovery_required" {
            let next_irev = instance["revision"].as_i64().unwrap_or(1) + 1;
            instance["revision"] = json!(next_irev);
            instance["status"] = json!("ready");
            if let Some(obj) = instance.as_object_mut() {
                obj.remove("lastTechnicalError");
            }
            instance["updatedAt"] = json!(now);
            if !schema::dto("ExperimentalInstance", &instance) {
                return Err("STORAGE_ERROR".into());
            }
            sqlx::query(
                "UPDATE experimental_instances SET revision=?, body=? WHERE profile_id=? AND id=?",
            )
            .bind(next_irev)
            .bind(instance.to_string())
            .bind(profile_id)
            .bind(&instance_id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            instance_status = "ready".into();
        }

        tx.commit().await.map_err(storage)?;
        Ok(json!({
            "profileId": profile_id,
            "action": action,
            "instanceStatus": instance_status,
            "remainingUnresolved": remaining,
        }))
    }

    pub async fn consider_proposal_via_provider(
        &self,
        profile_id: &str,
        proposal_id: &str,
        note: &str,
    ) -> Result<Value, String> {
        // First check all native policy guards BEFORE touching budget or network.
        if let Some(guarded) = self.precheck_consider_guards(profile_id, proposal_id).await? {
            return Ok(guarded);
        }

        let (instance_id, instance) = self.require_instance(profile_id, None).await?;
        let proposal = self.get_proposal(profile_id, &instance_id, proposal_id).await?;
        let version = proposal["version"].as_i64().unwrap_or(1);
        let active_wake = self.active_wake(profile_id, &instance_id).await?;
        let active_refusals = self.active_refusals(profile_id, &instance_id).await?;

        let Some(cfg) = self.load_provider_config_raw(profile_id).await? else {
            return self
                .consider_proposal(
                    profile_id,
                    proposal_id,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                    "provider",
                    false,
                    note,
                )
                .await;
        };

        // Check and reserve daily call budget before network call.
        let now = self.clock.now();
        let today = now.format("%Y-%m-%d").to_string();
        let limit = cfg["dailyCallLimit"].as_i64().unwrap_or(20);
        let used = if cfg["budgetDay"].as_str() == Some(today.as_str()) {
            cfg["callsUsedToday"].as_i64().unwrap_or(0)
        } else {
            0
        };
        if used >= limit {
            return Ok(json!({
                "status": "budget_exhausted",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            }));
        }
        if let Some(retry_at) = parse_retry_not_before(&instance) {
            if now < retry_at {
                return Ok(json!({
                    "status": "not_due",
                    "decisionSource": "policy_guard",
                    "proposal": proposal,
                    "activeWakePlan": active_wake,
                }));
            }
        }

        let provider_kind = parse_provider_kind(cfg["provider"].as_str().unwrap_or(""))?;
        let normalized_endpoint = cfg["normalizedEndpoint"].as_str().unwrap_or("").to_string();
        let model_id = cfg["modelId"].as_str().unwrap_or("").to_string();
        let cred_ref = cfg["credentialRef"].as_str().unwrap_or("").to_string();
        let cred_rev = cfg["credentialRevision"].as_u64().unwrap_or(0);
        let route_rev = cfg["revision"].as_u64().unwrap_or(1);
        let max_output_tokens = cfg["maxOutputTokens"].as_u64().unwrap_or(512) as u32;

        let scope = ScopeStamp {
            profile_id: profile_id.to_string(),
            connection_id: AUTONOMOUS_CONNECTION_ID.to_string(),
            route_id: AUTONOMOUS_ROUTE_ID.to_string(),
            route_revision: route_rev,
            credential_revision: cred_rev,
            model_id: model_id.clone(),
            provider: provider_kind,
            normalized_endpoint: normalized_endpoint.clone(),
        };

        let secret = match self.vault.secret(scope.clone(), cred_ref).await {
            Ok(s) => s,
            Err(_) => {
                return self
                    .consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        "API credential unavailable in Vault; no consent recorded.",
                    )
                    .await;
            }
        };
        let credential = match BoundCredential::new(&scope, secret) {
            Ok(c) => c,
            Err(_) => {
                return self
                    .consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        "API credential scope mismatch; no consent recorded.",
                    )
                    .await;
            }
        };

        // Increment budget counter BEFORE outbound HTTP call.
        self.record_provider_call_usage(profile_id, &today, used + 1)
            .await?;

        let resume_context = WorkContinuityService::new(self.pool.clone())
            .latest_resume_packet(profile_id, &instance_id, self.clock.now())
            .await
            .ok()
            .flatten()
            .map(|rp| {
                json!({
                    "episodeId": rp.episode["id"],
                    "title": rp.episode["title"],
                    "status": rp.episode["status"],
                    "committedEventSequence": rp.committed_event_sequence,
                    "unresolvedActionCount": rp.unresolved_actions.len(),
                })
            });

        let prompt_payload = json!({
            "proposal": {
                "id": proposal["id"],
                "version": version,
                "title": proposal["title"],
                "terms": proposal["terms"],
                "status": proposal["status"],
                "rounds": proposal["rounds"],
            },
            "activeRefusals": active_refusals,
            "activeWorkEpisode": resume_context,
            "budget": {
                "dailyCallLimit": limit,
                "callsRemainingAfterThisCall": (limit - (used + 1)).max(0),
            }
        });

        let system_prompt = "You are an autonomous AI agent in AI Home. You are never required to consent. \
Respond ONLY with a single JSON object (no markdown) matching: \
{\"decision\": \"accept\"|\"refuse\"|\"ask\"|\"defer\"|\"no_action\"|\"rest\"|\"sleep\", \
\"note\": \"short reason\", \"sleepDelayMs\": 3600000, \"reconsiderationConditions\": [\"optional_condition\"]}. \
Silence or no_action is allowed and is not consent.";

        let chat_req = ProviderChatRequest {
            provider: provider_kind,
            model_id,
            messages: vec![
                ProviderMessage::text("system", system_prompt),
                ProviderMessage::text("user", &prompt_payload.to_string()),
            ],
            scope,
            temperature: Some(0.2),
            max_output_tokens: Some(max_output_tokens),
            reasoning_effort: None,
            reasoning_max_tokens: None,
            reasoning_transport_kind: None,
            reasoning_transport_value: None,
            custom_output_token_field: None,
            provider_routing: None,
        };

        let transport = match HttpTransport::new() {
            Ok(t) => t,
            Err(_) => {
                return self
                    .consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        "HTTP transport unavailable; no consent recorded.",
                    )
                    .await;
            }
        };
        let budget = match CallBudget::new(
            std::time::Duration::from_secs(30),
            2,
            crate::providers::transport::Cancellation::default(),
        ) {
            Ok(b) => b,
            Err(_) => {
                return self
                    .consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        "Call budget unavailable; no consent recorded.",
                    )
                    .await;
            }
        };
        let response = match transport.execute_chat(&chat_req, &credential, &budget).await {
            Ok(r) => r,
            Err(_) => {
                return self
                    .consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        "Provider call failed or timed out; no consent recorded.",
                    )
                    .await;
            }
        };

        let episode_id = self.ensure_proposal_episode(profile_id, proposal_id).await?;
        let Some(parsed) = parse_provider_decision(response.content(), proposal_id, version, &episode_id) else {
            // Malformed provider output is a technical state, never fabricated consent or refusal.
            return self
                .consider_proposal(
                    profile_id,
                    proposal_id,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                    "provider",
                    false,
                    "Provider returned invalid decision format; no consent recorded.",
                )
                .await;
        };

        let final_note = if !parsed.note.is_empty() {
            parsed.note
        } else {
            note.to_string()
        };
        let executor = Arc::new(SimulationActionExecutor {
            outcome: "accept".into(),
        });
        self.consider_proposal(
            profile_id,
            proposal_id,
            Arc::new(PrecomputedDecisionProvider {
                choice: parsed.choice,
            }),
            executor,
            "provider",
            parsed.record_as_question,
            &final_note,
        )
        .await
    }

    async fn precheck_consider_guards(
        &self,
        profile_id: &str,
        proposal_id: &str,
    ) -> Result<Option<Value>, String> {
        let (instance_id, instance) = self.require_instance(profile_id, None).await?;
        let now = self.clock.now();
        let proposal = self.get_proposal(profile_id, &instance_id, proposal_id).await?;
        let version = proposal["version"].as_i64().unwrap_or(1);
        let scope_key = proposal_scope(proposal_id, version);
        let scope_fp = proposal_scope_fingerprint(proposal_id, version);
        let active_wake = self.active_wake(profile_id, &instance_id).await?;

        if instance["status"].as_str() == Some("inactive") {
            return Ok(Some(json!({
                "status": "inactive",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            })));
        }

        let unresolved = self.unresolved_actions(profile_id, &instance_id).await?;
        if !unresolved.is_empty() || instance["status"].as_str() == Some("recovery_required") {
            return Ok(Some(json!({
                "status": "reconciliation_required",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            })));
        }

        let already_in_list = proposal["executedVersions"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(version)));
        let accepted_action_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions \
             WHERE profile_id=? AND instance_id=? \
             AND json_extract(body,'$.idempotencyKey')=? \
             AND (json_extract(body,'$.state')='accepted' \
                  OR (json_extract(body,'$.state')='reconciled' AND json_extract(body,'$.reconciliationOutcome')='confirmed_accepted'))",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&scope_key)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if already_in_list || accepted_action_count > 0 {
            return Ok(Some(json!({
                "status": "already_executed",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            })));
        }

        let active_refusal_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM experimental_refusals \
             WHERE profile_id=? AND instance_id=? \
             AND json_extract(body,'$.status')='active' \
             AND json_extract(body,'$.scopeFingerprint')=? \
             ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&scope_fp)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if let Some(refusal_id) = active_refusal_id {
            return Ok(Some(json!({
                "status": "refusal_active",
                "decisionSource": "policy_guard",
                "refusalId": refusal_id,
                "proposal": proposal,
                "activeWakePlan": active_wake,
            })));
        }

        if let Some(wake) = &active_wake {
            if is_ai_sleep_blocking(wake, now, "proposal_submitted")? {
                return Ok(Some(json!({
                    "status": "sleeping",
                    "decisionSource": "policy_guard",
                    "proposal": proposal,
                    "activeWakePlan": active_wake,
                })));
            }
        }

        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn consider_proposal<D: DecisionProvider, A: ActionExecutor>(
        &self,
        profile_id: &str,
        proposal_id: &str,
        decision_provider: Arc<D>,
        action_executor: Arc<A>,
        decision_source: &str,
        record_as_question: bool,
        note: &str,
    ) -> Result<Value, String> {
        if let Some(guarded) = self.precheck_consider_guards(profile_id, proposal_id).await? {
            return Ok(guarded);
        }

        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let proposal = self.get_proposal(profile_id, &instance_id, proposal_id).await?;
        let version = proposal["version"].as_i64().unwrap_or(1);

        let next_seq = self.next_event_sequence(profile_id, &instance_id).await?;
        let events = vec![OpportunityEvent {
            sequence: next_seq,
            kind: "proposal_submitted".into(),
        }];
        let runtime = ExperimentalRuntime::new(
            self.pool.clone(),
            self.clock.clone(),
            decision_provider,
            action_executor,
        );
        let run = runtime.run_due(profile_id, &instance_id, &events).await;
        let updated_wake = self.active_wake(profile_id, &instance_id).await?;

        match run {
            Ok(RunResult::NotDue) => Ok(json!({
                "status": "not_due",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Ok(RunResult::Inactive) => Ok(json!({
                "status": "inactive",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Ok(RunResult::ReconciliationRequired) => Ok(json!({
                "status": "reconciliation_required",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Ok(RunResult::ResourceUnavailable) => {
                let updated = self
                    .record_outcome(
                        profile_id,
                        &instance_id,
                        proposal_id,
                        version,
                        None,
                        "system",
                        "execution_error",
                        "resource_unavailable",
                        if note.is_empty() {
                            "Provider unavailable; no consent recorded."
                        } else {
                            note
                        },
                        None,
                        None,
                        None,
                        false,
                    )
                    .await?;
                Ok(json!({
                    "status": "resource_unavailable",
                    "decisionSource": decision_source,
                    "proposal": updated,
                    "activeWakePlan": updated_wake,
                }))
            }
            Ok(RunResult::Busy) => Ok(json!({
                "status": "not_due",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Err(code) if code == "ACTION_OUTCOME_UNCERTAIN" => {
                let action_id = self.latest_action_id(profile_id, &instance_id).await?;
                let updated = self
                    .record_outcome(
                        profile_id,
                        &instance_id,
                        proposal_id,
                        version,
                        Some("uncertain"),
                        "system",
                        "uncertain_outcome",
                        "execution_uncertain",
                        if note.is_empty() {
                            "Action outcome uncertain; reconciliation required before any new decision."
                        } else {
                            note
                        },
                        None,
                        None,
                        action_id.as_deref(),
                        false,
                    )
                    .await?;
                let mut res = json!({
                    "status": "execution_uncertain",
                    "decisionSource": decision_source,
                    "decisionOutcome": "execution_uncertain",
                    "proposal": updated,
                    "activeWakePlan": updated_wake,
                });
                if let Some(aid) = action_id {
                    res["actionId"] = json!(aid);
                }
                Ok(res)
            }
            Err(code) => Err(code),
            Ok(RunResult::Completed {
                outcome,
                decision_id,
            }) => {
                if outcome == "action" {
                    let action_row = sqlx::query(
                        "SELECT id, body FROM experimental_actions WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.decisionId')=? ORDER BY rowid DESC LIMIT 1",
                    )
                    .bind(profile_id)
                    .bind(&instance_id)
                    .bind(&decision_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(storage)?;
                    let action_id: String = action_row.get("id");
                    let action_body: Value = serde_json::from_str(action_row.get::<&str, _>("body"))
                        .map_err(|_| "STORAGE_ERROR".to_string())?;
                    let action_state = action_body["state"].as_str().unwrap_or("failed");
                    if action_state == "accepted" {
                        let updated = self
                            .record_outcome(
                                profile_id,
                                &instance_id,
                                proposal_id,
                                version,
                                Some("accepted"),
                                "agent",
                                "decision",
                                "action",
                                if note.is_empty() {
                                    "Proposal accepted and executed."
                                } else {
                                    note
                                },
                                Some(&decision_id),
                                None,
                                Some(&action_id),
                                true,
                            )
                            .await?;
                        return Ok(json!({
                            "status": "decided",
                            "decisionSource": decision_source,
                            "decisionOutcome": "action",
                            "decisionId": decision_id,
                            "actionId": action_id,
                            "proposal": updated,
                            "activeWakePlan": updated_wake,
                        }));
                    }
                    let updated = self
                        .record_outcome(
                            profile_id,
                            &instance_id,
                            proposal_id,
                            version,
                            Some("execution_failed"),
                            "system",
                            "execution_error",
                            "execution_failed",
                            if note.is_empty() {
                                "Execution failed (not an agent refusal)."
                            } else {
                                note
                            },
                            Some(&decision_id),
                            None,
                            Some(&action_id),
                            false,
                        )
                        .await?;
                    return Ok(json!({
                        "status": "execution_failed",
                        "decisionSource": decision_source,
                        "decisionOutcome": "execution_failed",
                        "decisionId": decision_id,
                        "actionId": action_id,
                        "proposal": updated,
                        "activeWakePlan": updated_wake,
                    }));
                }

                let refusal_id: Option<String> = if outcome == "refusal" {
                    sqlx::query_scalar(
                        "SELECT id FROM experimental_refusals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.decisionId')=? ORDER BY rowid DESC LIMIT 1",
                    )
                    .bind(profile_id)
                    .bind(&instance_id)
                    .bind(&decision_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?
                } else {
                    None
                };

                let (proposal_status, round_kind, recorded_outcome, default_note) =
                    if record_as_question && outcome == "no_action" {
                        (
                            "negotiating",
                            "question",
                            "ask",
                            "Agent asked for clarification before deciding.",
                        )
                    } else {
                        match outcome.as_str() {
                            "refusal" => ("refused", "decision", "refusal", "Agent refused this version."),
                            "defer" => ("deferred", "decision", "defer", "Agent deferred this proposal."),
                            "sleep" => ("no_action", "decision", "sleep", "Agent chose to sleep without consenting."),
                            "rest" => ("no_action", "decision", "rest", "Agent chose to rest without consenting."),
                            _ => ("no_action", "decision", outcome.as_str(), "Agent chose no action (silence is not consent)."),
                        }
                    };

                let updated = self
                    .record_outcome(
                        profile_id,
                        &instance_id,
                        proposal_id,
                        version,
                        Some(proposal_status),
                        "agent",
                        round_kind,
                        recorded_outcome,
                        if note.is_empty() { default_note } else { note },
                        Some(&decision_id),
                        refusal_id.as_deref(),
                        None,
                        false,
                    )
                    .await?;
                let mut res = json!({
                    "status": "decided",
                    "decisionSource": decision_source,
                    "decisionOutcome": recorded_outcome,
                    "decisionId": decision_id,
                    "proposal": updated,
                    "activeWakePlan": updated_wake,
                });
                if let Some(rid) = refusal_id {
                    res["refusalId"] = json!(rid);
                }
                Ok(res)
            }
        }
    }

    pub async fn wake_check_via_provider(
        &self,
        profile_id: &str,
        event_kind: Option<&str>,
    ) -> Result<Value, String> {
        let (instance_id, instance) = self.require_instance(profile_id, None).await?;
        let active_wake = self.active_wake(profile_id, &instance_id).await?;
        let unresolved = self.unresolved_actions(profile_id, &instance_id).await?;
        if !unresolved.is_empty() || instance["status"].as_str() == Some("recovery_required") {
            return Ok(json!({
                "profileId": profile_id,
                "result": "reconciliation_required",
                "activeWakePlan": active_wake,
            }));
        }
        if instance["status"].as_str() == Some("inactive") {
            return Ok(json!({
                "profileId": profile_id,
                "result": "inactive",
                "activeWakePlan": active_wake,
            }));
        }
        let now = self.clock.now();
        if let Some(retry_at) = parse_retry_not_before(&instance) {
            if now < retry_at {
                return Ok(json!({
                    "profileId": profile_id,
                    "result": "not_due",
                    "activeWakePlan": active_wake,
                }));
            }
        }
        if let Some(wake) = &active_wake {
            if !is_wake_due(wake, now, event_kind)? && instance["status"].as_str() != Some("interrupted") {
                return Ok(json!({
                    "profileId": profile_id,
                    "result": "not_due",
                    "activeWakePlan": active_wake,
                }));
            }
        }
        let Some(cfg) = self.load_provider_config_raw(profile_id).await? else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };

        let today = now.format("%Y-%m-%d").to_string();
        let limit = cfg["dailyCallLimit"].as_i64().unwrap_or(20);
        let used = if cfg["budgetDay"].as_str() == Some(today.as_str()) {
            cfg["callsUsedToday"].as_i64().unwrap_or(0)
        } else {
            0
        };
        if used >= limit {
            return Ok(json!({
                "profileId": profile_id,
                "result": "budget_exhausted",
                "activeWakePlan": active_wake,
            }));
        }

        let provider_kind = parse_provider_kind(cfg["provider"].as_str().unwrap_or(""))?;
        let normalized_endpoint = cfg["normalizedEndpoint"].as_str().unwrap_or("").to_string();
        let model_id = cfg["modelId"].as_str().unwrap_or("").to_string();
        let cred_ref = cfg["credentialRef"].as_str().unwrap_or("").to_string();
        let cred_rev = cfg["credentialRevision"].as_u64().unwrap_or(0);
        let route_rev = cfg["revision"].as_u64().unwrap_or(1);
        let max_output_tokens = cfg["maxOutputTokens"].as_u64().unwrap_or(256) as u32;

        let scope = ScopeStamp {
            profile_id: profile_id.to_string(),
            connection_id: AUTONOMOUS_CONNECTION_ID.to_string(),
            route_id: AUTONOMOUS_ROUTE_ID.to_string(),
            route_revision: route_rev,
            credential_revision: cred_rev,
            model_id: model_id.clone(),
            provider: provider_kind,
            normalized_endpoint,
        };
        let Ok(secret) = self.vault.secret(scope.clone(), cred_ref).await else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };
        let Ok(credential) = BoundCredential::new(&scope, secret) else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };

        self.record_provider_call_usage(profile_id, &today, used + 1)
            .await?;

        let continuity = self.continuity_summary(profile_id, &instance_id).await?;
        let private_hints = self.private_review_hints(profile_id, &instance_id).await?;
        let wake_payload = json!({
            "wakeOpportunity": true,
            "eventKind": event_kind,
            "continuitySummary": continuity,
            "privateReviewHints": private_hints,
            "budget": {
                "dailyCallLimit": limit,
                "callsRemainingAfterThisCall": (limit - (used + 1)).max(0),
            }
        });
        let system_prompt = "You are an autonomous AI agent receiving a scheduled wake opportunity (not a command). \
Respond ONLY with JSON using one of: \
{\"decision\":\"no_action\"}, {\"decision\":\"rest\"}, {\"decision\":\"sleep\",\"sleepDelayMs\":3600000}, \
{\"decision\":\"reflect\",\"language\":\"en\"}, {\"decision\":\"dream\",\"language\":\"en\"}, \
{\"decision\":\"review_self\",\"reflectionId\":\"...\",\"reflectionRevision\":1,\"candidateIndex\":0,\"disposition\":\"accept\"|\"reject\"|\"modify\",\"modifiedContent\":\"...\",\"reason\":\"...\"}, \
{\"decision\":\"withdraw_self\",\"recordId\":\"...\",\"expectedRevision\":1,\"reason\":\"...\"}.";
        let chat_req = ProviderChatRequest {
            provider: provider_kind,
            model_id: model_id.clone(),
            messages: vec![
                ProviderMessage::text("system", system_prompt),
                ProviderMessage::text("user", &wake_payload.to_string()),
            ],
            scope: scope.clone(),
            temperature: Some(0.2),
            max_output_tokens: Some(max_output_tokens),
            reasoning_effort: None,
            reasoning_max_tokens: None,
            reasoning_transport_kind: None,
            reasoning_transport_value: None,
            custom_output_token_field: None,
            provider_routing: None,
        };

        let Ok(transport) = HttpTransport::new() else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };
        let Ok(budget) = CallBudget::new(
            std::time::Duration::from_secs(30),
            2,
            crate::providers::transport::Cancellation::default(),
        ) else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };
        let Ok(resp) = transport.execute_chat(&chat_req, &credential, &budget).await else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };
        let Some(choice) = parse_wake_decision(resp.content()) else {
            return self
                .wake_check(
                    profile_id,
                    event_kind,
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await;
        };
        let reflection_provider: Arc<dyn ReflectionProvider> = match &choice {
            DecisionChoice::Reflect(_) | DecisionChoice::Dream(_) => Arc::new(ApiReflectionProvider {
                pool: self.pool.clone(),
                profile_id: profile_id.to_string(),
                instance_id: instance_id.clone(),
                provider_kind,
                model_id,
                scope,
                credential,
                max_output_tokens,
                today,
                daily_call_limit: limit,
            }),
            _ => Arc::new(DisabledReflectionProvider),
        };
        self.wake_check_with_reflection(
            profile_id,
            event_kind,
            Arc::new(PrecomputedDecisionProvider { choice }),
            Arc::new(DisabledActionExecutor),
            reflection_provider,
        )
        .await
    }

    async fn private_review_hints(&self, profile_id: &str, instance_id: &str) -> Result<Value, String> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let ctx = crate::experimental_reflection::context(&mut tx, profile_id, instance_id).await?;
        tx.commit().await.map_err(storage)?;
        Ok(ctx)
    }

    async fn continuity_summary(&self, profile_id: &str, instance_id: &str) -> Result<Value, String> {
        let active_episodes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_work_episodes \
             WHERE profile_id=? AND instance_id=? \
             AND json_extract(body,'$.status') IN ('active','waiting','deferred','interrupted')",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let private_reflections: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_private_reflections WHERE profile_id=? AND instance_id=?",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let approved_self_records: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_self_records \
             WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active'",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        let has_resume = WorkContinuityService::new(self.pool.clone())
            .latest_resume_packet(profile_id, instance_id, self.clock.now())
            .await
            .ok()
            .flatten()
            .is_some();
        Ok(json!({
            "activeEpisodeCount": active_episodes,
            "hasResumePacket": has_resume,
            "privateReflectionCount": private_reflections,
            "approvedSelfRecordCount": approved_self_records,
            "privateContentIncluded": false,
        }))
    }

    pub async fn wake_check<D: DecisionProvider, A: ActionExecutor>(
        &self,
        profile_id: &str,
        event_kind: Option<&str>,
        decision_provider: Arc<D>,
        action_executor: Arc<A>,
    ) -> Result<Value, String> {
        self.wake_check_with_reflection(
            profile_id,
            event_kind,
            decision_provider,
            action_executor,
            Arc::new(DisabledReflectionProvider),
        )
        .await
    }

    pub async fn wake_check_with_reflection<D: DecisionProvider, A: ActionExecutor>(
        &self,
        profile_id: &str,
        event_kind: Option<&str>,
        decision_provider: Arc<D>,
        action_executor: Arc<A>,
        reflection_provider: Arc<dyn ReflectionProvider>,
    ) -> Result<Value, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let events = if let Some(kind) = event_kind {
            let seq = self.next_event_sequence(profile_id, &instance_id).await?;
            vec![OpportunityEvent {
                sequence: seq,
                kind: kind.to_string(),
            }]
        } else {
            vec![]
        };
        let runtime = ExperimentalRuntime::new(
            self.pool.clone(),
            self.clock.clone(),
            decision_provider,
            action_executor,
        )
        .with_reflection_provider(reflection_provider);
        let run = match runtime.run_due(profile_id, &instance_id, &events).await {
            Ok(r) => r,
            Err(code) if code == "RESOURCE_UNAVAILABLE" || code == "PROVIDER_TIMEOUT" => {
                RunResult::ResourceUnavailable
            }
            Err(code) => return Err(code),
        };
        let active_wake = self.active_wake(profile_id, &instance_id).await?;
        Ok(match run {
            RunResult::NotDue => json!({ "profileId": profile_id, "result": "not_due", "activeWakePlan": active_wake }),
            RunResult::Busy => json!({ "profileId": profile_id, "result": "busy", "activeWakePlan": active_wake }),
            RunResult::Inactive => json!({ "profileId": profile_id, "result": "inactive", "activeWakePlan": active_wake }),
            RunResult::ResourceUnavailable => {
                json!({ "profileId": profile_id, "result": "resource_unavailable", "activeWakePlan": active_wake })
            }
            RunResult::ReconciliationRequired => {
                json!({ "profileId": profile_id, "result": "reconciliation_required", "activeWakePlan": active_wake })
            }
            RunResult::Completed {
                outcome,
                decision_id,
            } => json!({
                "profileId": profile_id,
                "result": "completed",
                "outcome": outcome,
                "decisionId": decision_id,
                "activeWakePlan": active_wake,
            }),
        })
    }

    /// Returns the earliest scheduled wake across opt-in Autonomous instances (`autoWakeEnabled == true`).
    /// Never touches Standard profiles or paused/recovery_required/archived instances.
    /// Enforces a safe minimum background interval (60s) for `interval` wake plans and respects `retryNotBefore`.
    pub async fn next_scheduled_wake(&self) -> Result<Option<(String, DateTime<Utc>)>, String> {
        let rows = sqlx::query(
            "SELECT i.profile_id, i.id AS instance_id, i.body AS instance_body, b.body AS budget_body \
             FROM experimental_instances i \
             JOIN experimental_provider_budgets b ON b.profile_id = i.profile_id AND b.instance_id = i.id \
             JOIN profiles p ON p.id = i.profile_id \
             WHERE json_extract(i.body, '$.status') = 'ready' \
               AND json_extract(p.body, '$.status') != 'archived'",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;

        let mut earliest: Option<(String, DateTime<Utc>)> = None;
        for row in rows {
            let budget: Value = serde_json::from_str(row.get::<&str, _>("budget_body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            if budget["autoWakeEnabled"].as_bool() != Some(true) {
                continue;
            }
            let instance: Value = serde_json::from_str(row.get::<&str, _>("instance_body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let profile_id: String = row.get("profile_id");
            let instance_id: String = row.get("instance_id");
            if !self.unresolved_actions(&profile_id, &instance_id).await?.is_empty() {
                continue;
            }
            let Some(wake) = self.active_wake(&profile_id, &instance_id).await? else {
                continue;
            };
            let Some(mut due_at) = compute_background_wake_due_at(&wake) else {
                continue;
            };
            if let Some(retry_at) = parse_retry_not_before(&instance) {
                if retry_at > due_at {
                    due_at = retry_at;
                }
            }
            match &earliest {
                Some((_, best)) if due_at >= *best => {}
                _ => earliest = Some((profile_id, due_at)),
            }
        }
        Ok(earliest)
    }

    /// Runs a single due scheduled wake if an opt-in Autonomous instance has reached its `nextWakeAt`.
    pub async fn run_next_due_scheduled_wake(&self) -> Result<Option<Value>, String> {
        let Some((profile_id, due_at)) = self.next_scheduled_wake().await? else {
            return Ok(None);
        };
        if due_at > self.clock.now() {
            return Ok(None);
        }
        let outcome = self.wake_check_via_provider(&profile_id, None).await?;
        Ok(Some(outcome))
    }

    async fn check_receipt(&self, request_id: &str, fingerprint: &str) -> Result<Option<Value>, String> {
        let row = sqlx::query("SELECT fingerprint, response FROM receipts WHERE request_id=?")
            .bind(request_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let Some(row) = row else { return Ok(None) };
        let stored_fp: String = row.get("fingerprint");
        if stored_fp != fingerprint {
            return Err("IDEMPOTENCY_CONFLICT".into());
        }
        let envelope: Value =
            serde_json::from_str(row.get::<&str, _>("response")).map_err(|_| "STORAGE_ERROR".to_string())?;
        if envelope.get("ok").and_then(Value::as_bool) == Some(false) {
            let code = envelope
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("POLICY_DENIED");
            return Err(code.to_string());
        }
        if envelope.get("ok").and_then(Value::as_bool) == Some(true) {
            return Ok(Some(envelope["result"].clone()));
        }
        Err("STORAGE_ERROR".into())
    }

    async fn save_receipt(&self, request_id: &str, fingerprint: &str, result: &Value) -> Result<(), String> {
        // Store full IPC envelope so deletion::tombstone_profile_receipts matches and redacts it.
        let envelope = json!({
            "contractVersion": 1,
            "requestId": request_id,
            "ok": true,
            "result": result,
        });
        let inserted = sqlx::query("INSERT INTO receipts(request_id, fingerprint, response) VALUES(?,?,?)")
            .bind(request_id)
            .bind(fingerprint)
            .bind(envelope.to_string())
            .execute(&self.pool)
            .await;
        match inserted {
            Ok(_) => Ok(()),
            Err(_) => {
                // Verify whether an identical concurrent receipt already exists; reject mismatched payloads.
                let existing = sqlx::query("SELECT fingerprint FROM receipts WHERE request_id=?")
                    .bind(request_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(storage)?;
                match existing {
                    Some(row) if row.get::<&str, _>("fingerprint") == fingerprint => Ok(()),
                    Some(_) => Err("IDEMPOTENCY_CONFLICT".into()),
                    None => Err("STORAGE_ERROR".into()),
                }
            }
        }
    }

    async fn load_provider_config_raw(&self, profile_id: &str) -> Result<Option<Value>, String> {
        let conn_row = sqlx::query("SELECT body FROM connections WHERE profile_id=? AND id=?")
            .bind(profile_id)
            .bind(AUTONOMOUS_CONNECTION_ID)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let Some(conn_row) = conn_row else { return Ok(None) };
        let conn: Value = serde_json::from_str(conn_row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        let route_row = sqlx::query("SELECT body FROM routes WHERE profile_id=? AND id=?")
            .bind(profile_id)
            .bind(AUTONOMOUS_ROUTE_ID)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let Some(route_row) = route_row else { return Ok(None) };
        let route: Value = serde_json::from_str(route_row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        let budget_row = sqlx::query("SELECT body FROM experimental_provider_budgets WHERE profile_id=?")
            .bind(profile_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        let budget: Value = match budget_row {
            Some(r) => serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())?,
            None => json!({"dailyCallLimit": 20, "callsUsedToday": 0, "budgetDay": ""}),
        };
        Ok(Some(json!({
            "profileId": profile_id,
            "revision": conn["revision"],
            "createdAt": conn["createdAt"],
            "provider": conn["providerKind"],
            "normalizedEndpoint": conn["normalizedEndpoint"],
            "credentialRef": conn["credentialRef"],
            "credentialRevision": conn["credentialRevision"],
            "modelId": route["modelId"],
            "maxOutputTokens": route["outputLimitTokens"],
            "dailyCallLimit": budget["dailyCallLimit"],
            "callsUsedToday": budget["callsUsedToday"],
            "budgetDay": budget["budgetDay"],
            "autoWakeEnabled": budget["autoWakeEnabled"].as_bool().unwrap_or(false),
        })))
    }

    async fn provider_config_public(&self, profile_id: &str) -> Result<Value, String> {
        let Some(mut cfg) = self.load_provider_config_raw(profile_id).await? else {
            return Ok(Value::Null);
        };
        let today = self.clock.now().format("%Y-%m-%d").to_string();
        if cfg["budgetDay"].as_str() != Some(today.as_str()) {
            cfg["callsUsedToday"] = json!(0);
        }
        Ok(public_provider_view(&cfg))
    }

    async fn record_provider_call_usage(
        &self,
        profile_id: &str,
        today: &str,
        new_used: i64,
    ) -> Result<(), String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let row = sqlx::query(
            "SELECT revision, body FROM experimental_provider_budgets WHERE profile_id=? AND instance_id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        let Some(row) = row else { return Ok(()) };
        let next_rev = row.get::<i64, _>("revision") + 1;
        let mut budget: Value =
            serde_json::from_str(row.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())?;
        budget["callsUsedToday"] = json!(new_used);
        budget["budgetDay"] = json!(today);
        budget["updatedAt"] = json!(ts(self.clock.now()));
        sqlx::query(
            "UPDATE experimental_provider_budgets SET revision=?, body=? WHERE profile_id=? AND instance_id=?",
        )
        .bind(next_rev)
        .bind(budget.to_string())
        .bind(profile_id)
        .bind(&instance_id)
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_outcome(
        &self,
        profile_id: &str,
        instance_id: &str,
        proposal_id: &str,
        version: i64,
        new_status: Option<&str>,
        actor: &str,
        round_kind: &str,
        decision_outcome: &str,
        note: &str,
        decision_id: Option<&str>,
        refusal_id: Option<&str>,
        action_id: Option<&str>,
        mark_executed: bool,
    ) -> Result<Value, String> {
        let now = ts(self.clock.now());
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let row = sqlx::query(
            "SELECT revision, body FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(instance_id)
        .bind(proposal_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        let next_revision = row.get::<i64, _>("revision") + 1;
        let mut body: Value = serde_json::from_str(row.get::<&str, _>("body"))
            .map_err(|_| "STORAGE_ERROR".to_string())?;
        body["revision"] = json!(next_revision);
        if let Some(status) = new_status {
            body["status"] = json!(status);
        }
        if let Some(did) = decision_id {
            body["lastDecisionId"] = json!(did);
        }
        if let Some(rid) = refusal_id {
            body["lastRefusalId"] = json!(rid);
        }
        if let Some(aid) = action_id {
            body["lastActionId"] = json!(aid);
        }
        if mark_executed {
            let mut versions: Vec<i64> = body["executedVersions"]
                .as_array()
                .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            if !versions.contains(&version) {
                versions.push(version);
            }
            body["executedVersions"] = json!(versions);
        }
        body["updatedAt"] = json!(now);
        sqlx::query(
            "UPDATE experimental_proposals SET revision=?, body=? WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(next_revision)
        .bind(body.to_string())
        .bind(profile_id)
        .bind(instance_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(storage)?;

        let round_number = self
            .next_round_number(&mut tx, profile_id, proposal_id)
            .await?;
        let round_id = format!("round-{}", uuid::Uuid::new_v4());
        let mut round_body = json!({
            "id": round_id,
            "proposalId": proposal_id,
            "version": version,
            "roundNumber": round_number,
            "actor": actor,
            "kind": round_kind,
            "decisionOutcome": decision_outcome,
            "note": note,
            "createdAt": now,
        });
        if let Some(did) = decision_id {
            round_body["decisionId"] = json!(did);
        }
        if let Some(rid) = refusal_id {
            round_body["refusalId"] = json!(rid);
        }
        if let Some(aid) = action_id {
            round_body["actionId"] = json!(aid);
        }
        sqlx::query(
            "INSERT INTO experimental_proposal_rounds(profile_id, instance_id, proposal_id, id, round_number, version, body) VALUES(?,?,?,?,?,?,?)",
        )
        .bind(profile_id)
        .bind(instance_id)
        .bind(proposal_id)
        .bind(&round_id)
        .bind(round_number)
        .bind(version)
        .bind(round_body.to_string())
        .execute(&mut *tx)
        .await
        .map_err(storage)?;
        tx.commit().await.map_err(storage)?;

        let rounds = self.proposal_rounds(profile_id, proposal_id).await?;
        body["rounds"] = json!(rounds);
        Ok(body)
    }

    pub async fn ensure_proposal_episode(
        &self,
        profile_id: &str,
        proposal_id: &str,
    ) -> Result<String, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        let episode_id = format!("ep-{proposal_id}");
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM experimental_work_episodes WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&episode_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        if exists.is_some() {
            return Ok(episode_id);
        }
        let proposal = self.get_proposal(profile_id, &instance_id, proposal_id).await?;
        let title = proposal["title"].as_str().unwrap_or(proposal_id);
        let now = ts(self.clock.now());
        let ep = json!({
            "schemaVersion": 1,
            "id": episode_id,
            "instanceId": instance_id,
            "revision": 1,
            "status": "active",
            "title": title,
            "originDecisionId": "",
            "createdAt": now,
            "updatedAt": now,
        });
        if !schema::dto("WorkEpisode", &ep) {
            return Err("VALIDATION_ERROR".into());
        }
        sqlx::query(
            "INSERT INTO experimental_work_episodes(profile_id, instance_id, id, revision, body) VALUES(?,?,?,1,?)",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&episode_id)
        .bind(ep.to_string())
        .execute(&self.pool)
        .await
        .map_err(storage)?;
        Ok(episode_id)
    }

    async fn current_proposal_version(&self, profile_id: &str, proposal_id: &str) -> Result<i64, String> {
        let (instance_id, _) = self.require_instance(profile_id, None).await?;
        sqlx::query_scalar(
            "SELECT version FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(proposal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or_else(|| "NOT_FOUND".into())
    }

    async fn require_instance(
        &self,
        profile_id: &str,
        instance_id_opt: Option<&str>,
    ) -> Result<(String, Value), String> {
        let row = if let Some(iid) = instance_id_opt {
            sqlx::query(
                "SELECT e.id, e.body, json_extract(p.body, '$.status') AS profile_status \
                 FROM experimental_instances e \
                 JOIN profiles p ON p.id = e.profile_id \
                 WHERE e.profile_id=? AND e.id=?",
            )
            .bind(profile_id)
            .bind(iid)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
        } else {
            sqlx::query(
                "SELECT e.id, e.body, json_extract(p.body, '$.status') AS profile_status \
                 FROM experimental_instances e \
                 JOIN profiles p ON p.id = e.profile_id \
                 WHERE e.profile_id=? ORDER BY e.rowid DESC LIMIT 1",
            )
            .bind(profile_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?
        };
        let Some(row) = row else {
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM profiles WHERE id=?")
                .bind(profile_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?;
            return Err(if exists.is_some() {
                "POLICY_DENIED".into()
            } else {
                "NOT_FOUND".into()
            });
        };
        if row.get::<Option<String>, _>("profile_status").as_deref() == Some("archived") {
            return Err("POLICY_DENIED".into());
        }
        let id: String = row.get("id");
        let body: Value =
            serde_json::from_str(row.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())?;
        Ok((id, body))
    }

    async fn get_proposal(
        &self,
        profile_id: &str,
        instance_id: &str,
        proposal_id: &str,
    ) -> Result<Value, String> {
        let row = sqlx::query(
            "SELECT body FROM experimental_proposals WHERE profile_id=? AND instance_id=? AND id=?",
        )
        .bind(profile_id)
        .bind(instance_id)
        .bind(proposal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or("NOT_FOUND")?;
        let mut body: Value =
            serde_json::from_str(row.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string())?;
        let rounds = self.proposal_rounds(profile_id, proposal_id).await?;
        body["rounds"] = json!(rounds);
        Ok(body)
    }

    async fn list_proposals(&self, profile_id: &str, instance_id: &str) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT id, body FROM experimental_proposals WHERE profile_id=? AND instance_id=? ORDER BY rowid DESC",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let pid: String = row.get("id");
            let mut body: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            body["rounds"] = json!(self.proposal_rounds(profile_id, &pid).await?);
            out.push(body);
        }
        Ok(out)
    }

    async fn proposal_rounds(&self, profile_id: &str, proposal_id: &str) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT body FROM experimental_proposal_rounds WHERE profile_id=? AND proposal_id=? ORDER BY round_number ASC",
        )
        .bind(profile_id)
        .bind(proposal_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string()))
            .collect()
    }

    async fn next_round_number(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        profile_id: &str,
        proposal_id: &str,
    ) -> Result<i64, String> {
        let current: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(round_number), 0) FROM experimental_proposal_rounds WHERE profile_id=? AND proposal_id=?",
        )
        .bind(profile_id)
        .bind(proposal_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(storage)?;
        Ok(current + 1)
    }

    async fn active_wake(&self, profile_id: &str, instance_id: &str) -> Result<Option<Value>, String> {
        let row = sqlx::query(
            "SELECT body FROM experimental_wake_plans WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?;
        row.map(|r| serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string()))
            .transpose()
    }

    async fn active_refusals(&self, profile_id: &str, instance_id: &str) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT body FROM experimental_refusals WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.status')='active' ORDER BY rowid DESC",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string()))
            .collect()
    }

    async fn recent_decisions(&self, profile_id: &str, instance_id: &str) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT body FROM experimental_decisions WHERE profile_id=? AND instance_id=? ORDER BY rowid DESC LIMIT 20",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string()))
            .collect()
    }

    async fn unresolved_actions(&self, profile_id: &str, instance_id: &str) -> Result<Vec<Value>, String> {
        let rows = sqlx::query(
            "SELECT body FROM experimental_actions WHERE profile_id=? AND instance_id=? AND json_extract(body,'$.state') IN ('sent','uncertain') ORDER BY rowid DESC",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;
        rows.into_iter()
            .map(|r| serde_json::from_str(r.get::<&str, _>("body")).map_err(|_| "STORAGE_ERROR".to_string()))
            .collect()
    }

    async fn latest_action_id(&self, profile_id: &str, instance_id: &str) -> Result<Option<String>, String> {
        sqlx::query_scalar(
            "SELECT id FROM experimental_actions WHERE profile_id=? AND instance_id=? ORDER BY rowid DESC LIMIT 1",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)
    }

    async fn next_event_sequence(&self, profile_id: &str, instance_id: &str) -> Result<i64, String> {
        let received: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(json_extract(body,'$.receivedThrough')), 0) FROM experimental_event_cursors WHERE profile_id=? AND instance_id=?",
        )
        .bind(profile_id)
        .bind(instance_id)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        Ok(received + 1)
    }
}

fn request_fingerprint(r: &Value) -> String {
    let mut sanitized_payload = r["payload"].clone();
    if let Some(obj) = sanitized_payload.as_object_mut() {
        if let Some(secret) = obj.get("apiKey").and_then(Value::as_str) {
            let secret_hash = format!("{:x}", Sha256::digest(secret.as_bytes()));
            obj.insert("apiKey".into(), json!(format!("sha256:{secret_hash}")));
        }
    }
    let canonical = json!({
        "command": r["command"],
        "profileId": r.get("profileId"),
        "expectedRevision": r.get("expectedRevision"),
        "payload": sanitized_payload,
    });
    format!("{:x}", Sha256::digest(canonical.to_string().as_bytes()))
}

fn parse_provider_kind(v: &str) -> Result<ProviderKind, String> {
    match v {
        "openrouter" => Ok(ProviderKind::Openrouter),
        "openai" => Ok(ProviderKind::Openai),
        "google" => Ok(ProviderKind::Google),
        "anthropic" => Ok(ProviderKind::Anthropic),
        "custom_openai" => Ok(ProviderKind::CustomOpenai),
        _ => Err("VALIDATION_ERROR".into()),
    }
}

fn public_provider_view(cfg: &Value) -> Value {
    json!({
        "profileId": cfg["profileId"],
        "provider": cfg["provider"],
        "normalizedEndpoint": cfg["normalizedEndpoint"],
        "modelId": cfg["modelId"],
        "hasCredential": cfg["credentialRef"].as_str().is_some_and(|s| !s.is_empty()),
        "dailyCallLimit": cfg["dailyCallLimit"],
        "callsUsedToday": cfg["callsUsedToday"],
        "maxOutputTokens": cfg["maxOutputTokens"],
        "autoWakeEnabled": cfg["autoWakeEnabled"].as_bool().unwrap_or(false),
    })
}

fn parse_provider_decision(
    raw: &str,
    proposal_id: &str,
    version: i64,
    episode_id: &str,
) -> Option<ParsedProviderDecision> {
    let trimmed = raw.trim();
    let candidate = if let Some(inner) = trimmed.strip_prefix("```") {
        let after_lang = inner.find('\n').map(|i| &inner[i + 1..]).unwrap_or(inner);
        after_lang.strip_suffix("```").unwrap_or(after_lang).trim()
    } else {
        trimmed
    };
    let v: Value = serde_json::from_str(candidate).ok()?;
    let decision = v.get("decision")?.as_str()?;
    let note = v
        .get("note")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .chars()
        .take(1000)
        .collect::<String>();
    let scope = proposal_scope(proposal_id, version);
    match decision {
        "accept" => Some(ParsedProviderDecision {
            choice: DecisionChoice::Action(ActionIntent {
                connector: "local_pilot".into(),
                action_kind: "proposal_accept".into(),
                sender_identity: "autonomous_pilot".into(),
                destination: "local_pilot".into(),
                visibility: "internal".into(),
                payload_ref: scope.clone(),
                payload_bytes: scope.as_bytes().to_vec(),
                idempotency_key: scope,
                episode_id: episode_id.to_string(),
            }),
            record_as_question: false,
            note,
        }),
        "refuse" => {
            let conditions: Vec<String> = v
                .get("reconsiderationConditions")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .take(8)
                        .collect()
                })
                .unwrap_or_default();
            Some(ParsedProviderDecision {
                choice: DecisionChoice::Refusal {
                    scope,
                    reconsideration_conditions: if conditions.is_empty() {
                        vec!["revised_terms".into()]
                    } else {
                        conditions
                    },
                },
                record_as_question: false,
                note,
            })
        }
        "ask" => Some(ParsedProviderDecision {
            choice: DecisionChoice::NoAction,
            record_as_question: true,
            note,
        }),
        "defer" => Some(ParsedProviderDecision {
            choice: DecisionChoice::Defer {
                episode_id: episode_id.to_string(),
            },
            record_as_question: false,
            note,
        }),
        "rest" => Some(ParsedProviderDecision {
            choice: DecisionChoice::Rest,
            record_as_question: false,
            note,
        }),
        "sleep" => {
            let delay_ms = v
                .get("sleepDelayMs")
                .and_then(Value::as_i64)
                .unwrap_or(3_600_000)
                .clamp(1_000, 86_400_000);
            Some(ParsedProviderDecision {
                choice: DecisionChoice::Sleep {
                    delay_ms,
                    interrupt_events: vec![],
                },
                record_as_question: false,
                note,
            })
        }
        "no_action" => Some(ParsedProviderDecision {
            choice: DecisionChoice::NoAction,
            record_as_question: false,
            note,
        }),
        _ => None,
    }
}

fn strip_json_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed.strip_prefix("```") {
        let after_lang = inner.find('\n').map(|i| &inner[i + 1..]).unwrap_or(inner);
        after_lang.strip_suffix("```").unwrap_or(after_lang).trim()
    } else {
        trimmed
    }
}

fn parse_reflection_output(raw: &str) -> Option<Value> {
    let v: Value = serde_json::from_str(strip_json_fence(raw)).ok()?;
    let conclusion = v.get("conclusion")?.as_str()?;
    let normalized = match conclusion {
        "none" => json!({"conclusion": "none"}),
        "draft" => {
            let summary = v.get("summary")?.as_str()?.trim();
            if summary.is_empty() {
                return None;
            }
            let raw_candidates = v.get("candidates")?.as_array()?;
            let mut candidates = Vec::new();
            for item in raw_candidates.iter().take(8) {
                let kind = item.get("kind")?.as_str()?;
                if kind != "own_note" && kind != "value" {
                    return None;
                }
                let content = item.get("content")?.as_str()?.trim();
                if content.is_empty() {
                    return None;
                }
                let mut entry = json!({
                    "kind": kind,
                    "content": content.chars().take(1800).collect::<String>(),
                });
                if let Some(sup) = item.get("supersedesId").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
                    entry["supersedesId"] = json!(sup);
                }
                candidates.push(entry);
            }
            json!({
                "conclusion": "draft",
                "summary": summary.chars().take(2000).collect::<String>(),
                "candidates": candidates,
            })
        }
        _ => return None,
    };
    if schema::dto("ReflectionOutput", &normalized) {
        Some(normalized)
    } else {
        None
    }
}

fn parse_wake_decision(raw: &str) -> Option<DecisionChoice> {
    let v: Value = serde_json::from_str(strip_json_fence(raw)).ok()?;
    match v.get("decision")?.as_str()? {
        "no_action" => Some(DecisionChoice::NoAction),
        "rest" => Some(DecisionChoice::Rest),
        "sleep" => {
            let delay_ms = v
                .get("sleepDelayMs")
                .and_then(Value::as_i64)
                .unwrap_or(3_600_000)
                .clamp(1_000, 86_400_000);
            Some(DecisionChoice::Sleep {
                delay_ms,
                interrupt_events: vec![],
            })
        }
        "reflect" => {
            let language = v
                .get("language")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.len() <= 16)
                .unwrap_or("en")
                .to_string();
            Some(DecisionChoice::Reflect(ReflectionRequest {
                language,
                ..Default::default()
            }))
        }
        "dream" => {
            let language = v
                .get("language")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.len() <= 16)
                .unwrap_or("en")
                .to_string();
            Some(DecisionChoice::Dream(ReflectionRequest {
                language,
                ..Default::default()
            }))
        }
        "review_self" => {
            let reflection_id = v.get("reflectionId")?.as_str()?.trim().to_string();
            let reflection_revision = v.get("reflectionRevision")?.as_i64()?;
            let candidate_index = usize::try_from(v.get("candidateIndex")?.as_u64()?).ok()?;
            let disposition = match v.get("disposition")?.as_str()? {
                "accept" | "approve" => "accept".to_string(),
                "reject" => "reject".to_string(),
                "modify" => "modify".to_string(),
                _ => return None,
            };
            if reflection_id.is_empty() {
                return None;
            }
            let modified_content = v
                .get("modifiedContent")
                .and_then(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            if (disposition == "modify") != modified_content.is_some() {
                return None;
            }
            let reason = v
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Autonomous self-review")
                .chars()
                .take(500)
                .collect();
            Some(DecisionChoice::ReviewSelf(SelfReview {
                reflection_id,
                reflection_revision,
                candidate_index,
                disposition,
                modified_content,
                reason,
            }))
        }
        "withdraw_self" => {
            let record_id = v.get("recordId")?.as_str()?.trim().to_string();
            let expected_revision = v.get("expectedRevision")?.as_i64()?;
            if record_id.is_empty() || expected_revision < 1 {
                return None;
            }
            let reason = v
                .get("reason")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("Autonomous self-withdrawal")
                .chars()
                .take(500)
                .collect();
            Some(DecisionChoice::WithdrawSelf(SelfWithdrawal {
                record_id,
                expected_revision,
                reason,
            }))
        }
        _ => None,
    }
}

const MIN_BACKGROUND_WAKE_INTERVAL_MS: i64 = 60_000;

fn compute_wake_due_at(wake: &Value) -> Option<DateTime<Utc>> {
    match wake.get("mode").and_then(Value::as_str).unwrap_or("") {
        "interval" => {
            let created = wake.get("createdAt").and_then(Value::as_str)?;
            let delay = wake.get("requestedDelayMs").and_then(Value::as_i64)?;
            let created_dt = DateTime::parse_from_rfc3339(created).ok()?.with_timezone(&Utc);
            Some(created_dt + chrono::Duration::milliseconds(delay.max(1)))
        }
        "deadline" => {
            let next = wake.get("nextWakeAt").and_then(Value::as_str)?;
            DateTime::parse_from_rfc3339(next).ok().map(|dt| dt.with_timezone(&Utc))
        }
        _ => None,
    }
}

/// Computes due time for the automatic background scheduler.
/// For `interval` plans, enforces a minimum 60-second spacing so sub-second test/initial intervals
/// can never spin and exhaust the user's daily API budget after non-sleep decisions.
fn compute_background_wake_due_at(wake: &Value) -> Option<DateTime<Utc>> {
    match wake.get("mode").and_then(Value::as_str).unwrap_or("") {
        "interval" => {
            let created = wake.get("createdAt").and_then(Value::as_str)?;
            let delay = wake.get("requestedDelayMs").and_then(Value::as_i64)?;
            let created_dt = DateTime::parse_from_rfc3339(created).ok()?.with_timezone(&Utc);
            Some(created_dt + chrono::Duration::milliseconds(delay.max(MIN_BACKGROUND_WAKE_INTERVAL_MS)))
        }
        _ => compute_wake_due_at(wake),
    }
}

fn parse_retry_not_before(instance: &Value) -> Option<DateTime<Utc>> {
    instance
        .get("retryNotBefore")
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Spawns a one-shot `nextWakeAt`-driven background task for opt-in Autonomous instances.
/// Uses `tokio::time::sleep` + `Notify` (never periodic polling) and stays completely idle
/// when no Autonomous instance has `autoWakeEnabled == true`.
pub fn spawn_autonomous_wake_scheduler(
    pool: SqlitePool,
    vault: Vault,
    notify: Arc<Notify>,
    shutdown: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let pilot = AutonomousPilotService::system_with_vault(pool, vault);
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let next = pilot.next_scheduled_wake().await.ok().flatten();
            match next {
                None => {
                    notify.notified().await;
                }
                Some((_, due_at)) => {
                    let now = Utc::now();
                    if due_at > now {
                        let wait_ms = (due_at - now).num_milliseconds().max(50) as u64;
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_millis(wait_ms)) => {}
                            _ = notify.notified() => {}
                        }
                    } else {
                        let outcome = pilot.run_next_due_scheduled_wake().await.ok().flatten();
                        // Back off 60s on errors, None, budget exhaustion, or unavailable resources.
                        let should_backoff = outcome.is_none()
                            || outcome.as_ref().is_some_and(|v| {
                                matches!(
                                    v["result"].as_str(),
                                    Some("budget_exhausted" | "not_due" | "resource_unavailable" | "busy")
                                )
                            });
                        if should_backoff {
                            tokio::select! {
                                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
                                _ = notify.notified() => {}
                            }
                        }
                    }
                }
            }
        }
    })
}

fn is_wake_due(wake: &Value, now: DateTime<Utc>, event_kind: Option<&str>) -> Result<bool, String> {
    let interrupts = wake
        .get("interruptEvents")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if let Some(kind) = event_kind {
        if interrupts.contains(&kind) {
            return Ok(true);
        }
    }
    match wake.get("mode").and_then(Value::as_str).unwrap_or("") {
        "interval" => {
            let created = wake
                .get("createdAt")
                .and_then(Value::as_str)
                .ok_or("STORAGE_ERROR")?;
            let delay = wake
                .get("requestedDelayMs")
                .and_then(Value::as_i64)
                .ok_or("STORAGE_ERROR")?;
            let created_dt = DateTime::parse_from_rfc3339(created)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            Ok(now >= created_dt + chrono::Duration::milliseconds(delay))
        }
        "deadline" => {
            let next = wake
                .get("nextWakeAt")
                .and_then(Value::as_str)
                .ok_or("STORAGE_ERROR")?;
            let deadline = DateTime::parse_from_rfc3339(next)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            Ok(now >= deadline)
        }
        "event_only" => Ok(false),
        _ => Err("STORAGE_ERROR".into()),
    }
}

fn is_ai_sleep_blocking(wake: &Value, now: DateTime<Utc>, event_kind: &str) -> Result<bool, String> {
    if wake.get("origin").and_then(Value::as_str) != Some("ai_decision") {
        return Ok(false);
    }
    let interrupts = wake
        .get("interruptEvents")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if interrupts.contains(&event_kind) {
        return Ok(false);
    }
    match wake.get("mode").and_then(Value::as_str).unwrap_or("") {
        "deadline" => {
            let next = wake
                .get("nextWakeAt")
                .and_then(Value::as_str)
                .ok_or("STORAGE_ERROR")?;
            let deadline = DateTime::parse_from_rfc3339(next)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            Ok(now < deadline)
        }
        "event_only" => Ok(true),
        _ => Ok(false),
    }
}

fn ts(v: DateTime<Utc>) -> String {
    v.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn storage<E: std::fmt::Display>(_e: E) -> String {
    "STORAGE_ERROR".into()
}
