//! Native end-to-end regression through the real private runtime and permanent
//! conversation-deletion storage handler. This does not expose Experimental IPC.
use crate::{
    experimental_continuity::WorkContinuityService,
    experimental_reflection::{ReflectionProvider, ReflectionRequest, SelfReview},
    experimental_runtime::{
        Clock, DecisionChoice, DecisionProvider, DisabledActionExecutor, ExperimentalRuntime,
        OpportunityPacket,
    },
    persistence::Store,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
struct Time(Mutex<DateTime<Utc>>);
impl Clock for Time {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}
struct Primary(Mutex<VecDeque<DecisionChoice>>);
impl DecisionProvider for Primary {
    fn decide(&self, _: &OpportunityPacket) -> Result<DecisionChoice, String> {
        self.0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or("no decision".into())
    }
}
struct Generator;
impl ReflectionProvider for Generator {
    fn generate<'a>(
        &'a self,
        _: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>> {
        Box::pin(async {
            Ok(
                json!({"conclusion":"draft","summary":"A hypothesis","candidates":[{"kind":"own_note","content":"PRIVATE_SELF_AUTHORED_NOTE"}]}),
            )
        })
    }
}
async fn rows(store: &Store, table: &str) -> Vec<Value> {
    sqlx::query_scalar::<_, String>(&format!(
        "SELECT body FROM {table} WHERE profile_id='p' ORDER BY rowid"
    ))
    .fetch_all(&store.pool)
    .await
    .unwrap()
    .into_iter()
    .map(|s| serde_json::from_str(&s).unwrap())
    .collect()
}
fn source(id: &str, availability: &str) -> Value {
    json!({"sourceKind":"conversation","sourceId":id,"sourceRevision":1,"sha256":"a".repeat(64),"availability":availability})
}
fn disposition(r: &Value, name: &str) -> DecisionChoice {
    DecisionChoice::ReviewSelf(SelfReview {
        reflection_id: r["id"].as_str().unwrap().into(),
        reflection_revision: r["revision"].as_i64().unwrap(),
        candidate_index: 0,
        disposition: name.into(),
        modified_content: None,
        reason: "Independent Primary decision".into(),
    })
}
fn has_source(row: &Value, id: &str, availability: &str) -> bool {
    row["sourceRefs"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| {
            x["sourceKind"] == "conversation"
                && x["sourceId"] == id
                && x["availability"] == availability
        })
        .count()
        == 1
}
#[tokio::test]
async fn audit_f2_runtime_inherits_checkpoint_note_and_pending_sources_then_real_deletion_tombstones_them(
) {
    let store = Store::memory().await.unwrap();
    let time = Arc::new(Time(Mutex::new(
        DateTime::parse_from_rfc3339("2026-10-26T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )));
    let primary = Arc::new(Primary(Mutex::new(VecDeque::new())));
    let runtime = ExperimentalRuntime::new(
        store.pool.clone(),
        time.clone(),
        primary.clone(),
        Arc::new(DisabledActionExecutor),
    )
    .with_reflection_provider(Arc::new(Generator));
    runtime.create_instance("p", "exp", 1000).await.unwrap();
    let mut tx = store.pool.begin().await.unwrap();
    let c1 = super::execute(
        &mut tx,
        "conversation.create",
        "p",
        &json!({"title":"Original source","language":"en"}),
        0,
    )
    .await
    .unwrap();
    let c2 = super::execute(
        &mut tx,
        "conversation.create",
        "p",
        &json!({"title":"Semantic-only source","language":"en"}),
        0,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let cid = c1["id"].as_str().unwrap();
    let sid = c2["id"].as_str().unwrap();
    let service = WorkContinuityService::new(store.pool.clone());
    let ep = service
        .create_episode("p", "exp", "Reflect on source", "", time.now())
        .await
        .unwrap();
    let eid = ep["id"].as_str().unwrap();
    let tech = service
        .write_technical_checkpoint(
            "p",
            "exp",
            eid,
            "",
            "active",
            0,
            &[],
            &[],
            &[source(cid, "available")],
            time.now(),
        )
        .await
        .unwrap();
    service.write_semantic_checkpoint("p","exp",eid,tech["id"].as_str().unwrap(),"","primary","accepted",json!({"focus":"source","currentIntention":"consider","findings":[],"openQuestions":[],"nextSuggestedStep":"reflect"}),&[source(cid,"available"),source(sid,"available")],time.now()).await.unwrap();
    macro_rules! tick {
        ($choice:expr) => {{
            primary.0.lock().unwrap().push_back($choice);
            {
                let mut now = time.0.lock().unwrap();
                *now = *now + Duration::milliseconds(1000);
            }
            runtime.run_due("p", "exp", &[]).await.unwrap()
        }};
    }
    tick!(DecisionChoice::Dream(ReflectionRequest::default()));
    let first = rows(&store, "experimental_private_reflections").await[0].clone();
    assert!(has_source(&first, cid, "available"));
    assert!(has_source(&first, sid, "available"));
    tick!(disposition(&first, "accept"));
    // Finish the episode: subsequent generations cannot obtain c1/c2 from ResumePacket.
    sqlx::query("UPDATE experimental_work_episodes SET body=json_set(body,'$.status','completed') WHERE id=?").bind(eid).execute(&store.pool).await.unwrap();
    tick!(DecisionChoice::Reflect(ReflectionRequest::default()));
    let second = rows(&store, "experimental_private_reflections").await[1].clone();
    assert!(has_source(&second, cid, "available"));
    assert!(has_source(&second, sid, "available"));
    // Withdraw the approved note. Generation 3 must inherit from pending candidate 2.
    let note = rows(&store, "experimental_self_records").await[0].clone();
    tick!(DecisionChoice::WithdrawSelf(
        crate::experimental_reflection::SelfWithdrawal {
            record_id: note["id"].as_str().unwrap().into(),
            expected_revision: 1,
            reason: "Reconsidered".into()
        }
    ));
    tick!(DecisionChoice::Dream(ReflectionRequest::default()));
    let third = rows(&store, "experimental_private_reflections").await[2].clone();
    assert!(has_source(&third, cid, "available"));
    assert!(has_source(&third, sid, "available"));
    tick!(disposition(&third, "accept"));
    let before = rows(&store, "experimental_self_records").await;
    let mut tx = store.pool.begin().await.unwrap();
    let archived = super::execute(
        &mut tx,
        "conversation.archive",
        "p",
        &json!({"conversationId":cid,"archived":true}),
        1,
    )
    .await
    .unwrap();
    let deleted = super::execute(
        &mut tx,
        "conversation.delete.permanent",
        "p",
        &json!({"conversationId":cid,"confirm":true}),
        archived["revision"].as_i64().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(deleted["deleted"], true);
    tx.commit().await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM conversations WHERE id=?")
        .bind(cid)
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    for table in [
        "experimental_private_reflections",
        "experimental_self_records",
    ] {
        for r in rows(&store, table).await {
            assert!(has_source(&r, cid, "permanently_deleted"), "{r}");
            assert!(has_source(&r, sid, "available"));
            assert!(crate::domain::schema::dto(
                if table.ends_with("reflections") {
                    "PrivateReflection"
                } else {
                    "PrivateSelfRecord"
                },
                &r
            ));
        }
    }
    let after = rows(&store, "experimental_self_records").await;
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a["content"], b["content"]);
        assert_eq!(a["contentSha256"], b["contentSha256"]);
        assert_eq!(a["status"], b["status"]);
    }
    // A new generation inherits the tombstone as well, never resurrecting c1.
    tick!(DecisionChoice::Reflect(ReflectionRequest::default()));
    let newest = rows(&store, "experimental_private_reflections")
        .await
        .pop()
        .unwrap();
    assert!(has_source(&newest, cid, "permanently_deleted"));
    assert!(has_source(&newest, sid, "available"));
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&store.pool)
        .await
        .unwrap()
        .is_empty());
}
