use ai_home::{
    application::Application,
    experimental_pilot::{AutonomousPilotService, SimulationActionExecutor, SimulationDecisionProvider},
    experimental_runtime::{ActionExecutor, ActionOutcome, Clock, DecisionChoice, DecisionProvider, OpportunityPacket},
    persistence::Store,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

fn temp_db(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ai-home-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("pilot.sqlite3")
}

struct FakeClock(Mutex<DateTime<Utc>>);
impl FakeClock {
    fn new(t: DateTime<Utc>) -> Self {
        Self(Mutex::new(t))
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}
impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct CountingDecisionProvider {
    calls: Arc<AtomicUsize>,
    choice: DecisionChoice,
}
impl DecisionProvider for CountingDecisionProvider {
    fn decide(&self, _packet: &OpportunityPacket) -> Result<DecisionChoice, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.choice.clone())
    }
}

struct CountingActionExecutor {
    calls: Arc<AtomicUsize>,
    outcome: ActionOutcome,
}
impl ActionExecutor for CountingActionExecutor {
    fn execute(&self, _intent: &ai_home::experimental_runtime::ActionIntent) -> ActionOutcome {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.outcome.clone()
    }
}

fn req(command: &str, profile_id: Option<&str>, payload: Value) -> Value {
    let mut v = json!({
        "contractVersion": 1,
        "requestId": format!("req-{}", uuid::Uuid::new_v4()),
        "command": command,
        "payload": payload,
    });
    if let Some(pid) = profile_id {
        v["profileId"] = json!(pid);
    }
    v
}

