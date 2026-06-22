mod commands;
mod db;
mod downloader;
mod error;
mod orchestrator;
mod queue;
mod ytdlp;

use commands::{channel_prefs as channel_prefs_cmds, download, history, queue as queue_cmds, session as session_cmds, settings};
use orchestrator::{Orchestrator, OrchestratorHandle};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn run() {
    // Папка логов
    let log_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gruz")
        .join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    // Удаляем старые файлы — оставляем последние 20
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        let mut files: Vec<_> = entries
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("gruz_")
            })
            .collect();
        files.sort_by_key(|e| e.file_name());
        if files.len() > 20 {
            for old in &files[..files.len() - 20] {
                let _ = std::fs::remove_file(old.path());
            }
        }
    }

    // Файл на каждый запуск: gruz_2026-06-21_143022.log
    let log_name = format!(
        "gruz_{}.log",
        chrono::Local::now().format("%Y-%m-%d_%H%M%S")
    );
    let log_file = std::fs::File::create(log_dir.join(&log_name))
        .expect("не удалось создать файл лога");
    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    let filter = EnvFilter::from_default_env()
        .add_directive("gruz=debug".parse().unwrap());

    // Слой файла — подробный с target и thread id
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true);

    // Слой консоли — только в debug-билде
    #[cfg(debug_assertions)]
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(fmt::layer().with_target(false).with_thread_ids(false));

    #[cfg(not(debug_assertions))]
    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer);

    registry.init();

    // guard должен жить до конца процесса — flush при завершении
    std::mem::forget(guard);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async {
                let mut db = db::Database::connect().await?;
                db.migrate().await?;
                let orchestrator = Orchestrator::new(db, handle).await?;
                let orch_arc: OrchestratorHandle = Arc::new(Mutex::new(orchestrator));
                // Фоновый task: после завершения воркера запускает tick()
                let orch_tick = Arc::clone(&orch_arc);
                let notify = Arc::clone(&orch_arc.lock().await.tick_notify);
                tauri::async_runtime::spawn(async move {
                    loop {
                        notify.notified().await;
                        orch_tick.lock().await.tick().await;
                    }
                });
                app.manage(orch_arc);
                Ok::<_, anyhow::Error>(())
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            download::fetch_info,
            download::start_download,
            download::cancel_download,
            queue_cmds::get_queue,
            queue_cmds::reorder_task,
            queue_cmds::remove_task,
            queue_cmds::set_task_priority,
            history::get_history,
            history::delete_history_item,
            history::clear_history,
            history::open_file,
            history::open_folder,
            settings::get_settings,
            settings::update_settings,
            settings::get_free_space,
            channel_prefs_cmds::list_channel_prefs,
            channel_prefs_cmds::upsert_channel_prefs,
            channel_prefs_cmds::delete_channel_prefs,
            session_cmds::get_session,
            session_cmds::update_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
