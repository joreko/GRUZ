use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

// DETACHED_PROCESS: дочерний процесс не наследует консоль и живёт независимо
const DETACHED_PROCESS: u32 = 0x00000008;
// CREATE_NO_WINDOW используется только для служебных вспомогательных процессов
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Встраиваем все payload-файлы прямо в бинарник
static GRUZ_EXE:   &[u8] = include_bytes!("../payload/gruz.exe");
static YTDLP_EXE:  &[u8] = include_bytes!("../payload/yt-dlp.exe");
static FFMPEG_EXE: &[u8] = include_bytes!("../payload/ffmpeg.exe");
static GRUZ_ICO:   &[u8] = include_bytes!("../payload/gruz.ico");

// Суммарный размер payload + 10% запас для проверки свободного места
const NEEDED_BYTES: u64 = {
    let total = GRUZ_EXE.len() + YTDLP_EXE.len() + FFMPEG_EXE.len() + GRUZ_ICO.len();
    // +10% запас
    total as u64 + total as u64 / 10
};

const VERSION:  &str = env!("CARGO_PKG_VERSION");
const REG_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Gruz";
const REG_RUN:  &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

// ─── Типы ─────────────────────────────────────────────────────────────────────

#[derive(Clone, serde::Serialize)]
pub struct Progress {
    pub step:  String,
    pub pct:   u8,
    pub done:  bool,
    pub error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct InstallState {
    pub installed:    bool,
    pub version:      Option<String>,
    pub install_dir:  String,
    pub same_version: bool,
}

/// Расширенные опции установки — все поля опциональны, дефолты разумны
#[derive(serde::Deserialize)]
#[allow(dead_code)] // поля читаются в lib.rs (silent) и через serde десериализацию
pub struct InstallOptions {
    pub mode:             Option<String>,  // "install" | "upgrade"
    pub install_dir:      Option<String>,  // None = дефолт
    pub desktop_shortcut: Option<bool>,    // None = true
    pub add_to_path:      Option<bool>,    // None = false
    pub autostart:        Option<bool>,    // None = false
    pub launch_after:     Option<bool>,    // None = true
    pub silent: Option<bool>,    // None = false; зарезервировано для silent-режима в lib.rs
}

// ─── Helpers: системные пути ───────────────────────────────────────────────────

/// Дефолтная папка установки; возвращает Err если %LOCALAPPDATA% недоступен
pub fn install_dir() -> anyhow::Result<PathBuf> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| anyhow::anyhow!("переменная %LOCALAPPDATA% недоступна"))?;
    Ok(PathBuf::from(local).join("Gruz"))
}

/// Папка БД — %APPDATA%\Gruz, отдельно от папки установки
fn db_dir() -> anyhow::Result<PathBuf> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| anyhow::anyhow!("переменная %APPDATA% недоступна"))?;
    Ok(PathBuf::from(appdata).join("Gruz"))
}

/// Читает InstallLocation из реестра — для удаления, независимо от дефолтного пути
fn reg_read_install_location() -> Option<PathBuf> {
    use winreg::{enums::*, RegKey};
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(REG_PATH).ok()?;
    let loc: String = key.get_value("InstallLocation").ok()?;
    Some(PathBuf::from(loc))
}

fn reg_read_version() -> Option<String> {
    use winreg::{enums::*, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(REG_PATH).ok()?
        .get_value("DisplayVersion").ok()
}

fn emit(app: &AppHandle, p: Progress) {
    let _ = app.emit("install_progress", p);
}

/// PowerShell вспомогательная функция — проверяет статус и возвращает stderr как ошибку
fn run_ps(script: &str) -> anyhow::Result<()> {
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if !stderr.trim().is_empty() { stderr } else { stdout };
        anyhow::bail!("PowerShell ошибка: {}", msg.trim());
    }
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('\'', "''")
}

fn dir_size_kb(dir: &PathBuf) -> u32 {
    let bytes: u64 = std::fs::read_dir(dir)
        .into_iter().flatten().flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum();
    (bytes / 1024) as u32
}

