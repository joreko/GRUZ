use crate::error::Result;
use crate::logging;
use serde_json::json;
use std::io::{Read, Seek, SeekFrom};

/// Читает последний лог-файл, загружает на GitHub Gist анонимно, возвращает URL.
#[tauri::command]
pub async fn upload_log() -> Result<String> {
    let log_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gruz")
        .join("logs");

    // Последний файл по имени (имя содержит timestamp)
    let mut entries: Vec<_> = std::fs::read_dir(&log_dir)
        .map_err(|e| anyhow::anyhow!(e))?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("gruz_"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let last = entries
        .last()
        .ok_or_else(|| anyhow::anyhow!("Лог-файлы не найдены"))?;

    // Читаем последние 200_000 байт через seek — не грузим весь файл в память
    let mut file = std::fs::File::open(last.path()).map_err(|e| anyhow::anyhow!(e))?;
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let read_size = 200_000.min(file_len as usize);
    let mut content = String::with_capacity(read_size);
    if read_size > 0 {
        file.seek(SeekFrom::End(-(read_size as i64))).ok();
        file.take(read_size as u64)
            .read_to_string(&mut content)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    let filename = last.file_name().to_string_lossy().to_string();
    let body = json!({
        "description": "ГРУЗ — лог для поддержки",
        "public": false,
        "files": {
            filename: { "content": content }
        }
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.github.com/gists")
        .header("User-Agent", "gruz-app")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| anyhow::anyhow!(e))?;
    let url = json["html_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Не удалось получить URL от GitHub"))?
        .to_string();

    Ok(url)
}

/// Меняет уровень логирования на лету (без перезапуска).
/// `level` — директива фильтра, напр. `gruz=debug`, `gruz=trace`, `gruz=warn`, `off`.
#[tauri::command]
pub fn set_log_level(level: String) {
    logging::set_log_level(level.trim());
}

/// Возвращает последние строки лога (кольцевой буфер) для живого просмотра.
#[tauri::command]
pub fn get_log_history() -> Vec<String> {
    logging::get_log_history()
}

/// Логирует сообщение с фронта (тосты, пользовательские события) на бэкенде.
/// `level` — один из: error, warn, info, debug, trace (по умолчанию info).
#[tauri::command]
pub fn log_frontend(message: String, level: Option<String>) {
    match level.as_deref() {
        Some("error") => tracing::error!("[frontend] {message}"),
        Some("warn") => tracing::warn!("[frontend] {message}"),
        Some("debug") => tracing::debug!("[frontend] {message}"),
        Some("trace") => tracing::trace!("[frontend] {message}"),
        _ => tracing::info!("[frontend] {message}"),
    }
}
