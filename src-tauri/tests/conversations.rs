use ai_home::{application::Application, domain::schema, persistence::Store};
use serde_json::{json, Value};
use sqlx::Row;
fn request(command: &str, payload: Value, profile: Option<&str>, revision: Option<i64>) -> Value {
    let mut r = json!({"contractVersion":1,"requestId":uuid::Uuid::new_v4().to_string(),"command":command,"payload":payload});
    if let Some(p) = profile {
        r["profileId"] = json!(p)
    }
    if let Some(v) = revision {
        r["expectedRevision"] = json!(v)
    }
    r
}
async fn call(
    app: &Application,
    command: &str,
    payload: Value,
    p: Option<&str>,
    rev: Option<i64>,
) -> Value {
    app.dispatch(request(command, payload, p, rev))
        .await
        .response
}
async fn profile(app: &Application) -> String {
    let r = call(
        app,
        "profile.create",
        json!({"displayName":"Same name"}),
        None,
        None,
    )
    .await;
    assert_eq!(r["ok"], true);
    r["result"]["id"].as_str().unwrap().into()
}
async fn create(app: &Application, p: &str, title: &str) -> Value {
    let r = call(
        app,
        "conversation.create",
        json!({"title":title,"language":"pl"}),
        Some(p),
        None,
    )
    .await;
    assert_eq!(r["ok"], true, "{r}");
    r["result"].clone()
}
async fn setup() -> (Application, sqlx::SqlitePool, String) {
    let s = Store::memory().await.unwrap();
    let pool = s.pool.clone();
    let app = Application::new(s);
    let p = profile(&app).await;
    (app, pool, p)
}
#[tokio::test]
async fn separate_profiles_have_distinct_conversations_and_drafts() {
    let (app, _, p) = setup().await;
    let q = profile(&app).await;
    let c = create(&app, &p, "One").await;
    let d = create(&app, &q, "One").await;
    assert_ne!(c["id"], d["id"]);
    assert!(schema::dto("Conversation", &c));
    let r = call(
        &app,
        "conversation.get",
        json!({"conversationId":c["id"]}),
        Some(&q),
        None,
    )
    .await;
    assert_eq!(r["error"]["code"], "NOT_FOUND");
    let page = call(
        &app,
        "conversation.list",
        json!({"page":{"limit":50}}),
        Some(&q),
        None,
    )
    .await;
    assert_eq!(page["result"]["items"], json!([d]));
}
#[tokio::test]
async fn exact_unicode_draft_and_cas_are_preserved() {
    let (app, _, p) = setup().await;
    let c = create(&app, &p, "Tytuł").await;
    let payload = json!({"conversationId":c["id"],"text":"  Żółć 🪐\n<think>literal</think>  "});
    let r = call(
        &app,
        "conversation.draft.set",
        payload.clone(),
        Some(&p),
        Some(1),
    )
    .await;
    assert_eq!(r["ok"], true);
    assert_eq!(r["result"]["draftText"], payload["text"]);
    let stale = call(
        &app,
        "conversation.draft.set",
        json!({"conversationId":c["id"],"text":"lost"}),
        Some(&p),
        Some(1),
    )
    .await;
    assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
}
#[tokio::test]
async fn replay_and_changed_payload_do_not_duplicate_conversations() {
    let (app, pool, p) = setup().await;
    let r = request(
        "conversation.create",
        json!({"title":"One","language":"en"}),
        Some(&p),
        None,
    );
    let first = app.dispatch(r.clone()).await.response;
    assert_eq!(app.dispatch(r.clone()).await.response, first);
    let mut other = r;
    other["payload"]["title"] = json!("Other");
    assert_eq!(
        app.dispatch(other).await.response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM conversations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn missing_scope_revision_unknown_fields_and_language_fail_closed() {
    let (app, _, p) = setup().await;
    let c = create(&app, &p, "One").await;
    for (command, payload, scope, expected) in [
        (
            "conversation.create",
            json!({"title":"One","language":"en"}),
            None,
            "PROFILE_SCOPE_MISMATCH",
        ),
        (
            "conversation.create",
            json!({"title":"One","language":"INVALID"}),
            Some(p.as_str()),
            "VALIDATION_ERROR",
        ),
        (
            "conversation.draft.set",
            json!({"conversationId":c["id"],"text":"x"}),
            Some(p.as_str()),
            "VALIDATION_ERROR",
        ),
        (
            "conversation.get",
            json!({"conversationId":c["id"],"extra":true}),
            Some(p.as_str()),
            "VALIDATION_ERROR",
        ),
    ] {
        assert_eq!(
            call(&app, command, payload, scope, None).await["error"]["code"],
            expected
        );
    }
}
#[tokio::test]
async fn receipt_failure_rolls_back_draft_and_event() {
    let (app, pool, p) = setup().await;
    let c = create(&app, &p, "One").await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_draft BEFORE INSERT ON receipts WHEN NEW.request_id='fault' BEGIN SELECT RAISE(ABORT,'fault'); END").execute(&pool).await.unwrap();
    let mut r = request(
        "conversation.draft.set",
        json!({"conversationId":c["id"],"text":"new"}),
        Some(&p),
        Some(1),
    );
    r["requestId"] = json!("fault");
    assert_eq!(
        app.dispatch(r).await.response["error"]["code"],
        "STORAGE_ERROR"
    );
    assert_eq!(
        call(
            &app,
            "conversation.get",
            json!({"conversationId":c["id"]}),
            Some(&p),
            None
        )
        .await["result"],
        c
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM profile_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
}
#[tokio::test]
async fn list_cursor_is_profile_kind_and_limit_bound() {
    let (app, _, p) = setup().await;
    let q = profile(&app).await;
    for _ in 0..3 {
        create(&app, &p, "One").await;
    }
    let first = call(
        &app,
        "conversation.list",
        json!({"page":{"limit":1}}),
        Some(&p),
        None,
    )
    .await;
    let cursor = first["result"]["nextCursor"].clone();
    let next = call(
        &app,
        "conversation.list",
        json!({"page":{"limit":1,"cursor":cursor}}),
        Some(&p),
        None,
    )
    .await;
    assert_ne!(
        next["result"]["items"][0]["id"],
        first["result"]["items"][0]["id"]
    );
    for (scope, limit) in [(&q, 1), (&p, 2)] {
        assert_eq!(
            call(
                &app,
                "conversation.list",
                json!({"page":{"limit":limit,"cursor":cursor}}),
                Some(scope),
                None
            )
            .await["error"]["code"],
            "VALIDATION_ERROR"
        );
    }
}
#[tokio::test]
async fn message_history_is_ordered_complete_and_scoped() {
    let (app, pool, p) = setup().await;
    let c = create(&app, &p, "One").await;
    let d = create(&app, &p, "Two").await;
    for seq in [3, 1, 2] {
        let m = json!({"schemaVersion":1,"id":format!("m{seq}"),"profileId":p,"revision":1,"createdAt":"2026-10-12T12:00:00Z","conversationId":c["id"],"sequence":seq,"role":"human","content":format!("Original {seq}"),"language":"en","status":"complete","sourceRefs":[]});
        sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,?,?,?,1,?)").bind(&p).bind(m["id"].as_str().unwrap()).bind(c["id"].as_str().unwrap()).bind(seq).bind(m.to_string()).execute(&pool).await.unwrap();
    }
    let first = call(
        &app,
        "conversation.messages.list",
        json!({"conversationId":c["id"],"page":{"limit":2}}),
        Some(&p),
        None,
    )
    .await;
    assert_eq!(first["result"]["items"][0]["sequence"], 1);
    assert_eq!(first["result"]["items"][1]["sequence"], 2);
    let cursor = first["result"]["nextCursor"].clone();
    let last = call(
        &app,
        "conversation.messages.list",
        json!({"conversationId":c["id"],"page":{"limit":2,"cursor":cursor}}),
        Some(&p),
        None,
    )
    .await;
    assert_eq!(last["result"]["items"][0]["content"], "Original 3");
    assert_eq!(last["result"]["nextCursor"], "");
    assert_eq!(
        call(
            &app,
            "conversation.messages.list",
            json!({"conversationId":d["id"],"page":{"limit":2,"cursor":cursor}}),
            Some(&p),
            None
        )
        .await["error"]["code"],
        "VALIDATION_ERROR"
    );
}
#[tokio::test]
async fn restart_keeps_preferences_receipts_and_drafts() {
    let path = std::env::temp_dir().join(format!("ai-home-conv-{}.db", uuid::Uuid::new_v4()));
    let s = Store::open(&path).await.unwrap();
    let pool = s.pool.clone();
    let app = Application::new(s);
    let p = profile(&app).await;
    let c = create(&app, &p, "Persist").await;
    let req = request(
        "conversation.draft.set",
        json!({"conversationId":c["id"],"text":"Permanent"}),
        Some(&p),
        Some(1),
    );
    let saved = app.dispatch(req.clone()).await.response;
    let prefs = call(&app, "app.preferences.get", json!({}), None, None).await["result"].clone();
    drop(app);
    pool.close().await;
    let s = Store::open(&path).await.unwrap();
    let pool = s.pool.clone();
    let app = Application::new(s);
    assert_eq!(app.dispatch(req).await.response, saved);
    assert_eq!(
        call(&app, "app.preferences.get", json!({}), None, None).await["result"],
        prefs
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        6
    );
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        sqlx::query("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get::<&str, _>(0),
        "ok"
    );
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn actual_v3_database_without_conversation_tables_migrates() {
    let path = std::env::temp_dir().join(format!("ai-home-v3-{}.db", uuid::Uuid::new_v4()));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let p = profile(&app).await;
    let before = call(&app, "profile.snapshot", json!({}), Some(&p), None).await["result"].clone();
    for statement in [
        "DROP TABLE messages",
        "DROP TABLE conversations",
        "DROP TABLE conversation_cursors",
        "PRAGMA user_version=3",
    ] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    drop(app);
    pool.close().await;
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    assert_eq!(
        call(&app, "profile.snapshot", json!({}), Some(&p), None).await["result"],
        before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap(),
        6
    );
    let c = create(&app, &p, "After migration").await;
    assert_eq!(c["draftText"], "");
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .is_empty());
    drop(app);
    pool.close().await;
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn foreign_conversation_message_fk_and_corrupt_record_fail_closed() {
    let (app, pool, p) = setup().await;
    let q = profile(&app).await;
    let c = create(&app, &p, "Own").await;
    assert!(sqlx::query("INSERT INTO messages(profile_id,id,conversation_id,sequence,revision,body) VALUES(?,?,?,1,1,'{}')").bind(&q).bind("foreign").bind(c["id"].as_str().unwrap()).execute(&pool).await.is_err());
    sqlx::query("UPDATE conversations SET body='{}' WHERE profile_id=? AND id=?")
        .bind(&p)
        .bind(c["id"].as_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            "conversation.get",
            json!({"conversationId":c["id"]}),
            Some(&p),
            None
        )
        .await["error"]["code"],
        "STORAGE_ERROR"
    );
}
