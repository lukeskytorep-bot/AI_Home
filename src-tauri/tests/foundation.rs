use ai_home::{application::Application, domain::schema, persistence::Store};
use serde_json::{json, Value};
fn request(id: &str, command: &str, payload: Value) -> Value {
    json!({"contractVersion":1,"requestId":id,"command":command,"payload":payload})
}
fn set(id: &str, revision: i64) -> Value {
    let mut r = request(
        id,
        "app.preferences.set",
        json!({"locale":"pl","themeId":"aurora","themeVariant":"dark"}),
    );
    r["expectedRevision"] = json!(revision);
    r
}
#[test]
fn frozen_fixtures() {
    let f: Vec<Value> =
        serde_json::from_str(include_str!("../../contracts/fixtures.json")).unwrap();
    for x in f {
        assert!(
            schema::dto(x["dto"].as_str().unwrap(), &x["value"]),
            "{}",
            x["name"]
        );
        let mut bad = x["value"].clone();
        bad["unexpected"] = json!(true);
        assert!(!schema::dto(x["dto"].as_str().unwrap(), &bad));
    }
}
#[tokio::test]
async fn default_and_commands() {
    let app = Application::new(Store::memory().await.unwrap());
    let r = app
        .dispatch(request("r", "app.preferences.get", json!({})))
        .await;
    assert_eq!(r.response["result"]["locale"], "en");
    assert!(schema::dto("AppPreferences", &r.response["result"]));
    let r = app
        .dispatch(request("x", "not.implemented", json!({})))
        .await;
    assert_eq!(r.response["error"]["code"], "UNKNOWN_COMMAND");
    let r = app
        .dispatch(request("x", "app.preferences.get", json!({"sql":"secret"})))
        .await;
    assert_eq!(r.response["error"]["code"], "VALIDATION_ERROR");
}
#[tokio::test]
async fn idempotency_and_cas() {
    let app = Application::new(Store::memory().await.unwrap());
    let r = app.dispatch(set("same", 1)).await;
    assert_eq!(r.response["ok"], true);
    assert!(schema::dto("AppEvent", r.event.as_ref().unwrap()));
    let repeated = app.dispatch(set("same", 1)).await;
    assert_eq!(r.response, repeated.response);
    assert!(repeated.event.is_none());
    let mut changed = set("same", 1);
    changed["payload"]["locale"] = json!("en");
    assert_eq!(
        app.dispatch(changed).await.response["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    assert_eq!(
        app.dispatch(set("other", 1)).await.response["error"]["code"],
        "REVISION_CONFLICT"
    );
    let page = app
        .dispatch(request(
            "p",
            "app.events.list",
            json!({"afterSequence":0,"limit":100}),
        ))
        .await;
    assert_eq!(
        page.response["result"]["items"].as_array().unwrap().len(),
        1
    );
}
#[tokio::test]
async fn restart_recovers_preferences_receipts_and_events() {
    let path = std::env::temp_dir().join(format!(
        "ai-home-test-{}-{}.sqlite3",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    let store = Store::open(&path).await.unwrap();
    let pool = store.pool.clone();
    let app = Application::new(store);
    let first = app.dispatch(set("persist", 1)).await.response;
    drop(app);
    pool.close().await;
    let app = Application::new(Store::open(&path).await.unwrap());
    assert_eq!(app.dispatch(set("persist", 1)).await.response, first);
    let history = app
        .dispatch(request(
            "history",
            "app.events.list",
            json!({"afterSequence":0,"limit":100}),
        ))
        .await;
    assert_eq!(
        history.response["result"]["items"][0]["payload"]["revision"],
        2
    );
    drop(app);
    let _ = std::fs::remove_file(path);
}
#[tokio::test]
async fn failed_receipt_rolls_back_preferences_and_event() {
    let store = Store::memory().await.unwrap();
    sqlx::query("CREATE TRIGGER reject_receipt BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT,'private failure'); END").execute(&store.pool).await.unwrap();
    let app = Application::new(store);
    let r = app.dispatch(set("rollback", 1)).await;
    assert_eq!(r.response["error"]["code"], "STORAGE_ERROR");
    assert!(r.event.is_none());
    assert!(!r.response.to_string().contains("private failure"));
    let p = app
        .dispatch(request("read", "app.preferences.get", json!({})))
        .await;
    assert_eq!(p.response["result"]["revision"], 1);
    let history = app
        .dispatch(request(
            "h",
            "app.events.list",
            json!({"afterSequence":0,"limit":100}),
        ))
        .await;
    assert_eq!(history.response["result"]["lastSequence"], 0);
}
#[tokio::test]
async fn concurrent_stale_writes_have_one_winner() {
    let app = Application::new(Store::memory().await.unwrap());
    let (a, b) = tokio::join!(app.dispatch(set("a", 1)), app.dispatch(set("b", 1)));
    assert_ne!(a.response["ok"], b.response["ok"]);
    let loser = if a.response["ok"] == false { a } else { b };
    assert_eq!(loser.response["error"]["code"], "REVISION_CONFLICT");
}
#[tokio::test]
async fn validates_locale_extension_without_execution() {
    let app = Application::new(Store::memory().await.unwrap());
    let mut pack: Value = serde_json::from_str(include_str!("../../src/locales/en.json")).unwrap();
    pack["strings"]
        .as_object_mut()
        .unwrap()
        .remove("action.save");
    let r = app
        .dispatch(request(
            "p",
            "app.pack.validate",
            json!({"kind":"locale","pack":pack}),
        ))
        .await;
    assert!(r.response["result"]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "fallback.en:action.save"));
    pack["strings"]["unknown.key"] = json!("script");
    assert_eq!(
        app.dispatch(request(
            "p2",
            "app.pack.validate",
            json!({"kind":"locale","pack":pack})
        ))
        .await
        .response["error"]["code"],
        "PACK_INVALID"
    );
}

#[tokio::test]
async fn rejects_corrupt_preferences() {
    let store = Store::memory().await.unwrap();
    sqlx::query("UPDATE preferences SET body='not-json' WHERE id=1")
        .execute(&store.pool)
        .await
        .unwrap();
    let app = Application::new(store);
    let r = app
        .dispatch(request("corrupt", "app.preferences.get", json!({})))
        .await;
    assert_eq!(r.response["error"]["code"], "STORAGE_ERROR");
}