#[tokio::test]
async fn pilot_refusal_survives_restart_and_revised_terms_require_fresh_decision_without_double_exec() {
    let db = temp_db("pilot");
    let store = Store::open(&db).await.unwrap();
    let app = Application::new(store);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Pilot Nova", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    assert_eq!(created["ok"], true, "{created}");
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    // Standard profile list must never include the Autonomous instance.
    let std_list = app
        .dispatch(req("profile.list", None, json!({"limit": 50})))
        .await
        .response;
    assert_eq!(std_list["ok"], true, "{std_list}");
    assert_eq!(std_list["result"]["items"].as_array().unwrap().len(), 0);

    // Submit proposal v1.
    let sub1 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "Join local study", "terms": "Share weekly summary"}),
        ))
        .await
        .response;
    assert_eq!(sub1["ok"], true, "{sub1}");
    let proposal_id = sub1["result"]["proposal"]["id"].as_str().unwrap().to_string();
    assert_eq!(sub1["result"]["version"], 1);

    // Agent refuses v1.
    let refused = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({
                "proposalId": proposal_id,
                "simulationOutcome": "refuse",
                "note": "Scope is too broad",
                "reconsiderationConditions": ["limit_to_public_notes"]
            }),
        ))
        .await
        .response;
    assert_eq!(refused["ok"], true, "{refused}");
    assert_eq!(refused["result"]["status"], "decided");
    assert_eq!(refused["result"]["decisionOutcome"], "refusal");
    assert_eq!(refused["result"]["proposal"]["status"], "refused");
    let refusal_id = refused["result"]["refusalId"].as_str().unwrap().to_string();

    // Human message cannot override refusal.
    let neg = app
        .dispatch(req(
            "autonomous.proposal.negotiate",
            Some(&profile_id),
            json!({"proposalId": proposal_id, "kind": "response", "note": "Please do it anyway"}),
        ))
        .await
        .response;
    assert_eq!(neg["ok"], true, "{neg}");
    assert_eq!(neg["result"]["refusalPreserved"], true);
    assert_eq!(neg["result"]["proposal"]["status"], "refused");

    // Simulate full restart by closing store and reopening the SQLite database.
    drop(app);
    let reopened = Store::open(&db).await.unwrap();
    let app2 = Application::new(reopened);

    // Re-considering the same version v1 after restart must respect durable refusal.
    let retry_v1 = app2
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": proposal_id, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(retry_v1["ok"], true, "{retry_v1}");
    assert_eq!(retry_v1["result"]["status"], "refusal_active");
    assert_eq!(retry_v1["result"]["decisionSource"], "policy_guard");
    assert_eq!(retry_v1["result"]["refusalId"], refusal_id);

    // Revised terms create v2 (pending) while preserving v1 refusal record.
    let sub2 = app2
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({
                "proposalId": proposal_id,
                "title": "Join local study",
                "terms": "Only share opt-in public notes"
            }),
        ))
        .await
        .response;
    assert_eq!(sub2["ok"], true, "{sub2}");
    assert_eq!(sub2["result"]["version"], 2);
    assert_eq!(sub2["result"]["created"], false);
    assert_eq!(sub2["result"]["proposal"]["status"], "pending");

    // Agent makes a fresh recorded decision to accept v2.
    let accepted = app2
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": proposal_id, "simulationOutcome": "accept", "note": "Revised scope is acceptable"}),
        ))
        .await
        .response;
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(accepted["result"]["status"], "decided");
    assert_eq!(accepted["result"]["decisionOutcome"], "action");
    assert_eq!(accepted["result"]["proposal"]["status"], "accepted");

    // Restart again and attempt to consider v2 again -> must NOT double-execute.
    drop(app2);
    let reopened2 = Store::open(&db).await.unwrap();
    let app3 = Application::new(reopened2);
    let retry_v2 = app3
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": proposal_id, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(retry_v2["ok"], true, "{retry_v2}");
    assert_eq!(retry_v2["result"]["status"], "already_executed");
    assert_eq!(retry_v2["result"]["decisionSource"], "policy_guard");

    let status = app3
        .dispatch(req("autonomous.instance.status", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(status["ok"], true, "{status}");
    assert_eq!(status["result"]["forumConnected"], false);
    assert_eq!(status["result"]["activeRefusals"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn silence_and_sleep_are_not_consent_and_human_proposal_cannot_shorten_sleep() {
    let store = Store::memory().await.unwrap();
    let clock = Arc::new(FakeClock::new(Utc.with_ymd_and_hms(2026, 11, 25, 12, 0, 0).unwrap()));
    let pilot = AutonomousPilotService::new(store.pool.clone(), clock.clone());

    let inst = pilot.create_instance("Sleeper", 1000).await.unwrap();
    let profile_id = inst["profileId"].as_str().unwrap().to_string();
    clock.advance(Duration::seconds(2));

    let p1 = pilot
        .submit_proposal(&profile_id, None, "Quiet check", "Optional task")
        .await
        .unwrap();
    let pid1 = p1["proposal"]["id"].as_str().unwrap().to_string();

    // Silence (NoAction) is not consent.
    let decision_calls = Arc::new(AtomicUsize::new(0));
    let action_calls = Arc::new(AtomicUsize::new(0));
    let res1 = pilot
        .consider_proposal(
            &profile_id,
            &pid1,
            Arc::new(CountingDecisionProvider {
                calls: decision_calls.clone(),
                choice: DecisionChoice::NoAction,
            }),
            Arc::new(CountingActionExecutor {
                calls: action_calls.clone(),
                outcome: ActionOutcome::Accepted {
                    receipt_ref: "r1".into(),
                },
            }),
            "pilot_simulation",
            false,
            "",
        )
        .await
        .unwrap();
    assert_eq!(res1["status"], "decided");
    assert_eq!(res1["proposal"]["status"], "no_action");
    assert_eq!(action_calls.load(Ordering::SeqCst), 0);

    // Agent chooses to sleep for 1 hour.
    clock.advance(Duration::seconds(2));
    let p2 = pilot
        .submit_proposal(&profile_id, None, "Another ask", "Wait for reply")
        .await
        .unwrap();
    let pid2 = p2["proposal"]["id"].as_str().unwrap().to_string();
    let sleep_res = pilot
        .consider_proposal(
            &profile_id,
            &pid2,
            Arc::new(CountingDecisionProvider {
                calls: decision_calls.clone(),
                choice: DecisionChoice::Sleep {
                    delay_ms: 3_600_000,
                    interrupt_events: vec![],
                },
            }),
            Arc::new(CountingActionExecutor {
                calls: action_calls.clone(),
                outcome: ActionOutcome::Failed,
            }),
            "pilot_simulation",
            false,
            "",
        )
        .await
        .unwrap();
    assert_eq!(sleep_res["decisionOutcome"], "sleep");
    assert_eq!(sleep_res["activeWakePlan"]["mode"], "deadline");

    // Human submits a new proposal 10 minutes later -> must NOT shorten AI-selected sleep or call provider.
    clock.advance(Duration::minutes(10));
    let p3 = pilot
        .submit_proposal(&profile_id, None, "Urgent human ping", "Wake up now")
        .await
        .unwrap();
    let pid3 = p3["proposal"]["id"].as_str().unwrap().to_string();
    let before_calls = decision_calls.load(Ordering::SeqCst);
    let blocked = pilot
        .consider_proposal(
            &profile_id,
            &pid3,
            Arc::new(CountingDecisionProvider {
                calls: decision_calls.clone(),
                choice: DecisionChoice::NoAction,
            }),
            Arc::new(CountingActionExecutor {
                calls: action_calls.clone(),
                outcome: ActionOutcome::Failed,
            }),
            "pilot_simulation",
            false,
            "",
        )
        .await
        .unwrap();
    assert_eq!(blocked["status"], "sleeping");
    assert_eq!(blocked["decisionSource"], "policy_guard");
    assert_eq!(decision_calls.load(Ordering::SeqCst), before_calls);

    let wake_early = pilot
        .wake_check(
            &profile_id,
            Some("proposal_submitted"),
            Arc::new(CountingDecisionProvider {
                calls: decision_calls.clone(),
                choice: DecisionChoice::NoAction,
            }),
            Arc::new(CountingActionExecutor {
                calls: action_calls.clone(),
                outcome: ActionOutcome::Failed,
            }),
        )
        .await
        .unwrap();
    assert_eq!(wake_early["result"], "not_due");
    assert_eq!(decision_calls.load(Ordering::SeqCst), before_calls);

    // Once the AI-chosen deadline passes, consideration can proceed.
    clock.advance(Duration::minutes(51));
    let after_sleep = pilot
        .consider_proposal(
            &profile_id,
            &pid3,
            Arc::new(CountingDecisionProvider {
                calls: decision_calls.clone(),
                choice: DecisionChoice::Rest,
            }),
            Arc::new(CountingActionExecutor {
                calls: action_calls.clone(),
                outcome: ActionOutcome::Failed,
            }),
            "pilot_simulation",
            false,
            "",
        )
        .await
        .unwrap();
    assert_eq!(after_sleep["status"], "decided");
    assert_eq!(after_sleep["decisionOutcome"], "rest");
    assert_eq!(decision_calls.load(Ordering::SeqCst), before_calls + 1);
}

#[tokio::test]
async fn execution_failure_and_uncertain_outcome_are_not_agent_refusal() {
    let db = temp_db("exec");
    let store = Store::open(&db).await.unwrap();
    let app = Application::new(store);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Pilot Exec", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let p1 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "Run local action", "terms": "Execute once"}),
        ))
        .await
        .response;
    let pid1 = p1["result"]["proposal"]["id"].as_str().unwrap().to_string();

    // Execution failure -> execution_failed, ZERO refusals created.
    let failed = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid1, "simulationOutcome": "execution_failed"}),
        ))
        .await
        .response;
    assert_eq!(failed["ok"], true, "{failed}");
    assert_eq!(failed["result"]["status"], "execution_failed");
    assert_eq!(failed["result"]["proposal"]["status"], "execution_failed");

    let status1 = app
        .dispatch(req("autonomous.instance.status", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(status1["result"]["activeRefusals"].as_array().unwrap().len(), 0);

    // Uncertain outcome -> execution_uncertain, ZERO refusals, and fences further decisions across restart.
    let p2 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "Uncertain action", "terms": "Send once"}),
        ))
        .await
        .response;
    let pid2 = p2["result"]["proposal"]["id"].as_str().unwrap().to_string();
    let uncertain = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid2, "simulationOutcome": "execution_uncertain"}),
        ))
        .await
        .response;
    assert_eq!(uncertain["ok"], true, "{uncertain}");
    assert_eq!(uncertain["result"]["status"], "execution_uncertain");
    assert_eq!(uncertain["result"]["proposal"]["status"], "uncertain");

    drop(app);
    let reopened = Store::open(&db).await.unwrap();
    let app2 = Application::new(reopened);
    let fenced = app2
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid1, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(fenced["ok"], true, "{fenced}");
    assert_eq!(fenced["result"]["status"], "reconciliation_required");
    assert_eq!(fenced["result"]["decisionSource"], "policy_guard");
    let status2 = app2
        .dispatch(req("autonomous.instance.status", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(status2["result"]["activeRefusals"].as_array().unwrap().len(), 0);
    assert_eq!(status2["result"]["unresolvedActions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn standard_profile_rejected_by_autonomous_pilot_and_v17_tables_deleted_on_permanent_delete() {
    let store = Store::memory().await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);

    let std_profile = app
        .dispatch(req("profile.create", None, json!({"displayName": "Standard User"})))
        .await
        .response;
    let std_id = std_profile["result"]["id"].as_str().unwrap().to_string();

    let denied = app
        .dispatch(req("autonomous.instance.status", Some(&std_id), json!({})))
        .await
        .response;
    assert_eq!(denied["ok"], false);
    assert_eq!(denied["error"]["code"], "POLICY_DENIED");

    // Create an Autonomous instance with a proposal, deactivate & archive it, then permanently delete it.
    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Disposable Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let auto_id = created["result"]["profileId"].as_str().unwrap().to_string();
    let sub = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&auto_id),
            json!({"title": "Temp", "terms": "Temp terms"}),
        ))
        .await
        .response;
    assert_eq!(sub["ok"], true);

    // Mark instance inactive and profile archived for permanent deletion preflight.
    sqlx::query("UPDATE experimental_instances SET revision=revision+1, body=json_set(body,'$.revision',revision+1,'$.status','inactive') WHERE profile_id=?")
        .bind(&auto_id)
        .execute(&pool)
        .await
        .unwrap();
    let prof_rev: i64 = sqlx::query_scalar("SELECT revision FROM profiles WHERE id=?")
        .bind(&auto_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE profiles SET revision=?, body=json_set(body,'$.revision',?,'$.status','archived') WHERE id=?")
        .bind(prof_rev + 1)
        .bind(prof_rev + 1)
        .bind(&auto_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut del_req = req("profile.delete.permanent", Some(&auto_id), json!({"confirm": true}));
    del_req["expectedRevision"] = json!(prof_rev + 1);
    let deleted = app.dispatch(del_req).await.response;
    assert_eq!(deleted["ok"], true, "{deleted}");

    let proposals_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM experimental_proposals WHERE profile_id=?")
        .bind(&auto_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let rounds_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM experimental_proposal_rounds WHERE profile_id=?")
        .bind(&auto_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(proposals_left, 0);
    assert_eq!(rounds_left, 0);
}

#[test]
fn simulation_types_are_constructible() {
    let _ = SimulationDecisionProvider::decide;
    let _ = SimulationActionExecutor::execute;
}

#[derive(Default)]
struct TestSecretStore(Mutex<std::collections::HashMap<String, String>>);
impl ai_home::vault::SecretStore for TestSecretStore {
    fn read(&self, id: &str) -> Result<Option<String>, ai_home::vault::VaultError> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn write_new(&self, id: &str, value: &str) -> Result<(), ai_home::vault::VaultError> {
        let mut map = self.0.lock().unwrap();
        if map.contains_key(id) {
            return Err(ai_home::vault::VaultError::Exists);
        }
        map.insert(id.to_string(), value.to_string());
        Ok(())
    }
}

#[tokio::test]
async fn idempotency_receipts_replay_and_conflict_on_mutating_autonomous_commands() {
    let store = Store::memory().await.unwrap();
    let app = Application::new(store);

    let req1 = json!({
        "contractVersion": 1,
        "requestId": "fixed-create-req-1",
        "command": "autonomous.instance.create",
        "payload": {"displayName": "Idempotent Nova", "initialIntervalMs": 1}
    });
    let first = app.dispatch(req1.clone()).await.response;
    assert_eq!(first["ok"], true, "{first}");
    let replay = app.dispatch(req1).await.response;
    assert_eq!(replay["ok"], true, "{replay}");
    assert_eq!(first["result"]["profileId"], replay["result"]["profileId"]);

    let list = app
        .dispatch(req("autonomous.instance.list", None, json!({})))
        .await
        .response;
    assert_eq!(list["result"]["items"].as_array().unwrap().len(), 1);

    let conflict = app
        .dispatch(json!({
            "contractVersion": 1,
            "requestId": "fixed-create-req-1",
            "command": "autonomous.instance.create",
            "payload": {"displayName": "Different Name", "initialIntervalMs": 1}
        }))
        .await
        .response;
    assert_eq!(conflict["ok"], false);
    assert_eq!(conflict["error"]["code"], "IDEMPOTENCY_CONFLICT");
}

#[tokio::test]
async fn provider_adapter_uses_vault_and_http_api_enforces_daily_budget_and_handles_malformed_json() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_server = hits.clone();

    let server = tokio::spawn(async move {
        let replies = [
            json!({
                "id": "chatcmpl-1",
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {
                        "role": "assistant",
                        "content": "{\"decision\":\"refuse\",\"note\":\"Scope too wide\",\"reconsiderationConditions\":[\"narrow_scope\"]}"
                    }
                }]
            })
            .to_string(),
            json!({
                "id": "chatcmpl-2",
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {
                        "role": "assistant",
                        "content": "not-valid-json-at-all"
                    }
                }]
            })
            .to_string(),
        ];
        for body in replies {
            let Ok((mut stream, _)) = listener.accept().await else { break };
            hits_server.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });

    let store = Store::memory().await.unwrap();
    let pool = store.pool.clone();
    let vault = ai_home::vault::Vault::with_backend(Arc::new(TestSecretStore::default()));
    let app = Application::with_vault(store, vault);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "API Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let secret = "sk-test-autonomous-secret-999";
    let configured = app
        .dispatch(req(
            "autonomous.provider.configure",
            Some(&profile_id),
            json!({
                "provider": "custom_openai",
                "endpoint": format!("http://127.0.0.1:{}/v1", addr.port()),
                "modelId": "gpt-pilot-test",
                "apiKey": secret,
                "dailyCallLimit": 2,
                "maxOutputTokens": 256
            }),
        ))
        .await
        .response;
    assert_eq!(configured["ok"], true, "{configured}");
    assert_eq!(configured["result"]["hasCredential"], true);
    assert_eq!(configured["result"]["dailyCallLimit"], 2);

    // Raw API key must NEVER appear in SQLite connections or receipts.
    let conn_body: String = sqlx::query_scalar("SELECT body FROM connections WHERE profile_id=?")
        .bind(&profile_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!conn_body.contains(secret));
    let receipt_leaks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts WHERE response LIKE ? OR fingerprint LIKE ?")
        .bind(format!("%{secret}%"))
        .bind(format!("%{secret}%"))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipt_leaks, 0);

    // Submit proposal v1 -> real HTTP call returns refusal JSON.
    let p1 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "API Proposal", "terms": "Broad scope"}),
        ))
        .await
        .response;
    let pid = p1["result"]["proposal"]["id"].as_str().unwrap().to_string();

    let c1 = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid}),
        ))
        .await
        .response;
    assert_eq!(c1["ok"], true, "{c1}");
    assert_eq!(c1["result"]["status"], "decided");
    assert_eq!(c1["result"]["decisionSource"], "provider");
    assert_eq!(c1["result"]["decisionOutcome"], "refusal");
    assert_eq!(c1["result"]["proposal"]["status"], "refused");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Revise to v2 -> server returns malformed non-JSON -> resource_unavailable (never consent or refusal).
    let p2 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"proposalId": pid, "title": "API Proposal", "terms": "Narrower scope"}),
        ))
        .await
        .response;
    assert_eq!(p2["result"]["version"], 2);

    let c2 = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid}),
        ))
        .await
        .response;
    assert_eq!(c2["ok"], true, "{c2}");
    assert_eq!(c2["result"]["status"], "resource_unavailable");
    assert_eq!(c2["result"]["decisionSource"], "provider");
    assert_eq!(c2["result"]["proposal"]["status"], "pending");
    assert_eq!(hits.load(Ordering::SeqCst), 2);

    // Third call exceeds dailyCallLimit=2 -> budget_exhausted without any HTTP call.
    let c3 = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid}),
        ))
        .await
        .response;
    assert_eq!(c3["ok"], true, "{c3}");
    assert_eq!(c3["result"]["status"], "budget_exhausted");
    assert_eq!(c3["result"]["decisionSource"], "policy_guard");
    assert_eq!(hits.load(Ordering::SeqCst), 2);

    server.abort();
}

