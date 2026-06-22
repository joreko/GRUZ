use crate::{db::Database, error::Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: i64,
    pub last_url: Option<String>,
    pub last_dir: Option<String>,
    pub window_x: Option<i64>,
    pub window_y: Option<i64>,
    pub window_w: Option<i64>,
    pub window_h: Option<i64>,
    pub updated_at: i64,
}

impl Database {
    pub async fn get_session(&self) -> Result<Session> {
        // Гарантируем наличие строки (CREATE в миграции, но страховка не лишняя)
        sqlx::query("INSERT OR IGNORE INTO session (id, updated_at) VALUES (1, unixepoch())")
            .execute(&self.pool)
            .await?;
        let session = sqlx::query_as::<_, Session>(
            "SELECT id, last_url, last_dir, window_x, window_y, window_w, window_h, updated_at
             FROM session WHERE id = 1"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn update_session(&self, session: &Session) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE session SET last_url=?, last_dir=?, window_x=?, window_y=?,
             window_w=?, window_h=?, updated_at=? WHERE id=1"
        )
        .bind(&session.last_url).bind(&session.last_dir)
        .bind(session.window_x).bind(session.window_y)
        .bind(session.window_w).bind(session.window_h)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
