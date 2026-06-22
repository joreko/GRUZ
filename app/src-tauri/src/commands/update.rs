use crate::error::Result;
use tauri::AppHandle;

/// Скачивает Setup.exe по URL во временную директорию и запускает установщик.
#[tauri::command]
pub async fn install_version(url: String, app: AppHandle) -> Result<()> {
    let tmp = std::env::temp_dir().join("gruz_update_setup.exe");

    let bytes = reqwest::get(&url)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    tokio::fs::write(&tmp, &bytes)
        .await?;

    std::process::Command::new(&tmp)
        .spawn()?;

    app.exit(0);

    Ok(())
}
