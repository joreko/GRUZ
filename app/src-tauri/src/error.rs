#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("yt-dlp error: {0}")]
    YtDlp(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("download failed: {0}")]
    DownloadFailed(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// Tauri требует serde::Serialize для возврата ошибок через команды
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
