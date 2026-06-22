use crate::{db::channel_prefs::ChannelPrefs, error::Result, orchestrator::OrchestratorHandle};
use tauri::State;

#[tauri::command]
pub async fn list_channel_prefs(
    orchestrator: State<'_, OrchestratorHandle>,
) -> Result<Vec<ChannelPrefs>> {
    let orch = orchestrator.lock().await;
    orch.db.list_channel_prefs().await
}

#[tauri::command]
pub async fn upsert_channel_prefs(
    prefs: ChannelPrefs,
    orchestrator: State<'_, OrchestratorHandle>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.upsert_channel_prefs(&prefs).await
}

#[tauri::command]
pub async fn delete_channel_prefs(
    channel_id: String,
    orchestrator: State<'_, OrchestratorHandle>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.delete_channel_prefs(&channel_id).await
}
