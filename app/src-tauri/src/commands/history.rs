use crate::db::history::{Album, HistoryItem};
use crate::error::AppError;
use crate::error::Result;
use crate::orchestrator::Orchestrator;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

/// Проверить существование пути через async tokio::fs (не блокирует runtime).
/// `is_file = true` — ожидается обычный файл; `false` — директория.
async fn validate_path(path: &str, is_file: bool) -> Result<()> {
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|_| AppError::Validation(format!("Файл не найден: {path}")))?;

    if is_file && !meta.is_file() {
        return Err(AppError::Validation(format!(
            "Путь не является файлом: {path}"
        )));
    }
    if !is_file && !meta.is_dir() {
        return Err(AppError::Validation(format!(
            "Путь не является папкой: {path}"
        )));
    }
    // Запрещаем открывать исполняемые файлы
    if is_file {
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if matches!(
            ext.to_ascii_lowercase().as_str(),
            "exe" | "bat" | "cmd" | "com" | "ps1"
        ) {
            return Err(AppError::Validation(format!(
                "Запрещено открывать исполняемые файлы: {path}"
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_history(
    before_id: Option<String>,
    limit: i64,
    query: Option<String>,
    album_id: Option<String>,
    favorite: Option<bool>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<Vec<HistoryItem>> {
    let orch = orchestrator.lock().await;
    orch.db
        .get_history(before_id, limit, query, album_id, favorite)
        .await
}

#[tauri::command]
pub async fn delete_history_item(
    id: String,
    with_file: bool,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    // При with_file — удаляем сам файл (если он есть), но запись в БД
    // всё равно мягко удаляется (deleted_at), чтобы можно было восстановить.
    if with_file {
        let file_path: Option<String> =
            sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = ? LIMIT 1")
                .bind(&id)
                .fetch_optional(&orch.db.pool)
                .await
                .unwrap_or(None);
        if let Some(path) = file_path.filter(|p| !p.is_empty()) {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    orch.db.delete_history_item(&id).await?;
    orch.emit_history_updated();
    orch.thought_history_delete();
    Ok(())
}

#[tauri::command]
pub async fn restore_history_item(
    id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.restore_history_item(&id).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn purge_deleted(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.purge_deleted().await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn delete_history_items(
    ids: Vec<String>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.delete_history_items(&ids).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn set_favorite(
    ids: Vec<String>,
    value: bool,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.set_favorite(&ids, value).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn create_album(
    name: String,
    kind: String,
    query: Option<String>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<String> {
    let orch = orchestrator.lock().await;
    let id = orch.db.create_album(name, kind, query).await?;
    orch.emit_history_updated();
    Ok(id)
}

#[tauri::command]
pub async fn list_albums(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<Vec<Album>> {
    let orch = orchestrator.lock().await;
    orch.db.list_albums().await
}

#[tauri::command]
pub async fn rename_album(
    id: String,
    name: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.rename_album(&id, &name).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn delete_album(
    id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.delete_album(&id).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn add_to_album(
    album_id: String,
    ids: Vec<String>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.add_to_album(&album_id, &ids).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn remove_from_album(
    album_id: String,
    ids: Vec<String>,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.remove_from_album(&album_id, &ids).await?;
    orch.emit_history_updated();
    Ok(())
}

#[tauri::command]
pub async fn get_album_items(
    album_id: String,
    before_id: Option<String>,
    limit: i64,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<Vec<HistoryItem>> {
    let orch = orchestrator.lock().await;
    orch.db.get_album_items(&album_id, before_id, limit).await
}

#[tauri::command]
pub async fn generate_thumbnail(
    id: String,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.generate_thumbnail(&id).await?;
    orch.emit_history_updated();
    Ok(())
}

/// Пересоздать локальные превью для всех скачанных записей в фоне.
/// Вертикальные видео (rotation-метаданные) получат портретные превью.
/// После каждой успешной генерации шлём history:updated — сетка пересобирается.
#[tauri::command]
pub async fn backfill_thumbnails(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<()> {
    let orch_arc = (*orchestrator).clone();
    let db = {
        let orch = orch_arc.lock().await;
        orch.db.clone()
    };
    tauri::async_runtime::spawn(async move {
        let ids = match db.all_history_ids().await {
            Ok(ids) => ids,
            Err(e) => {
                tracing::warn!("backfill_thumbnails: список id не получен: {e}");
                return;
            }
        };
        let mut changed = false;
        for id in ids {
            if db.generate_thumbnail(&id).await.is_ok() {
                changed = true;
            }
        }
        // Один сигнал на всю пачку, а не на каждый файл — иначе фронтенд
        // перезагружает историю столько раз, сколько записей в архиве.
        if changed {
            let orch = orch_arc.lock().await;
            orch.emit_history_updated();
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn get_media_url(
    id: String,
    app: AppHandle,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<String> {
    let orch = orchestrator.lock().await;
    let path = orch.db.get_media_url(&id).await;
    // Файл может лежать вне статического asset-scope (кастомная папка загрузок,
    // другой регистр буквы диска и т.п.). Разрешаем сам конкретный файл прямо
    // при запросе — иначе WebView не отдаст его и лайтбокс покажет
    // «Файл не найден», хотя файл на диске есть.
    if !path.is_empty() {
        let _ = app
            .asset_protocol_scope()
            .allow_file(std::path::Path::new(&path));
    }
    Ok(path)
}

#[tauri::command]
pub async fn clear_history(orchestrator: State<'_, Arc<Mutex<Orchestrator>>>) -> Result<()> {
    let orch = orchestrator.lock().await;
    orch.db.clear_history().await?;
    orch.thought_history_clear();
    Ok(())
}

#[tauri::command]
pub async fn open_file(
    path: String,
    app: AppHandle,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    validate_path(&path, true).await?;
    app.opener()
        .open_path(&path, None::<&str>)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
    orchestrator.lock().await.thought_open_file();
    Ok(())
}

#[cfg(target_os = "windows")]
/// Открыть проводник на нужном файле/папке и сразу выделить его.
/// Используем SHOpenFolderAndSelectItems — единственный надёжный способ
/// выделить файл, когда проводник уже запущен (explorer /select через
/// CreateProcess в этом случае часто открывает не ту папку).
fn reveal_in_explorer(path: &str) {
    use std::ffi::c_void;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;

    // Объявляем нужные Win32-функции напрямую — без зависимости от windows-sys
    // (в 0.52 нужные символы недоступны из-за отсутствующего feature-флага).
    extern "system" {
        fn ILCreateFromPathW(pszpath: *const u16) -> *mut c_void;
        fn SHOpenFolderAndSelectItems(
            pidlfolder: *const c_void,
            cidl: u32,
            apidl: *const *const c_void,
            dwflags: u32,
        ) -> i32;
        fn CoTaskMemFree(pv: *const c_void);
    }

    // Директорию просто открываем (выделять нечего) — explorer с этим справляется надёжно.
    if std::path::Path::new(path).is_dir() {
        let _ = std::process::Command::new("explorer.exe")
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .arg(path)
            .status();
        return;
    }

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // Безопасно: поток уже в STA-апартаменте (WebView), COM инициализирован Tauri.
    let pidl = unsafe { ILCreateFromPathW(wide.as_ptr()) };
    if !pidl.is_null() {
        unsafe {
            let _ = SHOpenFolderAndSelectItems(pidl, 0, std::ptr::null(), 0);
            CoTaskMemFree(pidl);
        }
    }
}

#[tauri::command]
pub async fn open_folder(
    path: String,
    app: AppHandle,
    orchestrator: State<'_, Arc<Mutex<Orchestrator>>>,
) -> Result<()> {
    // На Windows открываем проводник сразу с выделением файла.
    #[cfg(target_os = "windows")]
    {
        let _ = &app;
        reveal_in_explorer(&path);
        orchestrator.lock().await.thought_open_folder();
        return Ok(());
    }

    // Прочие ОС: открываем родительскую папку обычным способом
    #[cfg(not(target_os = "windows"))]
    {
        let p = std::path::Path::new(&path);
        let is_file = tokio::fs::metadata(&path)
            .await
            .map(|m| m.is_file())
            .unwrap_or(false);
        let folder = if is_file {
            p.parent().unwrap_or(p).to_string_lossy().to_string()
        } else {
            path.clone()
        };
        validate_path(&folder, false).await?;
        app.opener()
            .open_path(&folder, None::<&str>)
            .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
        orchestrator.lock().await.thought_open_folder();
        Ok(())
    }
}
