use crate::{db::session::Session, error::Result, orchestrator::OrchestratorHandle};
use tauri::State;

#[tauri::command]
pub async fn get_session(
    orchestrator: State<'_, OrchestratorHandle>,
) -> Result<Session> {
    let orch = orchestrator.lock().await;
    orch.db.get_session().await
}

#[tauri::command]
pub async fn update_session(
    session: Session,
    orchestrator: State<'_, OrchestratorHandle>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.update_session(&session).await
}
