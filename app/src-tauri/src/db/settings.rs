use crate::{db::Database, error::Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_dir: String,
    pub max_concurrent: u32,
    pub default_format: String,
    pub default_quality: String,
    pub default_container: String,
    pub default_fps: Option<u32>,
    pub default_bitrate: Option<u32>,
    pub default_video_codec: Option<String>,
    pub default_audio_codec: Option<String>,
    pub auto_merge: bool,
    pub embed_subtitles: bool,
    pub proxy: String,
    pub ytdlp_extra_args: String,
    pub theme: String,
    pub minimize_to_tray: bool,
    // Куда сохранять (пусто = download_dir)
    pub save_dir_video: String,
    pub save_dir_audio: String,
    pub save_dir_playlist: String,
    pub save_dir_shorts: String,
    pub save_dir_trimmed: String,
    // Шаблоны имён файлов (yt-dlp output template)
    pub save_tpl_video: String,
    pub save_tpl_audio: String,
    pub save_tpl_playlist: String,
    pub save_tpl_shorts: String,
    pub save_tpl_trimmed: String,
}

#[derive(sqlx::FromRow)]
struct KV {
    key: String,
    value: String,
}

impl Database {
    pub async fn get_settings(&self) -> Result<Settings> {
        let rows = sqlx::query_as::<_, KV>("SELECT key, value FROM settings")
            .fetch_all(&self.pool)
            .await?;
        let map: HashMap<String, String> = rows.into_iter().map(|r| (r.key, r.value)).collect();
        Ok(Settings {
            download_dir: map.get("download_dir").cloned().unwrap_or_default(),
            max_concurrent: map
                .get("max_concurrent")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3),
            default_format: map
                .get("default_format")
                .cloned()
                .unwrap_or_else(|| "video".into()),
            default_quality: map
                .get("default_quality")
                .cloned()
                .unwrap_or_else(|| "best".into()),
            default_container: map
                .get("default_container")
                .cloned()
                .unwrap_or_else(|| "mp4".into()),
            default_fps: map.get("default_fps").and_then(|v| v.parse::<u32>().ok()),
            default_bitrate: map
                .get("default_bitrate")
                .and_then(|v| v.parse::<u32>().ok()),
            default_video_codec: map
                .get("default_video_codec")
                .filter(|v| !v.is_empty())
                .cloned(),
            default_audio_codec: map
                .get("default_audio_codec")
                .filter(|v| !v.is_empty())
                .cloned(),
            auto_merge: map.get("auto_merge").map(|v| v == "true").unwrap_or(true),
            embed_subtitles: map
                .get("embed_subtitles")
                .map(|v| v == "true")
                .unwrap_or(false),
            proxy: map.get("proxy").cloned().unwrap_or_default(),
            ytdlp_extra_args: map.get("ytdlp_extra_args").cloned().unwrap_or_default(),
            theme: map.get("theme").cloned().unwrap_or_else(|| "dark".into()),
            minimize_to_tray: map
                .get("minimize_to_tray")
                .map(|v| v == "true")
                .unwrap_or(true),
            save_dir_video: map.get("save_dir_video").cloned().unwrap_or_default(),
            save_dir_audio: map.get("save_dir_audio").cloned().unwrap_or_default(),
            save_dir_playlist: map.get("save_dir_playlist").cloned().unwrap_or_default(),
            save_dir_shorts: map.get("save_dir_shorts").cloned().unwrap_or_default(),
            save_dir_trimmed: map.get("save_dir_trimmed").cloned().unwrap_or_default(),
            save_tpl_video: map
                .get("save_tpl_video")
                .cloned()
                .unwrap_or_else(|| "%(title)s.%(ext)s".into()),
            save_tpl_audio: map
                .get("save_tpl_audio")
                .cloned()
                .unwrap_or_else(|| "%(title)s.%(ext)s".into()),
            save_tpl_playlist: map.get("save_tpl_playlist").cloned().unwrap_or_else(|| {
                "%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s".into()
            }),
            save_tpl_shorts: map
                .get("save_tpl_shorts")
                .cloned()
                .unwrap_or_else(|| "Shorts/%(title)s.%(ext)s".into()),
            save_tpl_trimmed: map
                .get("save_tpl_trimmed")
                .cloned()
                .unwrap_or_else(|| "%(title)s [trimmed].%(ext)s".into()),
        })
    }

    pub async fn update_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query_as::<_, KV>("SELECT key, value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.value))
    }
}
