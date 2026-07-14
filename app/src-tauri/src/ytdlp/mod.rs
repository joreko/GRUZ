use crate::error::{AppError, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub struct YtDlp {
    pub path: PathBuf,
    /// Путь к ffmpeg — для слияния видео+аудио потоков
    pub ffmpeg_path: Option<PathBuf>,
}

impl YtDlp {
    pub fn new() -> Result<Self> {
        // yt-dlp копируется в пользовательскую папку данных (где есть права на
        // запись), чтобы `yt-dlp -U` при старте мог обновить бинарник на месте.
        // В папке установки (Program Files) прав обычно нет — обновление там
        // падает. Если копия уже есть — используем её (это может быть свежая
        // версия после self-update). При ошибке копирования откатываемся на
        // зашитый бандл (тогда self-update не сработает, но приложение работает).
        let bundled = bundled_path("yt-dlp");
        if !bundled.exists() {
            return Err(AppError::YtDlp(format!(
                "yt-dlp not found at {}",
                bundled.display()
            )));
        }
        let path = resolve_writable_ytdlp(&bundled);

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

    /// Самообновление yt-dlp (`yt-dlp -U`) в фоне при старте приложения.
    /// Обновляет бинарник на месте. При ошибке (напр. нет прав на запись
    /// в Program Files) — логирует warning и продолжает со встроенной версией.
    /// Не блокирует запуск: вызывается из отдельного spawned-таска.
    pub async fn self_update(&self, proxy: Option<&str>) {
        let mut cmd = Command::new(&self.path);
        no_window(&mut cmd);
        cmd.arg("-U").stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(p) = proxy.filter(|p| !p.is_empty()) {
            cmd.args(["--proxy", p]);
        }
        match timeout(Duration::from_secs(120), cmd.status()).await {
            Ok(Ok(status)) if status.success() => {
                tracing::info!("yt-dlp самообновлён до последней версии");
            }
            Ok(Ok(status)) => {
                tracing::debug!(
                    "yt-dlp -U завершился с кодом {status}; используется встроенная версия"
                );
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    "не удалось запустить yt-dlp -U: {e}; используется встроенная версия"
                );
            }
            Err(_) => {
                tracing::warn!("yt-dlp -U не ответил за 120с; используется встроенная версия");
            }
        }
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

/// Резолвит путь к yt-dlp в пользовательской папке данных (с правами записи),
/// чтобы self-update (`-U`) мог обновлять бинарник. При первом запуске
/// копирует зашитый бандл; в дальнейшем переиспользует копию.
fn resolve_writable_ytdlp(bundled: &PathBuf) -> PathBuf {
    let data_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gruz");
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        tracing::warn!(
            "не удалось создать папку данных {}: {e}",
            data_dir.display()
        );
        return bundled.clone();
    }
    let target = data_dir.join("yt-dlp.exe");

    if target.exists() {
        return target;
    }
    match std::fs::copy(bundled, &target) {
        Ok(_) => {
            tracing::info!(
                "yt-dlp скопирован в пользовательскую папку: {}",
                target.display()
            );
            target
        }
        Err(e) => {
            tracing::warn!(
                "не удалось скопировать yt-dlp в {}: {e}; используем зашитый бандл (self-update недоступен)",
                target.display()
            );
            bundled.clone()
        }
    }
}
