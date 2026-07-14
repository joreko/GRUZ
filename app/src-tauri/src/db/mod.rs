use crate::error::Result;
use sqlx::Row;
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
///   2 — галерея: local_thumbnail/favorite/deleted_at/duration_real/width/height
///       в history; таблицы albums/album_items; мягкое удаление
///   3 — очередь: колонка ordering (явный ручной порядок задач)
///   4 — унификация: таблицы tasks и history слиты в единую downloads
///       (задачи и скачанное живут в одной таблице, одна строка на загрузку)
///   5 — source_fps: реальный fps источника, хранится для отображения
///       в галерее, даже когда пользователь не выбрал лимит fps
const SCHEMA_VERSION: i64 = 5;

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

        if current < 2 {
            self.migrate_v2().await?;
        }

        if current < 3 {
            self.migrate_v3().await?;
        }

        if current < 4 {
            self.migrate_v4().await?;
        }

        if current < 5 {
            self.migrate_v5().await?;
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

    /// Миграция схемы до версии 2 (галерея мирового уровня).
    /// Идемпотентна: каждая ALTER-колонка добавляется только если её ещё нет
    /// (на случай падения посреди предыдущей попытки). Никаких потерь данных.
    async fn migrate_v2(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Существующие колонки history — чтобы не падать на повторном ALTER.
        let existing: Vec<String> = sqlx::query("PRAGMA table_info(history)")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();
        let has = |name: &str| existing.iter().any(|c| c == name);

        if !has("local_thumbnail") {
            sqlx::query("ALTER TABLE history ADD COLUMN local_thumbnail TEXT")
                .execute(&mut *tx)
                .await?;
        }
        if !has("favorite") {
            sqlx::query("ALTER TABLE history ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0")
                .execute(&mut *tx)
                .await?;
        }
        if !has("deleted_at") {
            sqlx::query("ALTER TABLE history ADD COLUMN deleted_at INTEGER")
                .execute(&mut *tx)
                .await?;
        }
        if !has("duration_real") {
            sqlx::query("ALTER TABLE history ADD COLUMN duration_real INTEGER")
                .execute(&mut *tx)
                .await?;
        }
        if !has("width") {
            sqlx::query("ALTER TABLE history ADD COLUMN width INTEGER")
                .execute(&mut *tx)
                .await?;
        }
        if !has("height") {
            sqlx::query("ALTER TABLE history ADD COLUMN height INTEGER")
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS albums (
                id          TEXT    PRIMARY KEY,
                name        TEXT    NOT NULL,
                kind        TEXT    NOT NULL DEFAULT 'user',
                query       TEXT,
                created_at  INTEGER NOT NULL
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS album_items (
                album_id    TEXT    NOT NULL,
                history_id  TEXT    NOT NULL,
                position    INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (album_id, history_id)
            )",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_favorite ON history(favorite)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_deleted ON history(deleted_at)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_album_items_album ON album_items(album_id)")
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Миграция схемы до версии 3: добавляем колонку ordering в tasks.
    /// Идемпотентна — колонка добавляется только если её ещё нет.
    async fn migrate_v3(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let existing: Vec<String> = sqlx::query("PRAGMA table_info(tasks)")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();
        if !existing.iter().any(|c| c == "ordering") {
            sqlx::query("ALTER TABLE tasks ADD COLUMN ordering INTEGER")
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Миграция схемы до версии 4: унификация tasks + history в единую таблицу downloads.
    /// Идемпотентна и не теряет данные: создаёт downloads со всеми колонками обеих
    /// таблиц, переносит активные задачи (tasks) и скачанное (history), затем
    /// удаляет старые таблицы.
    async fn migrate_v4(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "CREATE TABLE downloads (
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
                ordering        INTEGER,
                schedule_at     INTEGER,
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL,
                local_thumbnail TEXT,
                favorite        INTEGER NOT NULL DEFAULT 0,
                deleted_at      INTEGER,
                duration_real    INTEGER,
                width           INTEGER,
                height          INTEGER
            )",
        )
        .execute(&mut *tx)
        .await?;

        // Активные/завершённые задачи из tasks → downloads (те же колонки).
        sqlx::query(
            "INSERT OR IGNORE INTO downloads
             (id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
              format, quality, container, fps, bitrate, audio_codec, video_codec,
              trim_start, trim_end, is_playlist, playlist_id, playlist_index,
              state, priority, progress, error, file_path, file_size,
              ordering, schedule_at, created_at, updated_at,
              local_thumbnail, favorite, deleted_at, duration_real, width, height)
             SELECT id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
              format, quality, container, fps, bitrate, audio_codec, video_codec,
              trim_start, trim_end, is_playlist, playlist_id, playlist_index,
              state, priority, progress, error, file_path, file_size,
              ordering, schedule_at, created_at, updated_at,
              NULL, 0, NULL, NULL, NULL, NULL FROM tasks",
        )
        .execute(&mut *tx)
        .await?;

        // Скачанное из history → downloads (state='completed'). Конфликты id игнорируем.
        sqlx::query(
            "INSERT OR IGNORE INTO downloads
             (id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
              file_path, file_size, format, quality, container, fps, bitrate,
              audio_codec, video_codec, trim_start, trim_end, playlist_id, playlist_index,
              created_at,
              local_thumbnail, favorite, deleted_at, duration_real, width, height,
              state, priority, progress, error, ordering, schedule_at, updated_at)
             SELECT id, url, video_id, platform, title, channel, channel_id, thumbnail, duration,
              file_path, file_size, format, quality, container, fps, bitrate,
              audio_codec, video_codec, trim_start, trim_end, playlist_id, playlist_index,
              created_at,
              local_thumbnail, favorite, deleted_at, duration_real, width, height,
              'completed', 1, 100.0, NULL, NULL, NULL, created_at FROM history",
        )
        .execute(&mut *tx)
        .await?;

        // Удаляем старые таблицы — данные уже перенесены.
        sqlx::query("DROP TABLE IF EXISTS tasks")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS history")
            .execute(&mut *tx)
            .await?;

        // Индексы (покрывают и запросы галереи, и запросы очереди).
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state)")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_priority ON downloads(priority DESC, created_at ASC)
             WHERE state IN ('waiting', 'paused')",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_created ON downloads(created_at DESC)",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_downloads_deleted ON downloads(deleted_at)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_downloads_favorite ON downloads(favorite)")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_video_id ON downloads(video_id) WHERE video_id IS NOT NULL",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_channel ON downloads(channel_id) WHERE channel_id IS NOT NULL",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_platform ON downloads(platform) WHERE platform != ''",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_downloads_playlist ON downloads(playlist_id) WHERE playlist_id IS NOT NULL",
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Миграция схемы до версии 5: source_fps в downloads.
    /// Идемпотентна — колонка добавляется только если её ещё нет.
    async fn migrate_v5(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let existing: Vec<String> = sqlx::query("PRAGMA table_info(downloads)")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();
        if !existing.iter().any(|c| c == "source_fps") {
            sqlx::query("ALTER TABLE downloads ADD COLUMN source_fps INTEGER")
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

/// Папка локальных превью: <data_dir>/gruz/thumbs/
/// (та же родительская папка, что и у gruz.db). Создаётся в setup().
pub fn thumbs_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gruz")
        .join("thumbs")
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
