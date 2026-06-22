use crate::{downloader::VideoInfo, error::Result, orchestrator::Orchestrator, queue::task::DownloadTask};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

#[derive(serde::Deserialize)]
pub struct StartDownloadRequest {
    pub url: String,
    pub format: String,
    pub quality: String,
    pub container: String,
    pub fps: Option<u32>,
    pub bitrate: Option<u32>,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub is_playlist: bool,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
}

#[tauri::command]
pub async fn fetch_info(
    url: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<VideoInfo> {
    orchestrator.lock().await.fetch_info(url).await
}

#[tauri::command]
pub async fn start_download(
    req: StartDownloadRequest,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<DownloadTask> {
    orchestrator
        .lock()
        .await
        .enqueue(req.url, req.format, req.quality, req.container, req.fps, req.bitrate, req.title, req.thumbnail, req.channel, req.duration, req.is_playlist, req.audio_codec, req.video_codec)
        .await
}

#[tauri::command]
pub async fn cancel_download(
    task_id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    orchestrator.lock().await.cancel(&task_id).await
}