#[tokio::test]
async fn reconcile_uncertain_action_clears_recovery_fence_and_prevents_double_execution() {
    let store = Store::memory().await.unwrap();
    let app = Application::new(store);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Reconcile Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let p1 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "External step", "terms": "Run once"}),
        ))
        .await
        .response;
    let pid = p1["result"]["proposal"]["id"].as_str().unwrap().to_string();

    let uncertain = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid, "simulationOutcome": "execution_uncertain"}),
        ))
        .await
        .response;
    assert_eq!(uncertain["result"]["status"], "execution_uncertain");
    let action_id = uncertain["result"]["actionId"].as_str().unwrap().to_string();

    // Reconcile as confirmed_accepted with explicit receipt evidence.
    let rec = app
        .dispatch(req(
            "autonomous.action.reconcile",
            Some(&profile_id),
            json!({
                "actionId": action_id,
                "outcome": "confirmed_accepted",
                "receiptRef": "manual-proof-123",
                "note": "Verified externally"
            }),
        ))
        .await
        .response;
    assert_eq!(rec["ok"], true, "{rec}");
    assert_eq!(rec["result"]["instanceStatus"], "ready");
    assert_eq!(rec["result"]["remainingUnresolved"], 0);
    assert_eq!(rec["result"]["action"]["state"], "reconciled");

    // Same version v1 must now report already_executed, never double-execute.
    let retry = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(retry["ok"], true, "{retry}");
    assert_eq!(retry["result"]["status"], "already_executed");
    assert_eq!(retry["result"]["decisionSource"], "policy_guard");
}

