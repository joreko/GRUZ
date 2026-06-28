use crate::queue::task::{DownloadTask, Priority, TaskState};
use crate::{db::Database, error::Result};
use chrono::Utc;

/// Промежуточная структура для загрузки задачи из БД
#[derive(Debug, sqlx::FromRow)]
pub struct SavedTask {
    pub id: String,
    pub url: String,
    pub video_id: Option<String>,
    pub platform: String,
    pub title: Option<String>,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<i64>,
    pub format: String,
    pub quality: String,
    pub container: String,
    pub fps: Option<i64>,
    pub bitrate: Option<i64>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub trim_start: Option<i64>,
    pub trim_end: Option<i64>,
    pub is_playlist: i64,
    // зарезервировано для будущих плейлистов
    #[allow(dead_code)]
    pub playlist_id: Option<String>,
    // зарезервировано для будущих плейлистов
    #[allow(dead_code)]
    pub playlist_index: Option<i64>,
    pub state: String,
    pub priority: i64,
    pub progress: f64,
    pub error: Option<String>,
    pub file_path: Option<String>,
    pub file_size: Option<i64>,
    // зарезервировано для отложенного запуска
    #[allow(dead_code)]
    pub schedule_at: Option<i64>,
    pub created_at: i64,
    // зарезервировано для отслеживания изменений
    #[allow(dead_code)]
    pub updated_at: i64,
}

impl From<SavedTask> for DownloadTask {
    fn from(s: SavedTask) -> Self {
        let was_interrupted = matches!(s.state.as_str(), "downloading" | "converting");
        let state = match s.state.as_str() {
            "waiting" => TaskState::Waiting,
            // при краше во время загрузки — помечаем Failed, пусть пользователь решает повторить
            "downloading" | "converting" => TaskState::Failed,
            "completed" => TaskState::Completed,
            "failed" => TaskState::Failed,
            "cancelled" => TaskState::Cancelled,
            // устаревшие состояния из старых версий БД — трактуем как Waiting
            "paused" | "scheduled" | "fetching" | "queued" => TaskState::Waiting,
            _ => TaskState::Waiting,
        };
        let priority = match s.priority {
            0 => Priority::Low,
            2 => Priority::High,
            _ => Priority::Normal,
        };
        DownloadTask {
            id: s.id,
            url: s.url,
            video_id: s.video_id,
            channel_id: s.channel_id,
            platform: s.platform,
            title: s.title,
            channel: s.channel,
            thumbnail: s.thumbnail,
            duration: s.duration,
            format: s.format,
            quality: s.quality,
            container: s.container,
            fps: s.fps.map(|v| u32::try_from(v).unwrap_or(0)),
            bitrate: s.bitrate.map(|v| u32::try_from(v).unwrap_or(0)),
            audio_codec: s.audio_codec,
            video_codec: s.video_codec,
            trim_start: s.trim_start,
            trim_end: s.trim_end,
            is_playlist: s.is_playlist != 0,
            state,
            priority,
            progress: s.progress as f32,
            speed: None,
            eta: None,
            error: if was_interrupted && s.error.is_none() {
                Some("прервано при предыдущем запуске".into())
            } else {
                s.error
            },
            file_path: s.file_path,
            file_size: s.file_size,
            // chrono::DateTime из unix-секунд; unwrap_or на случай переполнения
            created_at: chrono::DateTime::from_timestamp(s.created_at, 0)
                .unwrap_or_else(chrono::Utc::now),
        }
    }
}

impl Database {
    pub async fn save_task(&self, task: &DownloadTask) -> Result<()> {
        let state = task.state.to_string();
        let priority = task.priority.clone() as i64;
        let now = Utc::now().timestamp();
        // INSERT с сохранением created_at при повторном вызове (ON CONFLICT → UPDATE)
        sqlx::query(
            "INSERT INTO tasks
             (id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
              format, quality, container, fps, bitrate, audio_codec, video_codec,
              trim_start, trim_end, is_playlist, playlist_id, playlist_index,
              state, priority, progress, error, file_path, file_size, schedule_at,
              created_at, updated_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
             ON CONFLICT(id) DO UPDATE SET
              state=excluded.state, priority=excluded.priority, progress=excluded.progress,
              error=excluded.error, file_path=excluded.file_path, file_size=excluded.file_size,
              updated_at=excluded.updated_at",
        )
        .bind(&task.id)
        .bind(&task.url)
        .bind(&task.video_id)
        .bind(&task.platform)
        .bind(&task.title)
        .bind(&task.channel)
        .bind(&task.channel_id)
        .bind(&task.thumbnail)
        .bind(task.duration)
        .bind(&task.format)
        .bind(&task.quality)
        .bind(&task.container)
        .bind(task.fps.map(|v| v as i64))
        .bind(task.bitrate.map(|v| v as i64))
        .bind(&task.audio_codec)
        .bind(&task.video_codec)
        .bind(task.trim_start)
        .bind(task.trim_end)
        .bind(task.is_playlist as i64)
        .bind(Option::<String>::None)
        .bind(Option::<i64>::None)
        .bind(&state)
        .bind(priority)
        .bind(task.progress as f64)
        .bind(&task.error)
        .bind(&task.file_path)
        .bind(task.file_size)
        .bind(Option::<i64>::None)
        .bind(task.created_at.timestamp())
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_pending_tasks(&self) -> Result<Vec<SavedTask>> {
        let tasks = sqlx::query_as::<_, SavedTask>(
            "SELECT id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
             format, quality, container, fps, bitrate, audio_codec, video_codec,
             trim_start, trim_end, is_playlist, playlist_id, playlist_index,
             state, priority, progress, error, file_path, file_size, schedule_at,
             created_at, updated_at
             FROM tasks WHERE state IN ('waiting', 'paused', 'downloading', 'converting')
             ORDER BY priority DESC, created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }

    pub async fn delete_task(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
