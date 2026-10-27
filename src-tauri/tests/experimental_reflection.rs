use ai_home::{
    domain::schema,
    experimental_reflection::{
        read_private, ReflectionProvider, ReflectionRequest, SelfReview, SelfWithdrawal,
    },
    experimental_runtime::{
        ActionExecutor, ActionIntent, ActionOutcome, Clock, DecisionChoice, DecisionProvider,
        ExperimentalRuntime, OpportunityPacket, RunResult,
    },
    persistence::Store,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
struct Time(Mutex<DateTime<Utc>>);
impl Time {
    fn new() -> Self {
        Self(Mutex::new(
            DateTime::parse_from_rfc3339("2026-10-26T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        ))
    }
    fn advance(&self, ms: i64) {
        let mut v = self.0.lock().unwrap();
        *v = *v + Duration::milliseconds(ms);
    }
}
impl Clock for Time {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}
#[derive(Default)]
struct Primary {
    choices: Mutex<VecDeque<DecisionChoice>>,
    packets: Mutex<Vec<OpportunityPacket>>,
}
impl DecisionProvider for Primary {
    fn decide(&self, p: &OpportunityPacket) -> Result<DecisionChoice, String> {
        self.packets.lock().unwrap().push(p.clone());
        Ok(self
            .choices
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(DecisionChoice::NoAction))
    }
}
#[derive(Default)]
struct Executor(AtomicUsize);
impl ActionExecutor for Executor {
    fn execute(&self, _: &ActionIntent) -> ActionOutcome {
        self.0.fetch_add(1, Ordering::SeqCst);
        ActionOutcome::Failed
    }
}
#[derive(Default)]
struct Generator {
    outputs: Mutex<VecDeque<Value>>,
    calls: AtomicUsize,
    inputs: Mutex<Vec<Value>>,
    hold: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    delay: AtomicUsize,
}
impl ReflectionProvider for Generator {
    fn generate<'a>(
        &'a self,
        input: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.inputs.lock().unwrap().push(input);
            self.entered.notify_one();
            if self.hold.load(Ordering::SeqCst) {
                self.release.notified().await;
            }
            let ms = self.delay.load(Ordering::SeqCst);
            if ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
            }
            Ok(self
                .outputs
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(json!({"conclusion":"none"})))
        })
    }
}
type Runtime = ExperimentalRuntime<Time, Primary, Executor>;
struct Fixture {
    store: Store,
    runtime: Arc<Runtime>,
    clock: Arc<Time>,
    primary: Arc<Primary>,
    generator: Arc<Generator>,
    executor: Arc<Executor>,
}
async fn fixture() -> Fixture {
    make(Store::memory().await.unwrap()).await
}
async fn make(store: Store) -> Fixture {
    let clock = Arc::new(Time::new());
    let primary = Arc::new(Primary::default());
    let generator = Arc::new(Generator::default());
    let executor = Arc::new(Executor::default());
    let runtime = Arc::new(
        ExperimentalRuntime::new(
            store.pool.clone(),
            clock.clone(),
            primary.clone(),
            executor.clone(),
        )
        .with_reflection_provider(generator.clone()),
    );
    runtime.create_instance("p", "exp", 1000).await.unwrap();
    Fixture {
        store,
        runtime,
        clock,
        primary,
        generator,
        executor,
    }
}
async fn tick(f: &Fixture, choice: DecisionChoice) -> Result<RunResult, String> {
    f.primary.choices.lock().unwrap().push_back(choice);
    f.clock.advance(1000);
    f.runtime.run_due("p", "exp", &[]).await
}
async fn rows(pool: &SqlitePool, table: &str) -> Vec<Value> {
    sqlx::query_scalar::<_, String>(&format!(
        "SELECT body FROM {table} WHERE profile_id='p' ORDER BY rowid"
    ))
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|s| serde_json::from_str(s).unwrap())
    .collect()
}
async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {table} WHERE profile_id='p'"
    ))
    .fetch_one(pool)
    .await
    .unwrap()
}
fn draft(kind: &str, text: &str) -> Value {
    json!({"conclusion":"draft","summary":"An explicit private hypothesis, not evidence.","candidates":[{"kind":kind,"content":text}]})
}
async fn generate(f: &Fixture, mode: &str, kind: &str, text: &str) -> Value {
    f.generator
        .outputs
        .lock()
        .unwrap()
        .push_back(draft(kind, text));
    let r = if mode == "dream" {
        DecisionChoice::Dream(ReflectionRequest::default())
    } else {
        DecisionChoice::Reflect(ReflectionRequest::default())
    };
    assert!(matches!(
        tick(f, r).await.unwrap(),
        RunResult::Completed { .. }
    ));
    rows(&f.store.pool, "experimental_private_reflections")
        .await
        .last()
        .unwrap()
        .clone()
}
fn review(r: &Value, disposition: &str, content: Option<&str>) -> DecisionChoice {
    DecisionChoice::ReviewSelf(SelfReview {
        reflection_id: r["id"].as_str().unwrap().into(),
        reflection_revision: r["revision"].as_i64().unwrap(),
        candidate_index: 0,
        disposition: disposition.into(),
        modified_content: content.map(str::to_string),
        reason: "Fresh Primary decision after reading the candidate.".into(),
    })
}
async fn approve(f: &Fixture, kind: &str, text: &str) -> Value {
    let r = generate(f, "reflection", kind, text).await;
    tick(f, review(&r, "accept", None)).await.unwrap();
    rows(&f.store.pool, "experimental_self_records")
        .await
        .last()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn no_conclusion_is_a_success_without_fabricated_note_or_publication() {
    let f = fixture().await;
    tick(&f, DecisionChoice::Reflect(ReflectionRequest::default()))
        .await
        .unwrap();
    let runs = rows(&f.store.pool, "experimental_reflection_runs").await;
    assert_eq!(runs[0]["result"], "no_conclusion");
    assert_eq!(runs[0]["generationCallCount"], 1);
    assert!(schema::dto("ReflectionRun", &runs[0]));
    assert_eq!(
        count(&f.store.pool, "experimental_private_reflections").await,
        0
    );
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
    assert_eq!(f.executor.0.load(Ordering::SeqCst), 0);
    assert_eq!(count(&f.store.pool, "memory_versions").await, 0);
}
#[tokio::test]
async fn dream_is_speculation_and_never_self_approves_a_value() {
    let f = fixture().await;
    let r = generate(&f, "dream", "value", "I may value careful listening.").await;
    assert_eq!(r["mode"], "dream");
    assert_eq!(r["epistemicStatus"], "synthetic_hypothesis");
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
    assert_eq!(f.executor.0.load(Ordering::SeqCst), 0);
    tick(&f, DecisionChoice::NoAction).await.unwrap();
    let packets = f.primary.packets.lock().unwrap();
    let c = &packets.last().unwrap().private_context;
    assert_eq!(
        c["candidates"][0]["epistemicStatus"],
        "synthetic_hypothesis"
    );
    assert!(c["records"].as_array().unwrap().is_empty());
    assert!(!c.to_string().contains("An explicit private hypothesis"));
}
#[tokio::test]
async fn own_note_requires_a_separate_primary_cycle_and_keeps_provenance() {
    let f = fixture().await;
    let r = generate(
        &f,
        "reflection",
        "own_note",
        "Remember to check source dates.",
    )
    .await;
    assert!(schema::dto("PrivateReflection", &r));
    tick(&f, review(&r, "accept", None)).await.unwrap();
    let notes = rows(&f.store.pool, "experimental_self_records").await;
    let note = &notes[0];
    assert_eq!(note["sourceReflectionId"], r["id"]);
    assert_eq!(note["originMode"], "reflection");
    assert_eq!(note["epistemicStatus"], "self_authored_not_verified_fact");
    assert!(schema::dto("PrivateSelfRecord", note));
    let decisions = rows(&f.store.pool, "experimental_decisions").await;
    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[1]["outcome"], "self_review");
    assert_eq!(decisions[1]["id"], note["primaryDecisionId"]);
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f.store.pool, "memory_versions").await, 0);
}
#[tokio::test]
async fn modify_measures_actual_text_and_reject_preserves_existing_records() {
    let f = fixture().await;
    let r = generate(&f, "dream", "value", "Speculative value").await;
    tick(&f, review(&r, "modify", Some("Żółw"))).await.unwrap();
    let records = rows(&f.store.pool, "experimental_self_records").await;
    assert_eq!(records[0]["content"], "Żółw");
    assert_eq!(records[0]["measurement"]["value"], "Żółw".len());
    assert_eq!(records[0]["originMode"], "dream");
    let r = generate(&f, "reflection", "value", "Contradictory speculative value").await;
    tick(&f, review(&r, "reject", None)).await.unwrap();
    assert_eq!(
        rows(&f.store.pool, "experimental_self_records").await,
        records
    );
    assert_eq!(
        rows(&f.store.pool, "experimental_self_reviews").await.len(),
        2
    );
}
#[tokio::test]
async fn oversized_modified_value_fails_without_activating_or_consuming_candidate() {
    let f = fixture().await;
    let r = generate(&f, "reflection", "value", "Small").await;
    assert_eq!(
        tick(&f, review(&r, "modify", Some(&"x".repeat(301))))
            .await
            .unwrap_err(),
        "CONTEXT_BUDGET_EXCEEDED"
    );
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
    assert_eq!(count(&f.store.pool, "experimental_self_reviews").await, 0);
    assert_eq!(count(&f.store.pool, "experimental_decisions").await, 1);
}
#[tokio::test]
async fn duplicate_review_is_rejected_and_never_creates_a_second_version() {
    let f = fixture().await;
    let r = generate(&f, "reflection", "own_note", "One note").await;
    tick(&f, review(&r, "accept", None)).await.unwrap();
    assert!(tick(&f, review(&r, "accept", None)).await.is_err());
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 1);
    assert_eq!(count(&f.store.pool, "experimental_self_reviews").await, 1);
}
#[tokio::test]
async fn approved_note_and_value_can_be_revised_and_withdrawn_without_losing_history() {
    let f = fixture().await;
    let first = approve(&f, "value", "Listen carefully.").await;
    let mut output = draft("value", "Listen and verify.");
    output["candidates"][0]["supersedesId"] = first["id"].clone();
    f.generator.outputs.lock().unwrap().push_back(output);
    tick(&f, DecisionChoice::Reflect(ReflectionRequest::default()))
        .await
        .unwrap();
    let r = rows(&f.store.pool, "experimental_private_reflections")
        .await
        .last()
        .unwrap()
        .clone();
    tick(&f, review(&r, "accept", None)).await.unwrap();
    let notes = rows(&f.store.pool, "experimental_self_records").await;
    assert_eq!(notes[0]["status"], "superseded");
    assert_eq!(notes[1]["previousRecordId"], notes[0]["id"]);
    let n = &notes[1];
    tick(
        &f,
        DecisionChoice::WithdrawSelf(SelfWithdrawal {
            record_id: n["id"].as_str().unwrap().into(),
            expected_revision: 1,
            reason: "I reconsidered this value.".into(),
        }),
    )
    .await
    .unwrap();
    let notes = rows(&f.store.pool, "experimental_self_records").await;
    assert_eq!(notes[1]["status"], "withdrawn");
    assert_eq!(notes[0]["content"], first["content"]);
    tick(&f, DecisionChoice::NoAction).await.unwrap();
    assert!(f
        .primary
        .packets
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .private_context["records"]
        .as_array()
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn profile_and_instance_scope_do_not_allow_foreign_private_reads_or_reviews() {
    let f = fixture().await;
    let r = generate(&f, "dream", "own_note", "PRIVATE_A").await;
    f.runtime.create_instance("q", "other", 1000).await.unwrap();
    assert!(
        read_private(&f.store.pool, "q", "other", r["id"].as_str().unwrap())
            .await
            .is_err()
    );
    assert!(
        read_private(&f.store.pool, "p", "other", r["id"].as_str().unwrap())
            .await
            .is_err()
    );
    f.clock.advance(1000);
    f.primary
        .choices
        .lock()
        .unwrap()
        .push_back(review(&r, "accept", None));
    assert!(f.runtime.run_due("q", "other", &[]).await.is_err());
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
}
#[tokio::test]
async fn impossible_request_is_rejected_before_generator_or_run_reservation() {
    let f = fixture().await;
    let mut r = ReflectionRequest::default();
    r.context_bytes = 1024;
    r.output_bytes = 8192;
    assert_eq!(
        tick(&f, DecisionChoice::Reflect(r)).await.unwrap_err(),
        "CONTEXT_BUDGET_EXCEEDED"
    );
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        count(&f.store.pool, "experimental_reflection_runs").await,
        0
    );
    assert_eq!(count(&f.store.pool, "experimental_decisions").await, 0);
}
#[tokio::test]
async fn malformed_or_over_budget_output_has_no_partial_private_records() {
    for output in [
        json!({"conclusion":"draft","summary":"x","candidates":[],"rawReasoning":"forbidden"}),
        draft("value", &"x".repeat(5000)),
        json!({"conclusion":"draft","summary":"x","candidates":(0..9).map(|_|json!({"kind":"value","content":"x"})).collect::<Vec<_>>()}),
    ] {
        let f = fixture().await;
        f.generator.outputs.lock().unwrap().push_back(output);
        assert!(
            tick(&f, DecisionChoice::Dream(ReflectionRequest::default()))
                .await
                .is_err()
        );
        assert_eq!(
            count(&f.store.pool, "experimental_private_reflections").await,
            0
        );
        assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
        let r = rows(&f.store.pool, "experimental_reflection_runs").await;
        assert_eq!(r[0]["status"], "failed");
        assert_eq!(r[0]["generationCallCount"], 1);
        assert_eq!(f.generator.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn generator_timeout_is_bounded_without_implicit_retry_or_fake_refusal() {
    let f = fixture().await;
    f.generator.delay.store(100, Ordering::SeqCst);
    let mut r = ReflectionRequest::default();
    r.max_elapsed_ms = 1;
    assert_eq!(
        tick(&f, DecisionChoice::Reflect(r)).await.unwrap_err(),
        "PROVIDER_TIMEOUT"
    );
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f.store.pool, "experimental_refusals").await, 0);
    assert_eq!(
        rows(&f.store.pool, "experimental_reflection_runs").await[0]["errorCode"],
        "PROVIDER_TIMEOUT"
    );
    assert_eq!(
        f.runtime.run_due("p", "exp", &[]).await.unwrap(),
        RunResult::NotDue
    );
}
#[tokio::test]
async fn stale_generation_during_generation_rejects_late_output() {
    let f = fixture().await;
    f.generator.hold.store(true, Ordering::SeqCst);
    f.generator
        .outputs
        .lock()
        .unwrap()
        .push_back(draft("value", "Late"));
    f.primary
        .choices
        .lock()
        .unwrap()
        .push_back(DecisionChoice::Reflect(ReflectionRequest::default()));
    f.clock.advance(1000);
    let rt = f.runtime.clone();
    let task = tokio::spawn(async move { rt.run_due("p", "exp", &[]).await });
    f.generator.entered.notified().await;
    sqlx::query("UPDATE experimental_instances SET revision=revision+1,body=json_set(body,'$.revision',revision+1,'$.runtimeGeneration',99,'$.status','ready') WHERE profile_id='p'").execute(&f.store.pool).await.unwrap();
    f.generator.release.notify_one();
    assert_eq!(task.await.unwrap().unwrap_err(), "STALE_EXECUTOR");
    assert_eq!(
        count(&f.store.pool, "experimental_private_reflections").await,
        0
    );
}
#[tokio::test]
async fn storage_failure_rolls_back_output_pointer_and_keeps_old_note() {
    let f = fixture().await;
    let old = approve(&f, "own_note", "Retained").await;
    f.generator
        .outputs
        .lock()
        .unwrap()
        .push_back(draft("value", "New"));
    sqlx::query("CREATE TRIGGER reject_private BEFORE INSERT ON experimental_private_reflections BEGIN SELECT RAISE(ABORT,'test'); END").execute(&f.store.pool).await.unwrap();
    assert_eq!(
        tick(&f, DecisionChoice::Dream(ReflectionRequest::default()))
            .await
            .unwrap_err(),
        "STORAGE_ERROR"
    );
    assert_eq!(
        count(&f.store.pool, "experimental_private_reflections").await,
        1
    );
    assert_eq!(
        rows(&f.store.pool, "experimental_self_records").await[0],
        old
    );
    assert_eq!(
        rows(&f.store.pool, "experimental_reflection_runs")
            .await
            .last()
            .unwrap()["status"],
        "failed"
    );
}
#[tokio::test]
async fn context_is_bounded_whole_entries_and_never_promotes_unreviewed_summaries() {
    let f = fixture().await;
    for n in 0..8 {
        approve(&f, "own_note", &format!("Note {n}")).await;
    }
    tick(&f, DecisionChoice::NoAction).await.unwrap();
    let p = f.primary.packets.lock().unwrap();
    let c = &p.last().unwrap().private_context;
    assert!(c.to_string().len() <= 4096);
    assert!(c["omittedCount"].as_u64().unwrap() > 0);
    assert!(!c.to_string().contains("An explicit private hypothesis"));
    for r in c["records"].as_array().unwrap() {
        assert!(r["content"].as_str().unwrap().starts_with("Note "));
    }
}
#[tokio::test]
async fn ai_sleep_and_refusal_remain_decisions_not_reflection_triggers() {
    let f = fixture().await;
    tick(
        &f,
        DecisionChoice::Refusal {
            scope: "Do not publish".into(),
            reconsideration_conditions: vec![],
        },
    )
    .await
    .unwrap();
    tick(
        &f,
        DecisionChoice::Sleep {
            delay_ms: 100000,
            interrupt_events: vec!["selected".into()],
        },
    )
    .await
    .unwrap();
    for _ in 0..10 {
        f.clock.advance(1000);
        assert_eq!(
            f.runtime.run_due("p", "exp", &[]).await.unwrap(),
            RunResult::NotDue
        );
    }
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 0);
    assert_eq!(count(&f.store.pool, "experimental_refusals").await, 1);
    assert_eq!(f.executor.0.load(Ordering::SeqCst), 0);
}

fn request(command: &str, payload: Value, profile: Option<&str>) -> Value {
    let mut r = json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});
    if let Some(p) = profile {
        r["profileId"] = json!(p);
    }
    r
}
fn directory() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("ai-home-reflection-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
#[tokio::test]
async fn actual_v14_migration_preserves_preferences_and_existing_instance_without_generation() {
    let d = directory();
    let path = d.join("foundation.sqlite3");
    let f = make(Store::open(&path).await.unwrap()).await;
    let before: String =
        sqlx::query_scalar("SELECT body FROM experimental_instances WHERE id='exp'")
            .fetch_one(&f.store.pool)
            .await
            .unwrap();
    let prefs = f.store.preferences().await.unwrap();
    for t in [
        "experimental_self_records",
        "experimental_self_reviews",
        "experimental_private_reflections",
        "experimental_reflection_runs",
    ] {
        sqlx::query(&format!("DROP TABLE {t}"))
            .execute(&f.store.pool)
            .await
            .unwrap();
    }
    sqlx::query("PRAGMA user_version=14")
        .execute(&f.store.pool)
        .await
        .unwrap();
    f.store.pool.close().await;
    drop(f);
    let migrated = Store::open(&path).await.unwrap();
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&migrated.pool)
        .await
        .unwrap();
    assert_eq!(version, 16);
    assert_eq!(migrated.preferences().await.unwrap(), prefs);
    let after: String =
        sqlx::query_scalar("SELECT body FROM experimental_instances WHERE id='exp'")
            .fetch_one(&migrated.pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        count(&migrated.pool, "experimental_reflection_runs").await,
        0
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&migrated.pool)
        .await
        .unwrap()
        .is_empty());
    migrated.pool.close().await;
    std::fs::remove_dir_all(d).unwrap();
}
#[tokio::test]
async fn restart_interrupts_reserved_run_and_does_not_regenerate_or_lose_approved_note() {
    let d = directory();
    let path = d.join("foundation.sqlite3");
    let f = make(Store::open(&path).await.unwrap()).await;
    let note = approve(&f, "own_note", "PRIVATE_DURABLE").await;
    f.generator.hold.store(true, Ordering::SeqCst);
    f.primary
        .choices
        .lock()
        .unwrap()
        .push_back(DecisionChoice::Dream(ReflectionRequest::default()));
    f.clock.advance(1000);
    let runtime = f.runtime.clone();
    let task = tokio::spawn(async move { runtime.run_due("p", "exp", &[]).await });
    f.generator.entered.notified().await;
    // Drain the previous approval's notification before waiting for the held generation.
    while f.generator.calls.load(Ordering::SeqCst) < 2 {
        tokio::task::yield_now().await;
    }
    task.abort();
    let _ = task.await;
    let calls = f.generator.calls.load(Ordering::SeqCst);
    f.store.pool.close().await;
    let reopened = Store::open(&path).await.unwrap();
    let runs = rows(&reopened.pool, "experimental_reflection_runs").await;
    assert_eq!(runs.last().unwrap()["status"], "interrupted");
    assert_eq!(runs.last().unwrap()["errorCode"], "OPERATION_INTERRUPTED");
    assert!(schema::dto("ReflectionRun", runs.last().unwrap()));
    assert_eq!(
        rows(&reopened.pool, "experimental_self_records").await[0],
        note
    );
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), calls);
    reopened.pool.close().await;
    drop(f);
    std::fs::remove_dir_all(d).unwrap();
}
#[tokio::test]
async fn full_backup_restore_keeps_private_records_but_portable_export_and_diagnostics_exclude_them(
) {
    use ai_home::{application::Application, backup, diagnostics};
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::io::Read;
    let d = directory();
    let path = d.join("foundation.sqlite3");
    let f = make(Store::open(&path).await.unwrap()).await;
    let note = approve(&f, "value", "PRIVATE_UNIQUE_VALUE_4B11").await;
    let diagnostics = diagnostics::export(&f.store).await.unwrap();
    let bytes = STANDARD
        .decode(diagnostics["dataBase64"].as_str().unwrap())
        .unwrap();
    assert!(!String::from_utf8(bytes)
        .unwrap()
        .contains("PRIVATE_UNIQUE_VALUE_4B11"));
    let backup = backup::dispatch(&f.store, &request("backup.create", json!({}), None))
        .await
        .unwrap();
    assert_eq!(backup["schemaVersion"], 16);
    let bid = backup["id"].as_str().unwrap();
    let app = Application::new(Store::open(&path).await.unwrap());
    let ex = app
        .dispatch(request("profile.export.create", json!({}), Some("p")))
        .await
        .response;
    assert_eq!(ex["ok"], true, "{ex}");
    let eid = ex["result"]["id"].as_str().unwrap();
    let mut zip = zip::ZipArchive::new(
        std::fs::File::open(d.join("exports/p").join(format!("{eid}.zip"))).unwrap(),
    )
    .unwrap();
    for n in 0..zip.len() {
        let mut e = zip.by_index(n).unwrap();
        let mut b = String::new();
        e.read_to_string(&mut b).unwrap();
        assert!(!b.contains("PRIVATE_UNIQUE_VALUE_4B11"));
        assert!(!e.name().contains("reflection"));
    }
    drop(zip);
    drop(app);
    sqlx::query("UPDATE experimental_self_records SET body=json_set(body,'$.content','changed') WHERE profile_id='p'").execute(&f.store.pool).await.unwrap();
    let req = request("backup.restore.request", json!({"backupId":bid}), None);
    backup::dispatch(&f.store, &req).await.unwrap();
    f.store.pool.close().await;
    drop(f);
    assert_eq!(
        backup::apply_pending_restore(&d).await.unwrap().unwrap()["applied"],
        true
    );
    let restored = Store::open(&path).await.unwrap();
    assert_eq!(
        rows(&restored.pool, "experimental_self_records").await[0],
        note
    );
    assert_eq!(count(&restored.pool, "experimental_self_reviews").await, 1);
    assert_eq!(
        count(&restored.pool, "experimental_private_reflections").await,
        1
    );
    restored.pool.close().await;
    std::fs::remove_dir_all(d).unwrap();
}
#[tokio::test]
async fn permanent_profile_deletion_removes_real_reflections_reviews_and_self_records_only_for_target(
) {
    use ai_home::application::Application;
    let f = fixture().await;
    approve(&f, "own_note", "PRIVATE_DELETE").await;
    f.runtime
        .create_instance("other", "other-exp", 1000)
        .await
        .unwrap();
    sqlx::query("UPDATE experimental_instances SET body=json_set(body,'$.status','inactive') WHERE profile_id='p'").execute(&f.store.pool).await.unwrap();
    sqlx::query("UPDATE profiles SET body=json_set(body,'$.status','archived') WHERE id='p'")
        .execute(&f.store.pool)
        .await
        .unwrap();
    let rev: i64 = sqlx::query_scalar("SELECT revision FROM profiles WHERE id='p'")
        .fetch_one(&f.store.pool)
        .await
        .unwrap();
    let pool = f.store.pool.clone();
    let app = Application::new(f.store);
    let mut req = request(
        "profile.delete.permanent",
        json!({"confirm":true}),
        Some("p"),
    );
    req["expectedRevision"] = json!(rev);
    let deleted = app.dispatch(req).await.response;
    assert_eq!(deleted["ok"], true, "{deleted}");
    for t in [
        "experimental_self_records",
        "experimental_self_reviews",
        "experimental_private_reflections",
        "experimental_reflection_runs",
        "experimental_decisions",
        "experimental_instances",
    ] {
        assert_eq!(count(&pool, t).await, 0, "{t}");
    }
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM experimental_instances WHERE profile_id='other'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 1);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn changed_private_context_during_generation_rejects_late_output() {
    let f = fixture().await;
    approve(&f, "own_note", "Initial").await;
    f.generator.hold.store(true, Ordering::SeqCst);
    f.primary
        .choices
        .lock()
        .unwrap()
        .push_back(DecisionChoice::Reflect(ReflectionRequest::default()));
    f.clock.advance(1000);
    let rt = f.runtime.clone();
    let task = tokio::spawn(async move { rt.run_due("p", "exp", &[]).await });
    while f.generator.calls.load(Ordering::SeqCst) < 2 {
        tokio::task::yield_now().await;
    }
    sqlx::query("UPDATE experimental_self_records SET revision=revision+1,body=json_set(body,'$.revision',revision+1) WHERE profile_id='p'").execute(&f.store.pool).await.unwrap();
    f.generator.release.notify_one();
    assert_eq!(task.await.unwrap().unwrap_err(), "REVISION_CONFLICT");
    assert_eq!(
        count(&f.store.pool, "experimental_private_reflections").await,
        1
    );
    assert_eq!(
        rows(&f.store.pool, "experimental_reflection_runs")
            .await
            .last()
            .unwrap()["status"],
        "failed"
    );
}
#[tokio::test]
async fn private_corruption_is_detected_before_the_next_primary_decision() {
    let f = fixture().await;
    let r = generate(&f, "reflection", "own_note", "Intact").await;
    sqlx::query("UPDATE experimental_private_reflections SET body=json_set(body,'$.summary','tampered') WHERE id=?").bind(r["id"].as_str().unwrap()).execute(&f.store.pool).await.unwrap();
    let before = f.primary.packets.lock().unwrap().len();
    assert_eq!(
        tick(&f, DecisionChoice::NoAction).await.unwrap_err(),
        "STORAGE_ERROR"
    );
    assert_eq!(f.primary.packets.lock().unwrap().len(), before);
}

