pub mod process;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub channel: Option<String>,
    pub channel_avatar: Option<String>,
    pub channel_followers: Option<i64>,
    pub uploader_url: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<i64>,
    pub formats: Vec<VideoFormat>,
    pub is_playlist: bool,
    pub playlist_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFormat {
    pub format_id: String,
    pub ext: String,
    pub resolution: Option<String>, // "1920x1080"
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub abr: Option<f64>, // audio bitrate kbps
    pub vbr: Option<f64>, // video bitrate kbps
    pub filesize: Option<i64>,
    pub format_note: Option<String>, // "1080p", "720p60", "DASH audio"
    pub is_audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub task_id: String,
    pub state: String,
    pub progress: f32,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub downloaded_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
}
