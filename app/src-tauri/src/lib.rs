mod commands;
mod db;
mod downloader;
mod error;
mod logging;
mod orchestrator;
mod queue;
mod ytdlp;

use commands::{
    channel_prefs as channel_prefs_cmds, download, history, queue as queue_cmds,
    session as session_cmds, settings, shortcuts as shortcuts_cmds, support as support_cmds,
    update as update_cmds,
};
use orchestrator::{Orchestrator, OrchestratorHandle};
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub fn run() {
    // Логирование: JSON в файл с ротацией по размеру + panic-hook.
    // guard должен жить до конца run() — его Drop сбрасывает буфер.
    let _guard = logging::init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            // Live-лог: сохраняем handle, чтобы эмитить события `log:line`.
            logging::set_app_handle(&handle);
            tauri::async_runtime::block_on(async {
                // Прокси из настроек — для самообновления yt-dlp (заполняется ниже)
                let mut update_proxy: Option<String> = None;
                let mut db = db::Database::connect().await?;
                db.migrate().await?;

                // Галерея: папка превью + директории загрузок в asset-протокол,
                // чтобы WebView мог отдавать локальные превью и видео через
                // asset://localhost. Директории загрузок пользовательские —
                // читаем из настроек и добавляем динамически (статичный scope
                // в tauri.conf.json покрывает только известные пути). Делаем
                // ДО передачи db в оркестратор (db переезжает в Orchestrator::new).
                let thumbs = db::thumbs_dir();
                let _ = std::fs::create_dir_all(&thumbs);
                let scope = app.asset_protocol_scope();
                let _ = scope.allow_directory(&thumbs, true);
                // Стандартная папка Downloads (страховка если $HOME не резолвится)
                if let Some(dl) = dirs_next::download_dir() {
                    let _ = scope.allow_directory(&dl, true);
                }
                if let Ok(settings) = db.get_settings().await {
                    for d in [
                        settings.download_dir,
                        settings.save_dir_video,
                        settings.save_dir_audio,
                        settings.save_dir_playlist,
                        settings.save_dir_shorts,
                        settings.save_dir_trimmed,
                    ]
                    .into_iter()
                    .filter(|d| !d.is_empty())
                    {
                        let _ = scope.allow_directory(std::path::Path::new(&d), true);
                    }
                    // Прокси для самообновления yt-dlp при старте
                    if !settings.proxy.is_empty() {
                        update_proxy = Some(settings.proxy.clone());
                    }
                }

                let orch_arc: OrchestratorHandle =
                    Arc::new(Mutex::new(Orchestrator::new(db, handle).await?));
                // Запускаем задачи восстановленные из БД
                Orchestrator::start(Arc::clone(&orch_arc)).await;
                // Фоновый task: после завершения воркера запускает tick()
                let orch_tick = Arc::clone(&orch_arc);
                let notify = Arc::clone(&orch_arc.lock().await.tick_notify);
                tauri::async_runtime::spawn(async move {
                    loop {
                        notify.notified().await;
                        Orchestrator::tick(Arc::clone(&orch_tick)).await;
                    }
                });
                app.manage(orch_arc);

                // Самообновление yt-dlp при старте (не блокирует запуск).
                // Берём yt-dlp из оркестратора и прокси из настроек.
                let ytdlp = {
                    let orch = app.state::<OrchestratorHandle>().inner().lock().await;
                    orch.ytdlp()
                };
                let proxy = update_proxy.clone();
                tauri::async_runtime::spawn(async move {
                    ytdlp.self_update(proxy.as_deref()).await;
                });

                Ok::<_, anyhow::Error>(())
            })?;

            // Graceful shutdown при закрытии окна: даём воркерам шанс завершиться.
            // В Tauri v2 подписываемся через WebviewWindow::on_window_event.
            if let Some(main_window) = app.get_webview_window("main") {
                let orch_shutdown: OrchestratorHandle =
                    app.state::<OrchestratorHandle>().inner().clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let orch = Arc::clone(&orch_shutdown);
                        // Отправляем cancel всем воркерам — fire-and-forget здесь допустим:
                        // shutdown() только отправляет oneshot::send, это мгновенно.
                        tauri::async_runtime::spawn(async move {
                            orch.lock().await.shutdown();
                        });
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            download::fetch_info,
            download::start_download,
            download::cancel_download,
            queue_cmds::get_queue,
            queue_cmds::reorder_task,
            queue_cmds::retry_task,
            queue_cmds::remove_task,
            queue_cmds::clear_queue,
            queue_cmds::set_task_priority,
            history::get_history,
            history::delete_history_item,
            history::restore_history_item,
            history::purge_deleted,
            history::delete_history_items,
            history::set_favorite,
            history::create_album,
            history::list_albums,
            history::rename_album,
            history::delete_album,
            history::add_to_album,
            history::remove_from_album,
            history::get_album_items,
            history::generate_thumbnail,
            history::backfill_thumbnails,
            history::get_media_url,
            history::clear_history,
            history::open_file,
            history::open_folder,
            settings::get_settings,
            settings::update_settings,
            settings::get_setting,
            settings::get_free_space,
            channel_prefs_cmds::list_channel_prefs,
            channel_prefs_cmds::upsert_channel_prefs,
            channel_prefs_cmds::delete_channel_prefs,
            session_cmds::get_session,
            session_cmds::update_session,
            update_cmds::install_version,
            support_cmds::upload_log,
            support_cmds::set_log_level,
            support_cmds::get_log_history,
            support_cmds::log_frontend,
            shortcuts_cmds::list_shortcuts,
            shortcuts_cmds::fix_shortcut,
            shortcuts_cmds::remove_shortcut,
            shortcuts_cmds::set_shortcut,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