fn clean_db_files() -> anyhow::Result<()> {
    let dir = db_dir()?;
    for name in &["gruz.db", "gruz.db-wal", "gruz.db-shm"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    Ok(())
}

// ─── SHA256 верификация ────────────────────────────────────────────────────────

/// Проверяем что записанный файл соответствует embedded данным
fn verify_written_file(path: &PathBuf, data: &[u8]) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    let expected = Sha256::digest(data);
    let on_disk  = std::fs::read(path)?;
    let actual   = Sha256::digest(&on_disk);
    anyhow::ensure!(
        expected == actual,
        "SHA256 не совпадает для {}: ожидалось {}, получено {}",
        path.display(), hex_of(expected.as_slice()), hex_of(actual.as_slice())
    );
    Ok(())
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Записать файл и сразу проверить SHA256
fn write_and_verify(data: &[u8], dest: &PathBuf) -> anyhow::Result<()> {
    std::fs::write(dest, data)?;
    verify_written_file(dest, data)?;
    Ok(())
}

// ─── Preflight ─────────────────────────────────────────────────────────────────

/// Пробует создать и удалить тестовый файл — проверка прав на запись
fn check_write_permission(dir: &PathBuf) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(".gruz-write-probe");
    std::fs::write(&probe, b"probe")
        .map_err(|e| anyhow::anyhow!("нет прав на запись в {}: {e}", dir.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// GetDiskFreeSpaceExW — проверяет свободное место на томе
fn check_disk_space(dir: &PathBuf, needed: u64) -> anyhow::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows_sys::Win32::Foundation::BOOL;

    std::fs::create_dir_all(dir)?;

    // Путь в wide-строку с нулём
    let wide: Vec<u16> = dir.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut free_caller: u64 = 0;
    let mut total: u64 = 0;
    let mut free_total: u64 = 0;

    let ok: BOOL = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_caller as *mut u64,
            &mut total as *mut u64,
            &mut free_total as *mut u64,
        )
    };

    anyhow::ensure!(ok != 0, "не удалось получить размер диска для {}", dir.display());
    anyhow::ensure!(
        free_caller >= needed,
        "недостаточно места: нужно {} МБ, доступно {} МБ",
        needed / 1_048_576,
        free_caller / 1_048_576
    );
    Ok(())
}

/// Проверяет запущен ли gruz.exe через tasklist
pub fn is_gruz_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq gruz.exe", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("gruz.exe"))
        .unwrap_or(false)
}

// ─── WebView2 ─────────────────────────────────────────────────────────────────

fn webview2_installed() -> bool {
    use winreg::{enums::*, RegKey};
    let paths = [
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
        (HKEY_CURRENT_USER,  "SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"),
    ];
    for (hive, path) in &paths {
        if RegKey::predef(*hive).open_subkey(path).is_ok() {
            return true;
        }
    }
    false
}

