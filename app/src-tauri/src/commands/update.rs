use crate::error::Result;
use futures_util::StreamExt;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// DETACHED_PROCESS: установщик живёт независимо от завершающегося gruz.exe
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Прогресс скачивания обновления — отправляется в UI через событие `update:progress`.
#[derive(Clone, serde::Serialize)]
pub struct UpdateProgress {
    /// Скачано байт
    pub downloaded: u64,
    /// Всего байт (None если сервер не отдал Content-Length)
    pub total: Option<u64>,
    /// Процент 0..100 (None если total неизвестен)
    pub pct: Option<u8>,
    /// Файл скачан и установщик запущен — UI может показать финальное состояние
    pub done: bool,
}

/// Скачивает Setup.exe по URL во временную директорию со стримингом прогресса
/// и запускает установщик. Прогресс шлётся событиями `update:progress`.
/// При успехе закрывает приложение — установщик в preflight требует,
/// чтобы gruz.exe не был запущен. При ошибке чистит частично скачанный файл.
#[tauri::command]
pub async fn install_version(url: String, app: AppHandle) -> Result<()> {
    let tmp = std::env::temp_dir().join("gruz_update_setup.exe");

    match download_and_launch(&url, &tmp, &app).await {
        Ok(()) => {
            // Даём финальному событию долететь до UI, затем закрываемся.
            tokio::time::sleep(Duration::from_millis(700)).await;
            app.exit(0);
            Ok(())
        }
        Err(e) => {
            // Удаляем частично скачанный файл, чтобы не оставлять мусор и не
            // запустить повреждённый установщик при повторной попытке.
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

async fn download_and_launch(url: &str, tmp: &std::path::Path, app: &AppHandle) -> Result<()> {
    // Таймаут только на установку соединения — само скачивание может быть долгим,
    // но зависший коннект не должен блокировать UI навсегда.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("не удалось создать HTTP-клиент: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("не удалось начать скачивание: {e}"))?;

    if !resp.status().is_success() {
        return Err(crate::error::AppError::Other(anyhow::anyhow!(
            "сервер вернул {} (возможно, для этой версии нет установщика)",
            resp.status()
        )));
    }

    let total = resp.content_length();
    let mut downloaded: u64 = 0;
    let mut last_pct: i32 = -1;

    let mut file = tokio::fs::File::create(tmp).await?;
    let mut stream = resp.bytes_stream();

    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow::anyhow!("обрыв скачивания: {e}"))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let pct = total.map(|t| {
            if t == 0 { 0 } else { ((downloaded * 100) / t).min(100) as u8 }
        });

        // Шлём событие только при смене целого процента — не спамим UI
        let cur = pct.map(|p| p as i32).unwrap_or(-1);
        if cur != last_pct {
            last_pct = cur;
            let _ = app.emit(
                "update:progress",
                UpdateProgress { downloaded, total, pct, done: false },
            );
        }
    }

    file.flush().await?;
    drop(file);

    // Защита от обрыва: если сервер объявил размер, а скачали меньше — файл битый.
    if let Some(t) = total {
        if downloaded < t {
            return Err(crate::error::AppError::Other(anyhow::anyhow!(
                "скачивание прервано: получено {downloaded} из {t} байт"
            )));
        }
    }

    let mut cmd = std::process::Command::new(tmp);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    cmd.spawn()
        .map_err(|e| anyhow::anyhow!("не удалось запустить установщик: {e}"))?;

    let _ = app.emit(
        "update:progress",
        UpdateProgress { downloaded, total, pct: Some(100), done: true },
    );

    Ok(())
}
