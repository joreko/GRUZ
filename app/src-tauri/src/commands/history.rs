use crate::error::AppError;
use crate::{db::history::HistoryItem, error::Result, orchestrator::Orchestrator};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

fn validate_path(path: &str, is_file: bool) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(AppError::Validation(format!("Файл не найден: {path}")));
    }
    if is_file && !p.is_file() {
        return Err(AppError::Validation(format!(
            "Путь не является файлом: {path}"
        )));
    }
    if !is_file && !p.is_dir() {
        return Err(AppError::Validation(format!(
            "Путь не является папкой: {path}"
        )));
    }
    // Запрещаем открывать исполняемые файлы
    if is_file {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(
            ext.to_ascii_lowercase().as_str(),
            "exe" | "bat" | "cmd" | "com" | "ps1"
        ) {
            return Err(AppError::Validation(format!(
                "Запрещено открывать исполняемые файлы: {path}"
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_history(
    limit: Option<i64>,
    offset: Option<i64>,
    query: Option<String>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<Vec<HistoryItem>> {
    let orch = orchestrator.lock().await;
    let lim = limit.unwrap_or(50);
    let off = offset.unwrap_or(0);
    match query.as_deref().filter(|q| !q.is_empty()) {
        Some(q) => orch.db.search_history(q, lim, off).await,
        None => orch.db.get_history(lim, off).await,
    }
}

#[tauri::command]
pub async fn delete_history_item(
    id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.delete_history_item(&id).await?;
    orch.thought_history_delete();
    Ok(())
}

#[tauri::command]
pub async fn clear_history(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.clear_history().await?;
    orch.thought_history_clear();
    Ok(())
}

#[tauri::command]
pub async fn open_file(
    path: String,
    app: AppHandle,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    validate_path(&path, true)?;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
    orchestrator.lock().await.thought_open_file();
    Ok(())
}

#[tauri::command]
pub async fn open_folder(
    path: String,
    app: AppHandle,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let folder = std::path::Path::new(&path)
        .parent()
        .unwrap_or(std::path::Path::new(&path));
    let folder_str = folder.to_string_lossy().to_string();
    validate_path(&folder_str, false)?;
    app.opener()
        .open_path(&folder_str, None::<&str>)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
    orchestrator.lock().await.thought_open_folder();
    Ok(())
}
