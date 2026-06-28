use crate::error::AppError;
use crate::{db::settings::Settings, error::Result, orchestrator::Orchestrator};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use tracing::warn;

#[tauri::command]
pub async fn get_settings(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<Settings> {
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
        match value.parse::<usize>() {
            Ok(n) => {
                orchestrator.lock().await.update_max_concurrent(n).await;
            }
            Err(_) => {
                warn!("max_concurrent: нечисловое значение '{value}'");
                return Err(AppError::Validation(
                    "max_concurrent должно быть числом".into(),
                ));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_free_space(path: String) -> Result<Option<u64>> {
    use std::path::Path;
    let p = if path.is_empty() { "." } else { &path };
    match fs4::available_space(Path::new(p)) {
        Ok(free) => Ok(Some(free)),
        Err(e) => {
            warn!("get_free_space: не удалось получить размер диска: {e}");
            Ok(None)
        }
    }
}
