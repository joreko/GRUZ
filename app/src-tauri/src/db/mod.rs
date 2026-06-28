use crate::error::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::PathBuf;

pub mod channel_prefs;
pub mod history;
pub mod session;
pub mod settings;
pub mod tasks;

/// Текущая версия схемы БД. Увеличивать при каждом изменении.
/// История:
///   1 — все начальные таблицы (history, settings, tasks, channel_prefs,
///       quality_profiles, source_stats, session)
const SCHEMA_VERSION: i64 = 1;

pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn connect() -> Result<Self> {
        let pool = create_pool().await?;
        Ok(Self { pool })
    }

    /// Накатить схему до актуальной версии. Не удаляет пользовательские данные.
    pub async fn migrate(&mut self) -> Result<()> {
        sqlx::query("CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER NOT NULL)")
            .execute(&self.pool)
            .await?;

        let current: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE((SELECT version FROM _schema_version LIMIT 1), 0)",
        )
        .fetch_one(&self.pool)
        .await?;

        if current == SCHEMA_VERSION {
            return Ok(());
        }

        if current > SCHEMA_VERSION {
            return Err(anyhow::anyhow!(
                "База данных новее приложения — обновите программу (схема {current} > {SCHEMA_VERSION})"
            )
            .into());
        }

        if current < 1 {
            self.migrate_v1().await?;
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM _schema_version")
            .execute(&mut *tx)
            .await
            .ok();
        sqlx::query("INSERT INTO _schema_version (version) VALUES (?)")
            .bind(SCHEMA_VERSION)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        tracing::info!("БД приведена к версии {SCHEMA_VERSION}");
        Ok(())
    }

    async fn migrate_v1(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS history (
                id              TEXT    PRIMARY KEY,
                url             TEXT    NOT NULL,
                video_id        TEXT,
                platform        TEXT    NOT NULL DEFAULT '',
                title           TEXT    NOT NULL,
                channel         TEXT,
                channel_id      TEXT,
                thumbnail       TEXT,
                duration        INTEGER,
                file_path       TEXT    NOT NULL,
                file_size       INTEGER,
                format          TEXT    NOT NULL,
                quality         TEXT    NOT NULL,
                container       TEXT    NOT NULL,
                fps             INTEGER,
                bitrate         INTEGER,
                audio_codec     TEXT,
                video_codec     TEXT,
                trim_start      INTEGER,
                trim_end        INTEGER,
                playlist_id     TEXT,
                playlist_index  INTEGER,
                created_at      INTEGER NOT NULL
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_created  ON history(created_at DESC)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_video_id ON history(video_id) WHERE video_id IS NOT NULL")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_channel  ON history(channel_id) WHERE channel_id IS NOT NULL")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_platform ON history(platform) WHERE platform != ''")
            .execute(&mut *tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_playlist ON history(playlist_id) WHERE playlist_id IS NOT NULL")
            .execute(&mut *tx).await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                key         TEXT    PRIMARY KEY,
                value       TEXT    NOT NULL,
                updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO settings (key, value) VALUES
                ('download_dir',         ''),
                ('max_concurrent',       '3'),
                ('proxy',                ''),
                ('ytdlp_extra_args',     ''),

                ('default_format',       'video'),
                ('default_quality',      'best'),
                ('default_container',    'mp4'),
                ('auto_merge',           'true'),
                ('embed_subtitles',      'false'),
                ('save_dir_video',       ''),
                ('save_dir_audio',       ''),
                ('save_dir_playlist',    ''),
                ('save_dir_shorts',      ''),
                ('save_dir_trimmed',     ''),
                ('save_tpl_video',       '%(title)s.%(ext)s'),
                ('save_tpl_audio',       '%(title)s.%(ext)s'),
                ('save_tpl_playlist',    '%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s'),
                ('save_tpl_shorts',      'Shorts/%(title)s.%(ext)s'),
                ('save_tpl_trimmed',     '%(title)s [trimmed].%(ext)s'),
                ('theme',                'dark'),
                ('minimize_to_tray',     'true')
"
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id              TEXT    PRIMARY KEY,
                url             TEXT    NOT NULL,
                video_id        TEXT,
                platform        TEXT    NOT NULL DEFAULT '',
                title           TEXT,
                channel         TEXT,
                channel_id      TEXT,
                thumbnail       TEXT,
                duration        INTEGER,
                format          TEXT    NOT NULL,
                quality         TEXT    NOT NULL,
                container       TEXT    NOT NULL,
                fps             INTEGER,
                bitrate         INTEGER,
                audio_codec     TEXT,
                video_codec     TEXT,
                trim_start      INTEGER,
                trim_end        INTEGER,
                is_playlist     INTEGER NOT NULL DEFAULT 0,
                playlist_id     TEXT,
                playlist_index  INTEGER,
                state           TEXT    NOT NULL DEFAULT 'waiting',
                priority        INTEGER NOT NULL DEFAULT 1,
                progress        REAL    NOT NULL DEFAULT 0.0,
                error           TEXT,
                file_path       TEXT,
                file_size       INTEGER,
                schedule_at     INTEGER,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_state ON tasks(state)")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority DESC, created_at ASC)
             WHERE state IN ('waiting', 'paused')",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_tasks_schedule ON tasks(schedule_at)
             WHERE schedule_at IS NOT NULL",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS channel_prefs (
                channel_id      TEXT    PRIMARY KEY,
                channel_name    TEXT    NOT NULL DEFAULT '',
                platform        TEXT    NOT NULL DEFAULT '',
                format          TEXT,
                quality         TEXT,
                container       TEXT,
                audio_codec     TEXT,
                video_codec     TEXT,
                fps             INTEGER,
                download_dir    TEXT,
                updated_at      INTEGER NOT NULL
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS session (
                id          INTEGER PRIMARY KEY DEFAULT 1,
                last_url    TEXT,
                last_dir    TEXT,
                window_x    INTEGER,
                window_y    INTEGER,
                window_w    INTEGER,
                window_h    INTEGER,
                updated_at  INTEGER NOT NULL
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("INSERT OR IGNORE INTO session (id, updated_at) VALUES (1, unixepoch())")
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

async fn create_pool() -> Result<SqlitePool> {
    let db_path = db_path();
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous=NORMAL")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout=5000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA cache_size=-32000")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA temp_store=MEMORY")
        .execute(&pool)
        .await?;
    Ok(pool)
}

fn db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gruz")
        .join("gruz.db")
}