#[tokio::test]
async fn pause_and_resume_lifecycle_blocks_consider_without_erasing_refusals() {
    let store = Store::memory().await.unwrap();
    let app = Application::new(store);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Pause Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let p1 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "Refused first", "terms": "Scope A"}),
        ))
        .await
        .response;
    let pid = p1["result"]["proposal"]["id"].as_str().unwrap().to_string();

    let refused = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid, "simulationOutcome": "refuse"}),
        ))
        .await
        .response;
    assert_eq!(refused["result"]["decisionOutcome"], "refusal");

    let paused = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "pause"}),
        ))
        .await
        .response;
    assert_eq!(paused["ok"], true, "{paused}");
    assert_eq!(paused["result"]["status"], "inactive");

    let p2 = app
        .dispatch(req(
            "autonomous.proposal.submit",
            Some(&profile_id),
            json!({"title": "While paused", "terms": "Scope B"}),
        ))
        .await
        .response;
    let pid2 = p2["result"]["proposal"]["id"].as_str().unwrap().to_string();

    let blocked = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid2, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(blocked["ok"], true, "{blocked}");
    assert_eq!(blocked["result"]["status"], "inactive");
    assert_eq!(blocked["result"]["decisionSource"], "policy_guard");

    let resumed = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "resume"}),
        ))
        .await
        .response;
    assert_eq!(resumed["ok"], true, "{resumed}");
    assert_eq!(resumed["result"]["status"], "ready");

    // Previous refusal on p1 v1 must still be active after pause/resume.
    let still_refused = app
        .dispatch(req(
            "autonomous.proposal.consider",
            Some(&profile_id),
            json!({"proposalId": pid, "simulationOutcome": "accept"}),
        ))
        .await
        .response;
    assert_eq!(still_refused["result"]["status"], "refusal_active");
}

