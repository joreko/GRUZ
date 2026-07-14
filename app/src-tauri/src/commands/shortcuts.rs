use crate::error::{AppError, Result};
use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;

// CREATE_NO_WINDOW — вспомогательные процессы без видимого окна
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Ожидаемое корректное имя ярлыка (без расширения)
const EXPECTED_NAME: &str = "Груз";
const EXPECTED_DESC: &str = "Груз — загрузчик видео";

/// Один найденный ярлык, указывающий на gruz.exe
#[derive(Clone, Serialize)]
pub struct ShortcutInfo {
    pub name: String,
    pub path: String,
    pub target: String,
    /// 'start_menu' | 'desktop'
    pub location: String,
    /// указывает на gruz.exe
    pub is_gruz: bool,
    /// указывает на gruz.exe, но имя отличается от «Груз» (поломанное/нестандартное)
    pub is_broken: bool,
}

/// Запускает PowerShell-скрипт через временный .ps1 файл с UTF-8 BOM и
/// возвращает stdout. BOM критичен: без него PowerShell на ru-Windows читает
/// файл как CP1251 и портит кириллицу (имя ярлыка превращается в «РісѓР·»).
/// Принудительно ставит UTF-8 на stdout, иначе PowerShell выводит в кодировке
/// системы (CP1251/CP866) и JSON с кириллицей не парсится в Rust.
fn ps_capture(script: &str) -> anyhow::Result<String> {
    let tmp = std::env::temp_dir().join(format!("gruz_sc_{}.ps1", uuid::Uuid::new_v4()));
    let preamble = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8\n";
    let mut bytes = Vec::from(b"\xef\xbb\xbf");
    bytes.extend_from_slice(preamble.as_bytes());
    bytes.extend_from_slice(script.as_bytes());
    std::fs::write(&tmp, &bytes)?;
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            tmp.to_str().unwrap_or(""),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    let _ = std::fs::remove_file(&tmp);
    if !output.status.success() {
        anyhow::bail!(
            "PowerShell ошибка: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Экранирует строку для вставки внутрь одинарных кавычек PowerShell
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// Папка для данного расположения ярлыка (через known folders — без
/// хардкода языка ОС). start_menu → «…\Start Menu\Programs».
fn location_dir(location: &str) -> anyhow::Result<PathBuf> {
    let kind = match location {
        "start_menu" => "StartMenu",
        "desktop" => "Desktop",
        other => anyhow::bail!("неизвестное расположение ярлыка: {other}"),
    };
    let script = format!("Write-Output ([Environment]::GetFolderPath('{kind}'))");
    let out = ps_capture(&script)?.trim().to_string();
    anyhow::ensure!(!out.is_empty(), "не удалось получить папку для {location}");
    let mut dir = PathBuf::from(out);
    if location == "start_menu" {
        dir.push("Programs");
    }
    Ok(dir)
}

/// Перечислить все .lnk, указывающие на gruz.exe (меню Пуск + рабочий стол)
#[tauri::command]
pub async fn list_shortcuts() -> Result<Vec<ShortcutInfo>> {
    let script = r#"
$sh = New-Object -ComObject WScript.Shell
$dirs = @(
  @{ loc = 'start_menu'; path = [Environment]::GetFolderPath('StartMenu') + '\Programs' },
  @{ loc = 'desktop';    path = [Environment]::GetFolderPath('Desktop') }
)
$out = @()
foreach ($d in $dirs) {
  if (Test-Path -LiteralPath $d.path) {
    Get-ChildItem -LiteralPath $d.path -Filter *.lnk -Recurse | ForEach-Object {
      try {
        $lnk = $sh.CreateShortcut($_.FullName)
        $out += [PSCustomObject]@{
          name     = $_.BaseName
          path     = $_.FullName
          target   = $lnk.TargetPath
          location = $d.loc
        }
      } catch {}
    }
  }
}
if ($out.Count -eq 0) { '[]' } else { ConvertTo-Json -Compress -InputObject $out }
"#;

    let raw = ps_capture(script)?.trim().to_string();
    let items: Vec<serde_json::Value> = if raw.is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&raw).unwrap_or_default()
    };

    let mut result = Vec::new();
    for it in items {
        let target = it["target"].as_str().unwrap_or("").to_string();
        let is_gruz = target.to_lowercase().ends_with("gruz.exe");
        if !is_gruz {
            continue;
        }
        let name = it["name"].as_str().unwrap_or("").to_string();
        let is_broken = name != EXPECTED_NAME;
        result.push(ShortcutInfo {
            name,
            path: it["path"].as_str().unwrap_or("").to_string(),
            target,
            location: it["location"].as_str().unwrap_or("").to_string(),
            is_gruz,
            is_broken,
        });
    }
    Ok(result)
}

/// Пересоздать ярлык с корректным именем «Груз» в той же папке,
/// скопировав цель/рабочую папку/иконку из сломанного. Старый удаляется.
#[tauri::command]
pub async fn fix_shortcut(path: String) -> Result<()> {
    if !path.to_lowercase().ends_with(".lnk") {
        return Err(AppError::Validation("ожидается .lnk файл".into()));
    }
    let old = ps_quote(&path);
    let script = format!(
        "$old = '{old}'\n\
         $sh = New-Object -ComObject WScript.Shell\n\
         $lnk = $sh.CreateShortcut($old)\n\
         $dir = Split-Path -Parent $old\n\
         $new = Join-Path $dir '{name}.lnk'\n\
         $n = $sh.CreateShortcut($new)\n\
         $n.TargetPath = $lnk.TargetPath\n\
         $n.WorkingDirectory = $lnk.WorkingDirectory\n\
         $n.IconLocation = $lnk.IconLocation\n\
         $n.Description = '{desc}'\n\
         $n.Save()\n\
         Remove-Item -LiteralPath $old -Force\n",
        old = old,
        name = EXPECTED_NAME,
        desc = EXPECTED_DESC,
    );
    ps_capture(&script)?;
    Ok(())
}

/// Удалить ярлык по пути
#[tauri::command]
pub async fn remove_shortcut(path: String) -> Result<()> {
    if !path.to_lowercase().ends_with(".lnk") {
        return Err(AppError::Validation("ожидается .lnk файл".into()));
    }
    std::fs::remove_file(&path)
        .map_err(|e| anyhow::anyhow!("не удалось удалить ярлык {}: {e}", path))?;
    Ok(())
}

/// Создать или удалить канонический ярлык «Груз» в заданном расположении.
/// enabled=true → создаёт (перезаписывая существующий «Груз.lnk»),
/// enabled=false → удаляет только наш ярлык «Груз.lnk».
#[tauri::command]
pub async fn set_shortcut(location: String, enabled: bool) -> Result<()> {
    let dir = location_dir(&location)?;
    let new_path = dir.join(format!("{EXPECTED_NAME}.lnk"));

    if !enabled {
        // Удаляем только если это наш ярлык «Груз.lnk»
        if new_path.exists() {
            std::fs::remove_file(&new_path).ok();
        }
        return Ok(());
    }

    std::fs::create_dir_all(&dir)?;
    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("не удалось получить путь к exe: {e}"))?;
    let exe_str = exe.to_string_lossy().into_owned();
    let exe_dir = exe
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let icon = format!("{},0", exe_str);

    let script = format!(
        "$new = Join-Path '{dir}' '{name}.lnk'\n\
         $n = (New-Object -ComObject WScript.Shell).CreateShortcut($new)\n\
         $n.TargetPath = '{exe}'\n\
         $n.WorkingDirectory = '{exedir}'\n\
         $n.IconLocation = '{icon}'\n\
         $n.Description = '{desc}'\n\
         $n.Save()\n",
        dir = ps_quote(&dir.to_string_lossy()),
        name = EXPECTED_NAME,
        exe = ps_quote(&exe_str),
        exedir = ps_quote(&exe_dir),
        icon = ps_quote(&icon),
        desc = EXPECTED_DESC,
    );
    ps_capture(&script)?;
    Ok(())
}