async fn check_webview2(app: &AppHandle) -> anyhow::Result<()> {
    if webview2_installed() {
        return Ok(());
    }
    emit(app, Progress { step: "Устанавливаю WebView2…".into(), pct: 4, done: false, error: None });
    let tmp = std::env::temp_dir().join("MicrosoftEdgeWebview2Setup.exe");
    run_ps(&format!(
        "Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile '{}'",
        escape(&tmp.to_string_lossy())
    ))?;
    anyhow::ensure!(tmp.exists(), "не удалось скачать WebView2");
    std::process::Command::new(&tmp)
        .args(["/silent", "/install"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

// ─── Ярлыки через mslnk ────────────────────────────────────────────────────────

/// Запускает PowerShell скрипт через временный .ps1 файл (обходит проблемы с кавычками в -Command)
fn run_ps_script(script: &str) -> anyhow::Result<()> {
    let tmp = std::env::temp_dir().join(format!("gruz_{}.ps1", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, script.as_bytes())?;
    let result = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-File",
               tmp.to_str().unwrap_or("")])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = std::fs::remove_file(&tmp);
    let output = result?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if !stderr.trim().is_empty() { stderr } else { stdout };
        anyhow::bail!("PowerShell ошибка: {}", msg.trim());
    }
    Ok(())
}

fn create_shortcuts(install_dir: &PathBuf, desktop: bool) -> anyhow::Result<()> {
    let exe = install_dir.join("gruz.exe");
    anyhow::ensure!(exe.exists(), "gruz.exe не найден в {}", install_dir.display());

    let ico = install_dir.join("gruz.ico");
    let exe_str = exe.to_string_lossy();
    let ico_str = ico.to_string_lossy();

    let appdata = std::env::var("APPDATA")
        .map_err(|_| anyhow::anyhow!("%APPDATA% недоступен"))?;
    let start = PathBuf::from(&appdata)
        .join("Microsoft\\Windows\\Start Menu\\Programs\\Груз.lnk");
    if let Some(parent) = start.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let start_str = start.to_string_lossy();

    let mut script = format!(
        "$s=(New-Object -COM WScript.Shell).CreateShortcut('{start_str}')\n\
         $s.TargetPath='{exe_str}'\n\
         $s.IconLocation='{ico_str},0'\n\
         $s.Description='Груз — загрузчик видео'\n\
         $s.Save()\n"
    );

    if desktop {
        let userprofile = std::env::var("USERPROFILE")
            .map_err(|_| anyhow::anyhow!("%USERPROFILE% недоступен"))?;
        let desk = PathBuf::from(&userprofile).join("Desktop\\Груз.lnk");
        let desk_str = desk.to_string_lossy();
        script.push_str(&format!(
            "$s2=(New-Object -COM WScript.Shell).CreateShortcut('{desk_str}')\n\
             $s2.TargetPath='{exe_str}'\n\
             $s2.IconLocation='{ico_str},0'\n\
             $s2.Description='Груз — загрузчик видео'\n\
             $s2.Save()\n"
        ));
    }

    run_ps_script(&script)
}

fn remove_shortcuts() {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let _ = std::fs::remove_file(
        PathBuf::from(&appdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Груз.lnk")
    );
    let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
    let _ = std::fs::remove_file(PathBuf::from(&userprofile).join("Desktop\\Груз.lnk"));
}

// ─── Реестр ────────────────────────────────────────────────────────────────────

fn write_registry(install_dir: &PathBuf) -> anyhow::Result<()> {
    use winreg::{enums::*, RegKey};
    use chrono::Local;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(REG_PATH)?;

    let ico = install_dir.join("gruz.ico");
    let uninstall_exe = install_dir.join("gruz-setup.exe");
    let size_kb: u32 = dir_size_kb(install_dir);
    let install_date = Local::now().format("%Y%m%d").to_string();

    key.set_value("DisplayName",     &"Груз — загрузчик видео")?;
    key.set_value("DisplayVersion",  &VERSION)?;
    key.set_value("Publisher",       &"joreko")?;
    key.set_value("URLInfoAbout",    &"https://github.com/joreko/GRUZ")?;
    key.set_value("InstallLocation", &install_dir.to_string_lossy().as_ref())?;
    key.set_value("DisplayIcon",     &format!("{},0", ico.to_string_lossy()))?;
    key.set_value("UninstallString", &uninstall_exe.to_string_lossy().as_ref())?;
    key.set_value("EstimatedSize",   &size_kb)?;
    key.set_value("InstallDate",     &install_date.as_str())?;
    key.set_value("NoModify",        &1u32)?;
    key.set_value("NoRepair",        &1u32)?;

    Ok(())
}

fn remove_registry() {
    use winreg::{enums::*, RegKey};
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(REG_PATH);
}

// ─── PATH / автозапуск / ассоциации файлов ─────────────────────────────────────

/// Добавляет директорию в HKCU\Environment\PATH и рассылает WM_SETTINGCHANGE
fn add_to_path(dir: &PathBuf) -> anyhow::Result<()> {
    use winreg::{enums::*, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;

    let current: String = env.get_value("PATH").unwrap_or_default();
    let dir_str = dir.to_string_lossy();

    // Не добавлять дважды
    if current.split(';').any(|p| p.eq_ignore_ascii_case(dir_str.as_ref())) {
        return Ok(());
    }

    let new_path = if current.is_empty() {
        dir_str.into_owned()
    } else {
        format!("{current};{dir_str}")
    };
    env.set_value("PATH", &new_path.as_str())?;

    // Уведомить оболочку об изменении переменных окружения
    broadcast_settings_change();
    Ok(())
}

fn remove_from_path(dir: &PathBuf) {
    use winreg::{enums::*, RegKey};

    let Ok(env) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) else { return };

    let current: String = env.get_value("PATH").unwrap_or_default();
    let dir_str = dir.to_string_lossy();

    let new_path: Vec<&str> = current.split(';')
        .filter(|p| !p.eq_ignore_ascii_case(dir_str.as_ref()))
        .collect();
    let _ = env.set_value("PATH", &new_path.join(";").as_str());
    broadcast_settings_change();
}

/// SendMessageTimeoutW для рассылки WM_SETTINGCHANGE всем окнам
fn broadcast_settings_change() {
    // WM_SETTINGCHANGE = 0x001A, HWND_BROADCAST = 0xFFFF, SMTO_ABORTIFHUNG = 0x0002
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x0002;
    const HWND_BROADCAST: isize = 0xFFFF;

    // "Environment\0" в wide
    let env: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result: usize = 0;

    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env.as_ptr() as _,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

fn set_autostart(enable: bool, exe_path: &PathBuf) -> anyhow::Result<()> {
    use winreg::{enums::*, RegKey};

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(REG_RUN, KEY_READ | KEY_WRITE)?;

    if enable {
        key.set_value("Gruz", &exe_path.to_string_lossy().as_ref())?;
    } else {
        let _ = key.delete_value("Gruz");
    }
    Ok(())
}

/// Ассоциировать .mp4 .webm .mkv .mp3 .m4a с Грузом
#[allow(dead_code)] // будет вызвана когда UI добавит опцию register_associations
fn register_file_associations(exe_path: &PathBuf) -> anyhow::Result<()> {
    use winreg::{enums::*, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exe_str = exe_path.to_string_lossy();
    let cmd = format!("\"{}\" \"%1\"", exe_str);

    // Регистрируем ProgId
    let (prog_id, _) = hkcu.create_subkey("Software\\Classes\\Gruz.MediaFile")?;
    prog_id.set_value("", &"Медиафайл Груз")?;
    let (open_cmd, _) = hkcu.create_subkey("Software\\Classes\\Gruz.MediaFile\\shell\\open\\command")?;
    open_cmd.set_value("", &cmd.as_str())?;

    // Ассоциируем расширения
    for ext in &[".mp4", ".webm", ".mkv", ".mp3", ".m4a"] {
        let (key, _) = hkcu.create_subkey(format!("Software\\Classes\\{ext}"))?;
        key.set_value("", &"Gruz.MediaFile")?;
    }
    Ok(())
}

fn unregister_file_associations() {
    use winreg::{enums::*, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all("Software\\Classes\\Gruz.MediaFile");
    for ext in &[".mp4", ".webm", ".mkv", ".mp3", ".m4a"] {
        // Удаляем только если они ссылались на нас
        if let Ok(key) = hkcu.open_subkey(format!("Software\\Classes\\{ext}")) {
            let val: String = key.get_value("").unwrap_or_default();
            if val == "Gruz.MediaFile" {
                let _ = hkcu.delete_subkey_all(format!("Software\\Classes\\{ext}"));
            }
        }
    }
}

// ─── Надёжный self-delete через MoveFileExW ────────────────────────────────────

/// Планирует удаление файла/папки после перезагрузки через MoveFileExW
fn schedule_delete_on_reboot(path: &PathBuf) {
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};

    let wide: Vec<u16> = path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT);
    }
}

// ─── Атомарная установка ───────────────────────────────────────────────────────

/// Создаёт staging-директорию в %TEMP%
fn make_staging_dir() -> anyhow::Result<PathBuf> {
    let id = uuid::Uuid::new_v4();
    let staging = std::env::temp_dir().join(format!("gruz-staging-{id}"));
    std::fs::create_dir_all(&staging)?;
    Ok(staging)
}

/// Проверяет что два пути на одном томе Windows (первая буква пути)
fn same_volume(a: &PathBuf, b: &PathBuf) -> bool {
    let a_str = a.to_string_lossy();
    let b_str = b.to_string_lossy();
    // Сравниваем первую букву (том) без учёта регистра
    a_str.chars().next().map(|c| c.to_ascii_lowercase())
        == b_str.chars().next().map(|c| c.to_ascii_lowercase())
}

// ─── Команды ──────────────────────────────────────────────────────────────────

/// Проверить текущее состояние установки
#[tauri::command]
pub fn check_install() -> InstallState {
    let dir = install_dir().unwrap_or_else(|_| PathBuf::from("C:\\Gruz"));
    let exe = dir.join("gruz.exe");
    // installed = файл существует И запись в реестре есть
    let installed = exe.exists() && reg_read_version().is_some();
    let version = reg_read_version();
    let same_version = version.as_deref() == Some(VERSION);
    InstallState {
        installed,
        version,
        install_dir: dir.to_string_lossy().into_owned(),
        same_version,
    }
}

#[tauri::command]
pub fn get_install_dir() -> Result<String, String> {
    install_dir()
        .map(|d| d.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

/// Проверить запущен ли gruz.exe — для UI
#[tauri::command]
pub fn check_gruz_running() -> bool {
    is_gruz_running()
}

/// Завершить gruz.exe принудительно — вызывается из UI перед install
#[tauri::command]
pub async fn kill_gruz(app: AppHandle) -> Result<(), String> {
    // taskkill /F — принудительное завершение
    let output = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "gruz.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("не удалось выполнить taskkill: {e}"))?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr);
        // Если процесс не найден — это нормально
        if !msg.contains("не найден") && !msg.contains("not found") && !msg.contains("No tasks") {
            return Err(format!("taskkill: {}", msg.trim()));
        }
    }

    // Ждём завершения процесса
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    emit(&app, Progress { step: "Груз завершён. Продолжаю…".into(), pct: 2, done: false, error: None });
    Ok(())
}

/// Основная установка
#[tauri::command]
pub async fn install(app: AppHandle, opts: InstallOptions) -> Result<(), String> {
    tokio::spawn(async move {
        if let Err(e) = do_install(&app, opts).await {
            emit(&app, Progress {
                step:  format!("Ошибка: {e}"),
                pct:   0,
                done:  false,
                error: Some(e.to_string()),
            });
        }
    });
    Ok(())
}

async fn do_install(app: &AppHandle, opts: InstallOptions) -> anyhow::Result<()> {
    // Разрешаем путь установки: из опций или дефолтный
    let dir: PathBuf = match opts.install_dir.as_deref().filter(|s| !s.is_empty()) {
        Some(p) => PathBuf::from(p),
        None    => install_dir()?,
    };

    let desktop_shortcut = opts.desktop_shortcut.unwrap_or(true);
    let add_path         = opts.add_to_path.unwrap_or(false);
    let autostart        = opts.autostart.unwrap_or(false);
    let launch_after     = opts.launch_after.unwrap_or(true);

    // ── Preflight ────────────────────────────────────────────────────────────
    emit(app, Progress { step: "Preflight…".into(), pct: 2, done: false, error: None });

    // 1. Проверка что gruz.exe не запущен
    if is_gruz_running() {
        anyhow::bail!("__GRUZ_RUNNING__"); // специальный маркер для UI
    }

    // 2. Проверка прав на запись
    check_write_permission(&dir)?;

    // 3. Проверка свободного места
    check_disk_space(&dir, NEEDED_BYTES)?;

    // ── WebView2 ─────────────────────────────────────────────────────────────
    emit(app, Progress { step: "Проверяю систему…".into(), pct: 4, done: false, error: None });
    check_webview2(app).await?;

    // ── Staging ──────────────────────────────────────────────────────────────
    emit(app, Progress { step: "Подготовка…".into(), pct: 8, done: false, error: None });

    let staging = make_staging_dir()?;

    // При любой ошибке — очищаем staging
    let result = do_install_to_staging(&staging, app).await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return result;
    }

    // ── Финализация: staging → install_dir ───────────────────────────────────
    emit(app, Progress { step: "Финализирую…".into(), pct: 88, done: false, error: None });

    if dir.exists() {
        // При upgrade/переустановке — удаляем только файлы программы, не папку
        // Это сохраняет подпапки с данными (если вдруг есть)
        for name in &["gruz.exe", "yt-dlp.exe", "ffmpeg.exe", "gruz.ico", "gruz-setup.exe"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }

    if same_volume(&staging, &dir) {
        // Атомарное переименование на одном томе
        // rename поверх существующей директории на Windows работает только если целевая пуста
        // поэтому копируем файлы из staging в dir
        std::fs::create_dir_all(&dir)?;
        for entry in std::fs::read_dir(&staging)?.flatten() {
            std::fs::rename(entry.path(), dir.join(entry.file_name()))
                .map_err(|e| anyhow::anyhow!("не удалось переместить {}: {e}", entry.path().display()))?;
        }
        let _ = std::fs::remove_dir_all(&staging);
    } else {
        // Fallback: разные тома — обычная копия
        std::fs::create_dir_all(&dir)?;
        for entry in std::fs::read_dir(&staging)?.flatten() {
            std::fs::copy(entry.path(), dir.join(entry.file_name()))?;
        }
        let _ = std::fs::remove_dir_all(&staging);
    }

    // ── Постустановочные шаги ─────────────────────────────────────────────────
    // Ярлыки создаём ТОЛЬКО после переноса staging → install_dir,
    // иначе gruz.exe ещё нет по финальному пути и проверка exists() падает.
    emit(app, Progress { step: "Создаю ярлыки…".into(), pct: 89, done: false, error: None });
    create_shortcuts(&dir, desktop_shortcut)?;

    emit(app, Progress { step: "Регистрирую в системе…".into(), pct: 92, done: false, error: None });
    write_registry(&dir)?;

    if add_path {
        add_to_path(&dir)?;
    }
    if autostart {
        set_autostart(true, &dir.join("gruz.exe"))?;
    }

    // БД чистим только при чистой установке/переустановке, не при upgrade
    if opts.mode.as_deref() != Some("upgrade") {
        let _ = clean_db_files(); // не фатально если не получилось
    }

    emit(app, Progress { step: "Готово.".into(), pct: 100, done: true, error: None });

    // Запустить Груз после установки
    if launch_after {
        let exe = dir.join("gruz.exe");
        if exe.exists() {
            let _ = std::process::Command::new(&exe)
                .creation_flags(DETACHED_PROCESS)
                .spawn();
        }
    }

    Ok(())
}

/// Записывает все файлы в staging-директорию
async fn do_install_to_staging(
    staging: &PathBuf,
    app: &AppHandle,
) -> anyhow::Result<()> {
    emit(app, Progress { step: "Копирую Груз…".into(), pct: 15, done: false, error: None });
    write_and_verify(GRUZ_EXE, &staging.join("gruz.exe"))?;

    emit(app, Progress { step: "Копирую yt-dlp…".into(), pct: 35, done: false, error: None });
    write_and_verify(YTDLP_EXE, &staging.join("yt-dlp.exe"))?;

    emit(app, Progress { step: "Копирую ffmpeg…".into(), pct: 55, done: false, error: None });
    write_and_verify(FFMPEG_EXE, &staging.join("ffmpeg.exe"))?;

    write_and_verify(GRUZ_ICO, &staging.join("gruz.ico"))?;

    // Копируем установщик как средство удаления — из текущего exe в staging
    let self_exe = std::env::current_exe()?;
    std::fs::copy(&self_exe, staging.join("gruz-setup.exe"))?;

    Ok(())
}

/// Удаление
#[tauri::command]
pub async fn uninstall(app: AppHandle) -> Result<(), String> {
    tokio::spawn(async move {
        if let Err(e) = do_uninstall(&app).await {
            emit(&app, Progress {
                step:  format!("Ошибка: {e}"),
                pct:   0,
                done:  false,
                error: Some(e.to_string()),
            });
        }
    });
    Ok(())
}

async fn do_uninstall(app: &AppHandle) -> anyhow::Result<()> {
    // Читаем путь из реестра — не хардкодим
    let dir = reg_read_install_location()
        .unwrap_or_else(|| install_dir().unwrap_or_else(|_| PathBuf::from("C:\\Gruz")));

    emit(app, Progress { step: "Удаляю ярлыки…".into(), pct: 20, done: false, error: None });
    remove_shortcuts();

    emit(app, Progress { step: "Удаляю из реестра…".into(), pct: 40, done: false, error: None });
    remove_registry();

    // Убираем автозапуск и ассоциации файлов
    set_autostart(false, &dir.join("gruz.exe")).ok();
    unregister_file_associations();
    remove_from_path(&dir);

    emit(app, Progress { step: "Удаляю файлы…".into(), pct: 60, done: false, error: None });

    // Удаляем все файлы кроме себя (деинсталлятора)
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "gruz-setup.exe" && name != "gruz-uninstall.exe" {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    // Очищаем БД и настройки
    let _ = clean_db_files();

    emit(app, Progress { step: "Удалено.".into(), pct: 100, done: true, error: None });

    // Планируем удаление папки и себя после перезагрузки через MoveFileExW
    schedule_delete_on_reboot(&dir);

    // Fallback: cmd timeout для удаления папки без перезагрузки
    let dir_str = dir.to_string_lossy().to_string();
    if let Err(_) = std::process::Command::new("cmd")
        .args(["/c", &format!(
            "timeout /t 1 /nobreak >nul && rmdir /s /q \"{}\"",
            dir_str
        )])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        // Не фатально — папка удалится при перезагрузке
    }

    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    app.exit(0);
    Ok(())
}

/// Запустить Груз и закрыть установщик
#[tauri::command]
pub async fn launch_gruz(app: AppHandle) -> Result<(), String> {
    let dir = reg_read_install_location()
        .unwrap_or_else(|| install_dir().unwrap_or_else(|_| PathBuf::from("C:\\Gruz")));
    let exe = dir.join("gruz.exe");
    if !exe.exists() {
        return Err(format!("gruz.exe не найден: {}", exe.display()));
    }
    // DETACHED_PROCESS: процесс живёт независимо от установщика
    std::process::Command::new(&exe)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("не удалось запустить: {e}"))?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    app.exit(0);
    Ok(())
}

/// Открыть URL в браузере — только http/https
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| format!("некорректный URL: {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("разрешены только http/https ссылки".into());
    }
    // Открываем через cmd start — безопасно т.к. URL уже провалидирован
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url.as_str()])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Суммарный размер payload в МБ (для отображения в UI)
#[tauri::command]
pub fn get_payload_size_mb() -> u32 {
    // NEEDED_BYTES включает +10% запас — берём точный размер без запаса
    let bytes = GRUZ_EXE.len() + YTDLP_EXE.len() + FFMPEG_EXE.len() + GRUZ_ICO.len();
    (bytes as u64 / 1_048_576) as u32
}

/// Открыть нативный диалог выбора папки; возвращает пустую строку если пользователь отменил
#[tauri::command]
pub async fn pick_install_dir(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()).unwrap_or_default())
}

/// Вернуть changelog — include_str из payload, пустая строка если файла нет при сборке
#[tauri::command]
pub fn get_changelog() -> String {
    // CHANGELOG.md генерируется CI и кладётся в payload/
    // При локальной сборке файла нет — возвращаем пустую строку
    option_env!("GRUZ_CHANGELOG").unwrap_or("").to_string()
}
