use crate::error::{AppError, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

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
            if p.exists() { Some(p) } else { None }
        };
        Ok(Self { path, ffmpeg_path })
    }

    /// Версия установленного yt-dlp
    #[allow(dead_code)]
    pub async fn version(&self) -> Result<String> {
        let out = Command::new(&self.path).arg("--version").output().await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Обновить до последней версии (вызывается из настроек)
    #[allow(dead_code)]
    pub async fn self_update(&self) -> Result<()> {
        info!("updating yt-dlp...");
        let status = Command::new(&self.path).arg("-U").status().await?;
        if !status.success() {
            return Err(AppError::YtDlp("self-update failed".into()));
        }
        Ok(())
    }
}

fn bundled_path(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    #[cfg(target_os = "windows")]
    return dir.join(format!("{}.exe", name));
    #[cfg(not(target_os = "windows"))]
    return dir.join(name);
}
