#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("ошибка yt-dlp: {0}")]
    YtDlp(String),

    #[error("ошибка базы данных: {0}")]
    Database(#[from] sqlx::Error),

    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("ошибка загрузки: {0}")]
    DownloadFailed(String),

    #[error("отменено")]
    Cancelled,

    #[error("ошибка проверки: {0}")]
    Validation(String),

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
