use crate::error::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::PathBuf;

pub mod channel_prefs;
pub mod history;
pub mod session;
pub mod settings;
pub mod tasks;

pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn connect() -> Result<Self> {
        let pool = create_pool().await?;
        Ok(Self { pool })
    }

    /// Накатить миграции. Если схема несовместима (старая БД с изменёнными
    /// миграциями) — удаляем файлы БД и создаём заново.
    pub async fn migrate(&mut self) -> Result<()> {
        if let Err(e) = sqlx::migrate!("./src/db/migrations")
            .run(&self.pool)
            .await
        {
            let msg = format!("{e:#}");
            if msg.contains("previously applied") {
                tracing::warn!("БД несовместима: {e}. Удаляю и создаю заново.");
                let db_path = db_path();

                // Закрываем старый пул через замену на заглушку
                let dummy = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect("sqlite::memory:")
                    .await?;
                let old = std::mem::replace(&mut self.pool, dummy);
                old.close().await;

                // Удаляем файлы БД
                for name in &["gruz.db", "gruz.db-wal", "gruz.db-shm"] {
                    let _ = std::fs::remove_file(db_path.with_file_name(name));
                }

                // Создаём новый пул и повторяем миграцию
                self.pool = create_pool().await?;
                sqlx::migrate!("./src/db/migrations")
                    .run(&self.pool)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
                tracing::info!("БД пересоздана, миграции накатаны.");
                return Ok(());
            }
            return Err(anyhow::anyhow!(e).into());
        }
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
    // WAL mode для конкурентных чтений
    sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await?;
    // Производительность и надёжность при параллельных воркерах
    sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await?;
    sqlx::query("PRAGMA busy_timeout=5000").execute(&pool).await?;
    sqlx::query("PRAGMA cache_size=-32000").execute(&pool).await?;
    sqlx::query("PRAGMA temp_store=MEMORY").execute(&pool).await?;
    Ok(pool)
}

fn db_path() -> PathBuf {
    // %APPDATA%/gruz/gruz.db на Windows, ~/Library/... на macOS
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gruz")
        .join("gruz.db")
}
