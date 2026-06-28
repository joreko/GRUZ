use crate::error::{AppError, Result};
use std::path::PathBuf;

pub struct YtDlp {
    pub path: PathBuf,
    /// Путь к ffmpeg — для слияния видео+аудио потоков
    pub ffmpeg_path: Option<PathBuf>,
}

impl YtDlp {
    pub fn new() -> Result<Self> {
        let path = bundled_path("yt-dlp");
        if !path.exists() {
            return Err(AppError::YtDlp(format!(
                "yt-dlp not found at {}",
                path.display()
            )));
        }
        let ffmpeg_path = {
            let p = bundled_path("ffmpeg");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        };
        Ok(Self { path, ffmpeg_path })
    }
}

fn bundled_path(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        tracing::warn!("не удалось определить путь к exe: {e}; используем текущую директорию");
        std::path::PathBuf::from(".")
    });
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    #[cfg(target_os = "windows")]
    return dir.join(format!("{}.exe", name));
    #[cfg(not(target_os = "windows"))]
    return dir.join(name);
}
