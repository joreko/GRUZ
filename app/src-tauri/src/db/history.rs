use crate::{db::Database, error::Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoryItem {
    pub id: String,
    pub url: String,
    pub video_id: Option<String>,
    pub platform: String,
    pub title: String,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<i64>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub format: String,
    pub quality: String,
    pub container: String,
    pub fps: Option<i64>,
    pub bitrate: Option<i64>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub trim_start: Option<i64>,
    pub trim_end: Option<i64>,
    pub playlist_id: Option<String>,
    pub playlist_index: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewHistoryItem {
    pub url: String,
    pub video_id: Option<String>,
    pub platform: String,
    pub title: String,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<i64>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub format: String,
    pub quality: String,
    pub container: String,
    pub fps: Option<i64>,
    pub bitrate: Option<i64>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub trim_start: Option<i64>,
    pub trim_end: Option<i64>,
    pub playlist_id: Option<String>,
    pub playlist_index: Option<i64>,
}

impl Database {
    pub async fn add_history(&self, item: NewHistoryItem) -> Result<HistoryItem> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO history (id, url, video_id, platform, title, channel, channel_id,
             thumbnail, duration, file_path, file_size, format, quality, container,
             fps, bitrate, audio_codec, video_codec, trim_start, trim_end,
             playlist_id, playlist_index, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&item.url)
        .bind(&item.video_id)
        .bind(&item.platform)
        .bind(&item.title)
        .bind(&item.channel)
        .bind(&item.channel_id)
        .bind(&item.thumbnail)
        .bind(item.duration)
        .bind(&item.file_path)
        .bind(item.file_size)
        .bind(&item.format)
        .bind(&item.quality)
        .bind(&item.container)
        .bind(item.fps)
        .bind(item.bitrate)
        .bind(&item.audio_codec)
        .bind(&item.video_codec)
        .bind(item.trim_start)
        .bind(item.trim_end)
        .bind(&item.playlist_id)
        .bind(item.playlist_index)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(HistoryItem {
            id,
            url: item.url,
            video_id: item.video_id,
            platform: item.platform,
            title: item.title,
            channel: item.channel,
            channel_id: item.channel_id,
            thumbnail: item.thumbnail,
            duration: item.duration,
            file_path: item.file_path,
            file_size: item.file_size,
            format: item.format,
            quality: item.quality,
            container: item.container,
            fps: item.fps,
            bitrate: item.bitrate,
            audio_codec: item.audio_codec,
            video_codec: item.video_codec,
            trim_start: item.trim_start,
            trim_end: item.trim_end,
            playlist_id: item.playlist_id,
            playlist_index: item.playlist_index,
            created_at,
        })
    }

    pub async fn get_history(&self, limit: i64, offset: i64) -> Result<Vec<HistoryItem>> {
        let items = sqlx::query_as::<_, HistoryItem>(
            "SELECT id, url, video_id, platform, title, channel, channel_id,
             thumbnail, duration, file_path, file_size, format, quality, container,
             fps, bitrate, audio_codec, video_codec, trim_start, trim_end,
             playlist_id, playlist_index, created_at
             FROM history ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    pub async fn search_history(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<HistoryItem>> {
        // Экранируем спецсимволы LIKE: _ → \_, % → \%
        let escaped = query
            .replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%");
        let pattern = format!("%{}%", escaped);
        let items = sqlx::query_as::<_, HistoryItem>(
            "SELECT id, url, video_id, platform, title, channel, channel_id,
             thumbnail, duration, file_path, file_size, format, quality, container,
             fps, bitrate, audio_codec, video_codec, trim_start, trim_end,
             playlist_id, playlist_index, created_at
             FROM history WHERE title LIKE ? ESCAPE '\\' OR channel LIKE ? ESCAPE '\\' OR url LIKE ? ESCAPE '\\'
             ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(&pattern).bind(&pattern).bind(&pattern).bind(limit).bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    pub async fn delete_history_item(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM history WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_history(&self) -> Result<()> {
        sqlx::query("DELETE FROM history")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
