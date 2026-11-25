use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use crate::domain::schema;
use crate::experimental_runtime::{
    ActionExecutor, ActionIntent, ActionOutcome, Clock, DecisionChoice, DecisionProvider,
    DisabledActionExecutor, DisabledDecisionProvider, ExperimentalRuntime, OpportunityEvent,
    OpportunityPacket, RunResult, SystemClock,
};

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
            | "autonomous.proposal.submit"
            | "autonomous.proposal.negotiate"
            | "autonomous.proposal.consider"
            | "autonomous.wake.check"
    )
}

pub fn mutates(command: &str) -> bool {
    matches!(
        command,
        "autonomous.instance.create"
            | "autonomous.proposal.submit"
            | "autonomous.proposal.negotiate"
            | "autonomous.proposal.consider"
            | "autonomous.wake.check"
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

pub struct AutonomousPilotService<C: Clock> {
    pool: SqlitePool,
    clock: Arc<C>,
}

impl AutonomousPilotService<SystemClock> {
    pub fn system(pool: SqlitePool) -> Self {
        Self::new(pool, Arc::new(SystemClock))
    }
}

impl<C: Clock> AutonomousPilotService<C> {
    pub fn new(pool: SqlitePool, clock: Arc<C>) -> Self {
        Self { pool, clock }
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
                    self.consider_proposal(
                        profile_id,
                        proposal_id,
                        Arc::new(DisabledDecisionProvider),
                        Arc::new(DisabledActionExecutor),
                        "provider",
                        false,
                        note,
                    )
                    .await?
                }
            }
            "autonomous.wake.check" => {
                let profile_id = r["profileId"].as_str().ok_or("VALIDATION_ERROR")?;
                self.wake_check(
                    profile_id,
                    payload.get("eventKind").and_then(Value::as_str),
                    Arc::new(DisabledDecisionProvider),
                    Arc::new(DisabledActionExecutor),
                )
                .await?
            }
            _ => return Err("UNKNOWN_COMMAND".into()),
        };
        if !schema::valid(&result, &contract["result"]) {
            return Err("STORAGE_ERROR".into());
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
            "SELECT e.profile_id, e.id, e.body, p.body AS profile_body \
             FROM experimental_instances e \
             JOIN profiles p ON p.id = e.profile_id \
             ORDER BY e.rowid DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(storage)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let profile_id: String = row.get("profile_id");
            let instance_id: String = row.get("id");
            let instance: Value = serde_json::from_str(row.get::<&str, _>("body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
            let profile: Value = serde_json::from_str(row.get::<&str, _>("profile_body"))
                .map_err(|_| "STORAGE_ERROR".to_string())?;
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
                "displayName": profile["displayName"].as_str().unwrap_or(&profile_id),
                "status": instance["status"],
                "activeWakePlan": wake,
                "proposalCount": proposal_count,
                "activeRefusalCount": refusal_count,
            }));
        }
        Ok(json!({ "items": items }))
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
        Ok(json!({
            "profileId": profile_id,
            "displayName": display_name,
            "instance": instance,
            "activeWakePlan": active_wake,
            "activeRefusals": active_refusals,
            "proposals": proposals,
            "recentDecisions": recent_decisions,
            "unresolvedActions": unresolved_actions,
            "forumConnected": false,
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
        // Negotiation messages never override an active refusal or re-trigger an accepted execution.
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
        let (instance_id, instance) = self.require_instance(profile_id, None).await?;
        let now = self.clock.now();
        let proposal = self.get_proposal(profile_id, &instance_id, proposal_id).await?;
        let version = proposal["version"].as_i64().unwrap_or(1);
        let scope_key = proposal_scope(proposal_id, version);
        let scope_fp = proposal_scope_fingerprint(proposal_id, version);
        let active_wake = self.active_wake(profile_id, &instance_id).await?;

        // 1. Execution fence: unresolved sent/uncertain external action or recovery_required blocks new decisions.
        let unresolved = self.unresolved_actions(profile_id, &instance_id).await?;
        if !unresolved.is_empty() || instance["status"].as_str() == Some("recovery_required") {
            return Ok(json!({
                "status": "reconciliation_required",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            }));
        }

        // 2. No double execution for the same proposal version (including after restart).
        let already_in_list = proposal["executedVersions"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|v| v.as_i64() == Some(version)));
        let accepted_action_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM experimental_actions \
             WHERE profile_id=? AND instance_id=? \
             AND json_extract(body,'$.idempotencyKey')=? \
             AND json_extract(body,'$.state')='accepted'",
        )
        .bind(profile_id)
        .bind(&instance_id)
        .bind(&scope_key)
        .fetch_one(&self.pool)
        .await
        .map_err(storage)?;
        if already_in_list || accepted_action_count > 0 {
            return Ok(json!({
                "status": "already_executed",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": active_wake,
            }));
        }

        // 3. Durable refusal for the same proposal version cannot be overridden by human retry or restart.
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
            return Ok(json!({
                "status": "refusal_active",
                "decisionSource": "policy_guard",
                "refusalId": refusal_id,
                "proposal": proposal,
                "activeWakePlan": active_wake,
            }));
        }

        // 4. Respect AI-selected sleep / wait_event WakePlan: human proposal cannot shorten sleep.
        if let Some(wake) = &active_wake {
            if is_ai_sleep_blocking(wake, now, "proposal_submitted")? {
                return Ok(json!({
                    "status": "sleeping",
                    "decisionSource": "policy_guard",
                    "proposal": proposal,
                    "activeWakePlan": active_wake,
                }));
            }
        }

        // 5. Run through ExperimentalRuntime so every decision, refusal, wake plan and action ledger is canonical.
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
            Ok(RunResult::ReconciliationRequired) => Ok(json!({
                "status": "reconciliation_required",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Ok(RunResult::ResourceUnavailable) => {
                // Silence / provider unavailability is neither consent nor agent refusal.
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
            Ok(RunResult::Busy) | Ok(RunResult::Inactive) => Ok(json!({
                "status": "not_due",
                "decisionSource": "policy_guard",
                "proposal": proposal,
                "activeWakePlan": updated_wake,
            })),
            Err(code) if code == "ACTION_OUTCOME_UNCERTAIN" => {
                // Uncertain execution is an execution state, never an agent refusal.
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
                    // Execution failure is distinct from agent refusal: no RefusalRecord is created.
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

    pub async fn wake_check<D: DecisionProvider, A: ActionExecutor>(
        &self,
        profile_id: &str,
        event_kind: Option<&str>,
        decision_provider: Arc<D>,
        action_executor: Arc<A>,
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
        );
        let run = runtime.run_due(profile_id, &instance_id, &events).await?;
        let active_wake = self.active_wake(profile_id, &instance_id).await?;
        Ok(match run {
            RunResult::NotDue => json!({ "result": "not_due", "activeWakePlan": active_wake }),
            RunResult::Busy => json!({ "result": "busy", "activeWakePlan": active_wake }),
            RunResult::Inactive => json!({ "result": "inactive", "activeWakePlan": active_wake }),
            RunResult::ResourceUnavailable => {
                json!({ "result": "resource_unavailable", "activeWakePlan": active_wake })
            }
            RunResult::ReconciliationRequired => {
                json!({ "result": "reconciliation_required", "activeWakePlan": active_wake })
            }
            RunResult::Completed {
                outcome,
                decision_id,
            } => json!({
                "result": "completed",
                "outcome": outcome,
                "decisionId": decision_id,
                "activeWakePlan": active_wake,
            }),
        })
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
            sqlx::query("SELECT id, body FROM experimental_instances WHERE profile_id=? AND id=?")
                .bind(profile_id)
                .bind(iid)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
        } else {
            sqlx::query("SELECT id, body FROM experimental_instances WHERE profile_id=? ORDER BY rowid DESC LIMIT 1")
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
