//! Tauri owns one runtime. Both ordinary and Community calls use that owner.
use crate::community_host::desktop::{local_caller, DesktopRuntime};
use serde_json::Value;
use tauri::{Emitter, Manager};
use zeroize::Zeroizing;

#[tauri::command]
async fn home_dispatch(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DesktopRuntime>,
    request: Value,
) -> Result<Value, String> {
    if !local_caller(
        window.label(),
        window.url().map_err(|_| "POLICY_DENIED")?.as_str(),
    ) {
        return Err("POLICY_DENIED".into());
    }
    let outcome = state.dispatch(request).await.map_err(str::to_owned)?;
    if let Some(event) = outcome.event {
        // State is already committed. Event loss is recovered by app.events.list.
        let _ = app.emit_to("main", "home:app-event", event);
    }
    Ok(outcome.response)
}

#[tauri::command]
async fn community_dispatch(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DesktopRuntime>,
    request: Value,
    invite: Option<String>,
) -> Result<Value, String> {
    // Out of the command envelope and receipts. Never log IPC arguments.
    let invite = invite.map(Zeroizing::new);
    let url = window.url().map_err(|_| "POLICY_DENIED")?;
    state
        .registration(window.label(), url.as_str(), request, invite)
        .await
        .map_err(str::to_owned)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            let runtime = tauri::async_runtime::block_on(DesktopRuntime::open(
                &directory,
                crate::vault::Vault::os(),
                Some(std::sync::Arc::new(crate::imports::DesktopPicker(
                    app.handle().clone(),
                ))),
            ))
            .map_err(std::io::Error::other)?;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![home_dispatch, community_dispatch])
        .run(tauri::generate_context!())
        .expect("AI Home startup failed");
}
