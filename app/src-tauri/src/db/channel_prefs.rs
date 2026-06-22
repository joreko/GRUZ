use crate::{db::Database, error::Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChannelPrefs {
    pub channel_id: String,
    pub channel_name: String,
    pub platform: String,
    pub format: Option<String>,
    pub quality: Option<String>,
    pub container: Option<String>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub fps: Option<i64>,
    pub download_dir: Option<String>,
    pub updated_at: i64,
}

impl Database {
    pub async fn get_channel_prefs(&self, channel_id: &str) -> Result<Option<ChannelPrefs>> {
        let prefs = sqlx::query_as::<_, ChannelPrefs>(
            "SELECT channel_id, channel_name, platform, format, quality, container,
             audio_codec, video_codec, fps, download_dir, updated_at
             FROM channel_prefs WHERE channel_id = ?"
        )
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(prefs)
    }

    pub async fn upsert_channel_prefs(&self, prefs: &ChannelPrefs) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO channel_prefs
             (channel_id, channel_name, platform, format, quality, container,
              audio_codec, video_codec, fps, download_dir, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(channel_id) DO UPDATE SET
               channel_name=excluded.channel_name, platform=excluded.platform,
               format=excluded.format, quality=excluded.quality,
               container=excluded.container, audio_codec=excluded.audio_codec,
               video_codec=excluded.video_codec, fps=excluded.fps,
               download_dir=excluded.download_dir, updated_at=excluded.updated_at"
        )
        .bind(&prefs.channel_id).bind(&prefs.channel_name).bind(&prefs.platform)
        .bind(&prefs.format).bind(&prefs.quality).bind(&prefs.container)
        .bind(&prefs.audio_codec).bind(&prefs.video_codec).bind(prefs.fps)
        .bind(&prefs.download_dir).bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_channel_prefs(&self) -> Result<Vec<ChannelPrefs>> {
        let prefs = sqlx::query_as::<_, ChannelPrefs>(
            "SELECT channel_id, channel_name, platform, format, quality, container,
             audio_codec, video_codec, fps, download_dir, updated_at
             FROM channel_prefs ORDER BY channel_name ASC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(prefs)
    }

    pub async fn delete_channel_prefs(&self, channel_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM channel_prefs WHERE channel_id = ?")
            .bind(channel_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
