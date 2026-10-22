pub mod application;
pub mod domain;
pub mod kernel;
pub mod persistence;
pub mod providers;
pub mod vault;
pub mod imports;
pub mod backup;
pub mod memory;
pub mod retrieval;
mod connections;
#[cfg(feature="desktop")]
mod desktop {
    use tauri::{Emitter,Manager};
    use serde_json::Value;
    use crate::{application::Application,persistence::Store};
    #[tauri::command]
    pub async fn home_dispatch(app:tauri::AppHandle,state:tauri::State<'_,Application>,request:Value)->Result<Value,String>{
        let outcome=state.dispatch(request).await;
        if let Some(event)=outcome.event {
            // State is already committed. Event loss is recovered by app.events.list.
            let _=app.emit_to("main","home:app-event",event);
        }
        Ok(outcome.response)
    }
    pub fn run(){
        tauri::Builder::default().plugin(tauri_plugin_dialog::init()).setup(|app|{
            let directory=app.path().app_data_dir()?;std::fs::create_dir_all(&directory)?;
            tauri::async_runtime::block_on(crate::backup::apply_pending_restore(&directory))?;
            let store=tauri::async_runtime::block_on(Store::open(&directory.join("foundation.sqlite3")))?;
            app.manage(Application::with_file_picker(store,crate::vault::Vault::os(),std::sync::Arc::new(crate::imports::DesktopPicker(app.handle().clone()))));Ok(())
        }).invoke_handler(tauri::generate_handler![home_dispatch])
          .run(tauri::generate_context!()).expect("AI Home startup failed");
    }
}
#[cfg(feature="desktop")]
pub use desktop::run;
