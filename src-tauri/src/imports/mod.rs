//! Local user-selected files. No caller-supplied paths, network or model calls.
mod parser;
pub(crate) use parser::{parse, ParsedDocument};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::Future,
    io::Read,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
pub const MAX_FILE_BYTES: usize = 15 * 1024 * 1024;
pub const MAX_BATCH_BYTES: usize = 30 * 1024 * 1024;
pub const MAX_FILES: usize = 12;
const MAX_SELECTION_BYTES: usize = 60 * 1024 * 1024;
const TTL: Duration = Duration::from_secs(300);
pub trait FilePicker: Send + Sync {
    fn pick(&self) -> Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, ()>> + Send + '_>>;
}
#[cfg(feature = "desktop")]
pub struct DesktopPicker(pub tauri::AppHandle);
#[cfg(feature = "desktop")]
impl FilePicker for DesktopPicker {
    fn pick(&self) -> Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, ()>> + Send + '_>> {
        use tauri_plugin_dialog::DialogExt;
        let app = self.0.clone();
        Box::pin(async move {
            tauri::async_runtime::spawn_blocking(move || {
                app.dialog()
                    .file()
                    .add_filter("TXT / MD / DOCX / PDF", &["txt", "md", "docx", "pdf"])
                    .blocking_pick_files()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.into_path().map_err(|_| ()))
                    .collect()
            })
            .await
            .map_err(|_| ())?
        })
    }
}
#[derive(Clone)]
pub(crate) struct SelectedFile {
    pub name: String,
    pub bytes: Arc<Vec<u8>>,
    pub sha256: String,
}
#[derive(Clone)]
pub(crate) struct Selection {
    pub profile: String,
    pub conversation: String,
    pub files: Vec<SelectedFile>,
    created: Instant,
}
#[derive(Clone)]
pub(crate) struct ImportService {
    picker: Option<Arc<dyn FilePicker>>,
    selections: Arc<Mutex<HashMap<String, Selection>>>,
    pub worker: Arc<Semaphore>,
}
pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl ImportService {
    pub fn new(picker: Option<Arc<dyn FilePicker>>) -> Self {
        Self {
            picker,
            selections: Arc::new(Mutex::new(HashMap::new())),
            worker: Arc::new(Semaphore::new(1)),
        }
    }
    pub async fn select(&self, profile: &str, conversation: &str) -> Result<Value, &'static str> {
        let picker = self.picker.as_ref().ok_or("MODE_UNAVAILABLE")?;
        let paths = picker.pick().await.map_err(|_| "IMPORT_UNREADABLE")?;
        if paths.len() > MAX_FILES {
            return Err("IMPORT_BATCH_TOO_LARGE");
        }
        if paths.is_empty() {
            return Ok(json!({"cancelled":true,"selectionToken":"","items":[]}));
        }
        let (files, items) = tokio::task::spawn_blocking(move || {
            let mut files = vec![];
            let mut items = vec![];
            let mut total = 0usize;
            for path in paths {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Document")
                    .to_string();
                let read = (|| -> Result<SelectedFile, &'static str> {
                    if name.len() > 512 || name.chars().any(char::is_control) {
                        return Err("IMPORT_UNREADABLE");
                    }
                    let lower = name.to_lowercase();
                    if ![".txt", ".md", ".docx", ".pdf"]
                        .iter()
                        .any(|ext| lower.ends_with(ext))
                    {
                        return Err("IMPORT_UNREADABLE");
                    }
                    let file = std::fs::File::open(&path).map_err(|_| "IMPORT_UNREADABLE")?;
                    let meta = file.metadata().map_err(|_| "IMPORT_UNREADABLE")?;
                    if !meta.is_file() || meta.len() == 0 {
                        return Err("IMPORT_UNREADABLE");
                    }
                    if meta.len() > MAX_FILE_BYTES as u64 {
                        return Err("IMPORT_TOO_LARGE");
                    }
                    let mut bytes = Vec::new();
                    file.take(MAX_FILE_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| "IMPORT_UNREADABLE")?;
                    if bytes.is_empty() {
                        return Err("IMPORT_UNREADABLE");
                    }
                    if bytes.len() > MAX_FILE_BYTES {
                        return Err("IMPORT_TOO_LARGE");
                    }
                    total = total.checked_add(bytes.len()).ok_or("IMPORT_TOO_LARGE")?;
                    if total > MAX_BATCH_BYTES {
                        return Err("IMPORT_BATCH_TOO_LARGE");
                    }
                    Ok(SelectedFile {
                        name: name.clone(),
                        sha256: digest(&bytes),
                        bytes: Arc::new(bytes),
                    })
                })();
                match read {
                    Ok(f) => {
                        items.push(
                            json!({"fileName":name,"accepted":true,"sizeBytes":f.bytes.len()}),
                        );
                        files.push(f);
                    }
                    Err("IMPORT_BATCH_TOO_LARGE") => return Err("IMPORT_BATCH_TOO_LARGE"),
                    Err(code) => {
                        items.push(json!({"fileName":name,"accepted":false,"errorCode":code}))
                    }
                }
            }
            Ok((files, items))
        })
        .await
        .map_err(|_| "IMPORT_UNREADABLE")??;
        if files.is_empty() {
            return Ok(json!({"cancelled":false,"selectionToken":"","items":items}));
        }
        let mut snapshots = self.selections.lock().await;
        snapshots.retain(|_, s| s.created.elapsed() < TTL);
        let used: usize = snapshots
            .values()
            .flat_map(|s| s.files.iter())
            .map(|f| f.bytes.len())
            .sum();
        let size: usize = files.iter().map(|f| f.bytes.len()).sum();
        if snapshots.len() >= 4 || used + size > MAX_SELECTION_BYTES {
            return Err("IMPORT_TOO_LARGE");
        }
        let token = uuid::Uuid::new_v4().to_string();
        snapshots.insert(
            token.clone(),
            Selection {
                profile: profile.into(),
                conversation: conversation.into(),
                files,
                created: Instant::now(),
            },
        );
        Ok(json!({"cancelled":false,"selectionToken":token,"items":items}))
    }
    pub async fn snapshot(
        &self,
        token: &str,
        profile: &str,
        conversation: &str,
    ) -> Result<Selection, &'static str> {
        let mut snapshots = self.selections.lock().await;
        snapshots.retain(|_, s| s.created.elapsed() < TTL);
        let s = snapshots.get(token).ok_or("NOT_FOUND")?;
        if s.profile != profile || s.conversation != conversation {
            return Err("NOT_FOUND");
        }
        Ok(s.clone())
    }
    pub async fn consume(&self, token: &str) {
        self.selections.lock().await.remove(token);
    }
}
pub(crate) fn parse_error(message: &str) -> &'static str {
    if message.contains("no text layer") {
        "PDF_NO_TEXT"
    } else if message.contains("encrypted") || message.contains("password") {
        "ENCRYPTED_OR_DAMAGED"
    } else if message.contains("limit")
        || message.contains("too many")
        || message.contains("ratio")
        || message.contains("complexity")
    {
        "IMPORT_TOO_LARGE"
    } else if message.contains("macros")
        || message.contains("DTD")
        || message.contains("entity")
        || message.contains("unsafe package")
    {
        "UNSAFE_DOCUMENT"
    } else {
        "IMPORT_UNREADABLE"
    }
}
