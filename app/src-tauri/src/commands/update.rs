use crate::error::Result;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Read;
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
    // Имя кэш-файла берём из URL — уникально для каждой версии.
    // Если файл уже полностью скачан (размер совпадёт с Content-Length), повторно не качаем.
    let raw = url.rsplit('/').next().unwrap_or("gruz_setup.exe");
    let raw = raw.split('?').next().unwrap_or("gruz_setup.exe");
    // Санитизация: оставляем только безопасные символы, убираем path traversal
    let filename: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    let filename = if filename.is_empty() {
        "gruz_setup.exe".to_string()
    } else {
        filename
    };
    let tmp = std::env::temp_dir().join(&filename);

    // Скачиваем — при ошибке чистим неполный файл.
    if let Err(e) = download(&url, &tmp, &app).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }

    // Сверяем контрольную сумму, если она опубликована (защита от MITM/битого файла).
    // Для старых релизов без .sha256 проверка пропускается — установка не блокируется.
    if let Err(e) = verify_checksum(&url, &tmp, &app).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }

    // Запускаем — файл корректен, не удаляем даже при ошибке spawn.
    launch_installer(&tmp, &app)?;

    tokio::time::sleep(Duration::from_millis(700)).await;
    app.exit(0);
    Ok(())
}

async fn download(url: &str, tmp: &std::path::Path, app: &AppHandle) -> Result<()> {
    // Таймаут только на установку соединения — само скачивание может быть долгим,
    // но зависший коннект не должен блокировать UI навсегда.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("не удалось создать HTTP-клиент: {e}"))?;

    // Проверяем кэш: если файл уже лежит и размер совпадает — не качаем повторно.
    let cached_size = tokio::fs::metadata(tmp).await.ok().map(|m| m.len());
    if let Some(on_disk) = cached_size {
        if let Ok(head) = client.head(url).send().await {
            if let Some(remote_len) = head.content_length() {
                if on_disk == remote_len && remote_len > 0 {
                    // Файл уже полный — сигналим UI и возвращаемся без скачивания.
                    let _ = app.emit(
                        "update:progress",
                        UpdateProgress {
                            downloaded: on_disk,
                            total: Some(remote_len),
                            pct: Some(100),
                            done: false,
                        },
                    );
                    return Ok(());
                }
            }
        }
    }

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
            if t == 0 {
                0
            } else {
                ((downloaded * 100) / t).min(100) as u8
            }
        });

        // Шлём событие только при смене целого процента — не спамим UI
        let cur = pct.map(|p| p as i32).unwrap_or(-1);
        if cur != last_pct {
            last_pct = cur;
            let _ = app.emit(
                "update:progress",
                UpdateProgress {
                    downloaded,
                    total,
                    pct,
                    done: false,
                },
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

    // Сигнализируем UI что скачивание завершено (запуск — отдельный шаг).
    let _ = app.emit(
        "update:progress",
        UpdateProgress {
            downloaded,
            total,
            pct: Some(100),
            done: false,
        },
    );
    Ok(())
}

fn launch_installer(tmp: &std::path::Path, app: &AppHandle) -> Result<()> {
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
        UpdateProgress {
            downloaded: 0,
            total: None,
            pct: Some(100),
            done: true,
        },
    );
    Ok(())
}

fn sha256_file(path: &std::path::Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// Сверяет SHA256 скачанного установщика с опубликованным файлом `<exe>.sha256`.
// Если файл контрольной суммы недоступен (старые релизы) или некорректен —
// проверка пропускается, установка не блокируется. Блокируем только при
// реальном НЕсовпадении (файл повреждён при скачивании или подменён).
async fn verify_checksum(exe_url: &str, tmp: &std::path::Path, _app: &AppHandle) -> Result<()> {
    let checksum_url = format!("{}.sha256", exe_url);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("не удалось создать HTTP-клиент: {e}"))?;

    let resp = match client.get(&checksum_url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("контрольная сумма недоступна, проверка пропущена: {e}");
            return Ok(());
        }
    };
    if !resp.status().is_success() {
        tracing::warn!(
            "файл контрольной суммы отсутствует ({}), проверка пропущена",
            resp.status()
        );
        return Ok(());
    }

    let text = resp
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("не удалось прочитать контрольную сумму: {e}"))?;
    // Формат sha256sum: "<hash>  <имя>" или просто "<hash>"
    let expected = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::warn!("некорректный формат контрольной суммы, проверка пропущена");
        return Ok(());
    }

    let actual = sha256_file(tmp)?;
    if actual != expected {
        tracing::error!(
            "контрольная сумма установщика НЕ совпала: ожидалось {expected}, получено {actual}"
        );
        return Err(crate::error::AppError::Other(anyhow::anyhow!(
            "контрольная сумма не совпала: файл повреждён или подменён"
        )));
    }

    tracing::info!("контрольная сумма установщика совпала");
    Ok(())
}
