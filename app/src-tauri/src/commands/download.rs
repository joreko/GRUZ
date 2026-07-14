use crate::downloader::{process, VideoInfo};
use crate::error::Result;
use crate::orchestrator::Orchestrator;
use crate::queue::task::DownloadTask;
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
    pub source_fps: Option<u32>,
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
    // Фаза 1: быстро читаем настройки под блокировкой (proxy + ytdlp path + extra_args)
    let (proxy, ytdlp, extra_args) = {
        let orch = orchestrator.lock().await;
        orch.fetch_info_prepare().await
    };
    // Фаза 2: yt-dlp без блокировки оркестратора (может длиться 10+ секунд)
    let result = process::fetch_info(&ytdlp, &url, proxy.as_deref(), Some(&extra_args)).await;
    // Сброс idle-таймера после завершения
    orchestrator.lock().await.reset_idle();
    result
}

#[tauri::command]
pub async fn start_download(
    req: StartDownloadRequest,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<DownloadTask> {
    let orch = Arc::clone(&orchestrator);
    Orchestrator::enqueue(
        orch,
        req.url,
        req.format,
        req.quality,
        req.container,
        req.fps,
        req.source_fps,
        req.bitrate,
        req.title,
        req.thumbnail,
        req.channel,
        req.duration,
        req.is_playlist,
        req.audio_codec,
        req.video_codec,
    )
    .await
}

#[tauri::command]
pub async fn cancel_download(
    task_id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = Arc::clone(&orchestrator);
    Orchestrator::cancel(orch, &task_id).await
}
