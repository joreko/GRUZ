use crate::queue::task::{DownloadTask, Priority};
use crate::{error::Result, orchestrator::Orchestrator};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[tauri::command]
pub async fn get_queue(
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<Vec<DownloadTask>> {
    Ok(orchestrator.lock().await.get_queue().await)
}

#[tauri::command]
pub async fn remove_task(
    task_id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = Arc::clone(&orchestrator);
    Orchestrator::remove_task(orch, &task_id).await?;
    orchestrator.lock().await.thought_remove();
    Ok(())
}

#[tauri::command]
pub async fn clear_queue(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<()> {
    orchestrator.lock().await.clear_queue().await
}

#[tauri::command]
pub async fn set_task_priority(
    task_id: String,
    priority: Priority,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    orchestrator
        .lock()
        .await
        .set_priority(&task_id, priority)
        .await
}

#[tauri::command]
pub async fn reorder_task(
    task_id: String,
    new_index: usize,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    orchestrator
        .lock()
        .await
        .reorder_task(&task_id, new_index)
        .await
}

#[tauri::command]
pub async fn retry_task(
    task_id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = Arc::clone(&orchestrator);
    Orchestrator::retry_task(orch, &task_id).await
}
