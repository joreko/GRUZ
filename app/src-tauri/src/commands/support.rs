use crate::error::Result;
use serde_json::json;

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

    let last = entries.last().ok_or_else(|| anyhow::anyhow!("Лог-файлы не найдены"))?;
    let content = std::fs::read_to_string(last.path()).map_err(|e| anyhow::anyhow!(e))?;

    // Обрезаем до последних 200 КБ чтобы не превысить лимит Gist
    let content = if content.len() > 200_000 {
        &content[content.len() - 200_000..]
    } else {
        &content
    };

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
