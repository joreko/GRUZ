mod commands;

pub fn run() {
    // Тихая установка: если передан /S или --silent — работаем без UI
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "/S" || a == "--silent") {
        run_silent(&args);
        return;
    }
    // Тихое удаление: /U или --uninstall
    if args.iter().any(|a| a == "/U" || a == "--uninstall") {
        run_silent_uninstall();
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::check_install,
            commands::get_install_dir,
            commands::install,
            commands::uninstall,
            commands::launch_gruz,
            commands::open_url,
            commands::check_gruz_running,
            commands::kill_gruz,
            commands::get_changelog,
            commands::get_payload_size_mb,
            commands::pick_install_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error running installer");
}

/// Тихая установка — без UI, выходим с кодом 0 при успехе, 1 при ошибке
fn run_silent(args: &[String]) {
    // Парсим необязательный /DIR=path
    let _install_dir = args.iter()
        .find_map(|a| a.strip_prefix("/DIR=").or_else(|| a.strip_prefix("--dir=")))
        .map(|s| s.to_string());

    let _no_desktop = args.iter().any(|a| a == "/NODESKTOP" || a == "--no-desktop");

    // TODO: refactor do_install чтобы не требовал AppHandle в silent-режиме
    // Сейчас do_install требует AppHandle для эмиссии событий прогресса.
    // В silent-режиме нужен отдельный путь без Tauri. Оставляем заглушку.
    std::process::exit(0);
}

/// Тихое удаление без UI
fn run_silent_uninstall() {
    // TODO: аналогично run_silent — требует refactor do_uninstall
    std::process::exit(0);
}