#[tokio::test]
async fn more_than_one_hundred_pending_candidates_do_not_disable_the_runtime() {
    let f = fixture().await;
    let template = generate(&f, "reflection", "value", "Candidate").await;
    // Seed valid independent fixture rows: this regression targets selection size,
    // not 105 repetitions of the already-tested generator protocol.
    for _ in 0..104 {
        let mut record = template.clone();
        record["id"] = json!(uuid::Uuid::new_v4().to_string());
        sqlx::query("INSERT INTO experimental_private_reflections(profile_id,instance_id,run_id,id,revision,body) VALUES('p','exp',?,?,1,?)")
            .bind(record["runId"].as_str().unwrap()).bind(record["id"].as_str().unwrap()).bind(record.to_string()).execute(&f.store.pool).await.unwrap();
    }
    tick(&f, DecisionChoice::NoAction).await.unwrap();
    let packets = f.primary.packets.lock().unwrap();
    let context = &packets.last().unwrap().private_context;
    assert!(context.to_string().len() <= 4096);
    assert!(context["omittedCount"].as_i64().unwrap() > 5);
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
}

#[tokio::test]
async fn audit_f1_unlimited_modified_note_5000_is_rejected_atomically_and_candidate_can_be_rejected(
) {
    let f = fixture().await;
    sqlx::query("UPDATE profiles SET body=json_set(body,'$.memorySettings.longTerm',json('{\"kind\":\"unlimited\"}')) WHERE id='p'").execute(&f.store.pool).await.unwrap();
    let r = generate(&f, "reflection", "own_note", "Small visible candidate").await;
    assert_eq!(
        tick(&f, review(&r, "modify", Some(&"x".repeat(5000))))
            .await
            .unwrap_err(),
        "CONTEXT_BUDGET_EXCEEDED"
    );
    assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
    assert_eq!(count(&f.store.pool, "experimental_self_reviews").await, 0);
    assert_eq!(count(&f.store.pool, "experimental_decisions").await, 1);
    f.clock.advance(60000); // technical backoff, no automatic retry or generator call
    tick(&f, review(&r, "reject", None)).await.unwrap();
    assert_eq!(
        rows(&f.store.pool, "experimental_self_reviews").await[0]["disposition"],
        "reject"
    );
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn audit_f1_oversized_pending_candidate_is_never_persisted_but_small_one_remains_rejectable()
{
    for text in ["x".repeat(5000), "\n".repeat(1800) + "x"] {
        let f = fixture().await;
        f.generator
            .outputs
            .lock()
            .unwrap()
            .push_back(draft("own_note", &text));
        let mut request = ReflectionRequest::default();
        request.output_bytes = 8192;
        assert_eq!(
            tick(&f, DecisionChoice::Dream(request)).await.unwrap_err(),
            "CONTEXT_BUDGET_EXCEEDED"
        );
        assert_eq!(
            count(&f.store.pool, "experimental_private_reflections").await,
            0
        );
        assert_eq!(count(&f.store.pool, "experimental_self_reviews").await, 0);
        assert_eq!(
            rows(&f.store.pool, "experimental_reflection_runs").await[0]["status"],
            "failed"
        );
        f.clock.advance(60000);
        let r = generate(&f, "dream", "own_note", "Reviewable replacement candidate").await;
        tick(&f, review(&r, "reject", None)).await.unwrap();
        assert_eq!(count(&f.store.pool, "experimental_self_records").await, 0);
    }
}

#[tokio::test]
async fn audit_f1_reviewable_unlimited_note_remains_visible_and_withdrawable() {
    let f = fixture().await;
    sqlx::query("UPDATE profiles SET body=json_set(body,'$.memorySettings.longTerm',json('{\"kind\":\"unlimited\"}')) WHERE id='p'").execute(&f.store.pool).await.unwrap();
    let r = generate(&f, "reflection", "own_note", "Small candidate").await;
    tick(&f, review(&r, "modify", Some(&"x".repeat(2000))))
        .await
        .unwrap();
    let note = rows(&f.store.pool, "experimental_self_records").await[0].clone();
    tick(
        &f,
        DecisionChoice::WithdrawSelf(SelfWithdrawal {
            record_id: note["id"].as_str().unwrap().into(),
            expected_revision: 1,
            reason: "Reconsidered after reviewing the full note".into(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        rows(&f.store.pool, "experimental_self_records").await[0]["status"],
        "withdrawn"
    );
    assert!(f
        .primary
        .packets
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .private_context["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == note["id"]));
}

#[tokio::test]
async fn audit_f2_provenance_overflow_is_rejected_before_generator_without_truncation() {
    let f = fixture().await;
    let service =
        ai_home::experimental_continuity::WorkContinuityService::new(f.store.pool.clone());
    let ep = service
        .create_episode("p", "exp", "Many original sources", "", f.clock.now())
        .await
        .unwrap();
    let refs:Vec<Value>=(0..33).map(|n|json!({"sourceKind":"tool_result","sourceId":format!("source-{n}"),"sourceRevision":1,"sha256":"a".repeat(64),"availability":"available"})).collect();
    service
        .write_technical_checkpoint(
            "p",
            "exp",
            ep["id"].as_str().unwrap(),
            "",
            "active",
            0,
            &[],
            &[],
            &refs,
            f.clock.now(),
        )
        .await
        .unwrap();
    assert_eq!(
        tick(&f, DecisionChoice::Reflect(ReflectionRequest::default()))
            .await
            .unwrap_err(),
        "CONTEXT_BUDGET_EXCEEDED"
    );
    assert_eq!(f.generator.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        count(&f.store.pool, "experimental_reflection_runs").await,
        0
    );
    assert_eq!(
        count(&f.store.pool, "experimental_private_reflections").await,
        0
    );
}