#[tokio::test]
async fn permanent_profile_deletion_tombstones_autonomous_receipts_and_connections_keep_strict_dtos() {
    let store = Store::memory().await.unwrap();
    let pool = store.pool.clone();
    let vault = ai_home::vault::Vault::with_backend(Arc::new(TestSecretStore::default()));
    let app = Application::with_vault(store, vault);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Ephemeral Autonomous", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    assert_eq!(created["ok"], true, "{created}");
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let cfg = app
        .dispatch(req(
            "autonomous.provider.configure",
            Some(&profile_id),
            json!({
                "provider": "openrouter",
                "modelId": "openai/gpt-4o-mini",
                "apiKey": "sk-ephemeral-secret",
                "dailyCallLimit": 5,
                "maxOutputTokens": 256
            }),
        ))
        .await
        .response;
    assert_eq!(cfg["ok"], true, "{cfg}");

    // connections and routes rows MUST validate against frozen P2 DTO schemas.
    let conn_raw: String = sqlx::query_scalar("SELECT body FROM connections WHERE profile_id=?")
        .bind(&profile_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let conn_val: Value = serde_json::from_str(&conn_raw).unwrap();
    assert!(
        ai_home::domain::schema::dto("ProviderConnection", &conn_val),
        "connections row must be strict ProviderConnection DTO: {conn_val}"
    );
    let route_raw: String = sqlx::query_scalar("SELECT body FROM routes WHERE profile_id=?")
        .bind(&profile_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let route_val: Value = serde_json::from_str(&route_raw).unwrap();
    assert!(
        ai_home::domain::schema::dto("ModelRoute", &route_val),
        "routes row must be strict ModelRoute DTO: {route_val}"
    );

    let submit_req = json!({
        "contractVersion": 1,
        "requestId": "req-ephemeral-proposal-1",
        "profileId": profile_id,
        "command": "autonomous.proposal.submit",
        "payload": {"title": "Confidential Proposal", "terms": "SECRET-PROPOSAL-TERMS-XYZ"}
    });
    let submitted = app.dispatch(submit_req.clone()).await.response;
    assert_eq!(submitted["ok"], true, "{submitted}");

    // Pause, archive, and permanently delete the profile.
    let paused = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "pause"}),
        ))
        .await
        .response;
    assert_eq!(paused["ok"], true, "{paused}");

    let prof_rev: i64 = sqlx::query_scalar("SELECT revision FROM profiles WHERE id=?")
        .bind(&profile_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE profiles SET revision=?, body=json_set(body,'$.revision',?,'$.status','archived') WHERE id=?")
        .bind(prof_rev + 1)
        .bind(prof_rev + 1)
        .bind(&profile_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut del_req = req("profile.delete.permanent", Some(&profile_id), json!({"confirm": true}));
    del_req["expectedRevision"] = json!(prof_rev + 1);
    let deleted = app.dispatch(del_req).await.response;
    assert_eq!(deleted["ok"], true, "{deleted}");

    // Budget table must be cleaned.
    let budgets_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM experimental_provider_budgets WHERE profile_id=?")
        .bind(&profile_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(budgets_left, 0);

    // Every autonomous.* receipt for the deleted profile must be tombstoned (no leaked terms/titles).
    let leaked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM receipts WHERE response LIKE '%SECRET-PROPOSAL-TERMS-XYZ%' OR response LIKE '%Confidential Proposal%' OR response LIKE '%Ephemeral Autonomous%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(leaked, 0, "Autonomous receipts must be tombstoned on permanent profile deletion");

    // Replaying the tombstoned request ID must return POLICY_DENIED (never resurrected data).
    let replayed = app.dispatch(submit_req).await.response;
    assert_eq!(replayed["ok"], false, "{replayed}");
    assert_eq!(replayed["error"]["code"], "POLICY_DENIED");
}

#[tokio::test]
async fn wake_check_uses_configured_api_provider_and_respects_ai_sleep_without_extra_calls() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_server = hits.clone();

    let server = tokio::spawn(async move {
        let body = json!({
            "id": "chatcmpl-wake-1",
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "{\"decision\":\"sleep\",\"sleepDelayMs\":3600000}"
                }
            }]
        })
        .to_string();
        if let Ok((mut stream, _)) = listener.accept().await {
            hits_server.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf).await;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });

    let store = Store::memory().await.unwrap();
    let vault = ai_home::vault::Vault::with_backend(Arc::new(TestSecretStore::default()));
    let app = Application::with_vault(store, vault);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Wake API Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    let cfg = app
        .dispatch(req(
            "autonomous.provider.configure",
            Some(&profile_id),
            json!({
                "provider": "custom_openai",
                "endpoint": format!("http://127.0.0.1:{}/v1", addr.port()),
                "modelId": "gpt-wake-test",
                "apiKey": "sk-wake-secret",
                "dailyCallLimit": 5,
                "maxOutputTokens": 128
            }),
        ))
        .await
        .response;
    assert_eq!(cfg["ok"], true, "{cfg}");

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let wake1 = app
        .dispatch(req("autonomous.wake.check", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(wake1["ok"], true, "{wake1}");
    assert_eq!(wake1["result"]["result"], "completed");
    assert_eq!(wake1["result"]["outcome"], "sleep");
    assert_eq!(wake1["result"]["activeWakePlan"]["mode"], "deadline");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // Second wake check before deadline must return not_due without calling HTTP.
    let wake2 = app
        .dispatch(req("autonomous.wake.check", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(wake2["ok"], true, "{wake2}");
    assert_eq!(wake2["result"]["result"], "not_due");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let status = app
        .dispatch(req("autonomous.instance.status", Some(&profile_id), json!({})))
        .await
        .response;
    assert_eq!(status["ok"], true, "{status}");
    assert_eq!(status["result"]["continuitySummary"]["privateContentIncluded"], false);
    assert_eq!(status["result"]["continuitySummary"]["privateReflectionCount"], 0);

    server.abort();
}

#[tokio::test]
async fn archive_lifecycle_requires_inactive_and_enables_permanent_delete() {
    let store = Store::memory().await.unwrap();
    let app = Application::new(store);

    let created = app
        .dispatch(req(
            "autonomous.instance.create",
            None,
            json!({"displayName": "Archive Pilot", "initialIntervalMs": 1}),
        ))
        .await
        .response;
    let profile_id = created["result"]["profileId"].as_str().unwrap().to_string();

    // Archiving while ready must be denied.
    let denied = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "archive"}),
        ))
        .await
        .response;
    assert_eq!(denied["ok"], false);
    assert_eq!(denied["error"]["code"], "POLICY_DENIED");

    // Pause -> archive -> permanent delete succeeds cleanly.
    let paused = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "pause"}),
        ))
        .await
        .response;
    assert_eq!(paused["ok"], true, "{paused}");

    let archived = app
        .dispatch(req(
            "autonomous.instance.lifecycle",
            Some(&profile_id),
            json!({"action": "archive"}),
        ))
        .await
        .response;
    assert_eq!(archived["ok"], true, "{archived}");
    assert_eq!(archived["result"]["status"], "archived");
    let rev = archived["result"]["profileRevision"].as_i64().unwrap();

    let mut del = req("profile.delete.permanent", Some(&profile_id), json!({"confirm": true}));
    del["expectedRevision"] = json!(rev);
    let deleted = app.dispatch(del).await.response;
    assert_eq!(deleted["ok"], true, "{deleted}");
}



