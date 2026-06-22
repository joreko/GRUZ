use crate::{db::settings::Settings, error::Result, orchestrator::Orchestrator};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn get_settings(
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<Settings> {
    orchestrator.lock().await.db.get_settings().await
}

#[tauri::command]
pub async fn update_settings(
    key: String,
    value: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.update_setting(&key, &value).await?;
    orch.thought_settings();
    drop(orch);
    if key == "max_concurrent" {
        if let Ok(n) = value.parse::<usize>() {
            orchestrator.lock().await.update_max_concurrent(n).await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_free_space(path: String) -> Result<u64> {
    use std::path::Path;
    let p = if path.is_empty() { ".".to_string() } else { path };
    let free = fs4::available_space(Path::new(&p)).unwrap_or(0);
    Ok(free)
}
