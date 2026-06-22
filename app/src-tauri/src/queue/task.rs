use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Fetching,   // получаем метаданные
    Waiting,    // в очереди ожидания воркера
    Scheduled,  // ждёт schedule_at
    Downloading,
    Converting,  // ffmpeg постпроцессинг после загрузки
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub url: String,
    pub video_id: Option<String>,
    pub channel_id: Option<String>,
    pub platform: String,             // 'youtube' | 'vk' | 'tiktok' | ...
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<i64>,
    pub is_playlist: bool,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub format: String,               // "video" | "audio"
    pub quality: String,              // format_id из yt-dlp
    pub fps: Option<u32>,             // ограничение fps (None = оригинал)
    pub bitrate: Option<u32>,         // ограничение битрейта kbps (None = максимум)
    pub container: String,            // "mp4", "webm", "mp3"
    pub trim_start: Option<i64>,      // секунды | None
    pub trim_end: Option<i64>,        // секунды | None
    pub state: TaskState,
    pub priority: Priority,
    pub progress: f32,                // 0.0 - 100.0
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub error: Option<String>,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl DownloadTask {
    pub fn new(url: String, format: String, quality: String, container: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            video_id: None,
            channel_id: None,
            platform: String::new(),
            title: None,
            thumbnail: None,
            channel: None,
            duration: None,
            is_playlist: false,
            audio_codec: None,
            video_codec: None,
            format,
            quality,
            fps: None,
            bitrate: None,
            container,
            trim_start: None,
            trim_end: None,
            state: TaskState::Queued,
            priority: Priority::Normal,
            progress: 0.0,
            speed: None,
            eta: None,
            error: None,
            file_path: None,
            file_size: None,
            created_at: Utc::now(),
        }
    }
}
