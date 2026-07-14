use serde::Serialize;

/// События которые оркестратор шлёт в WebView
#[derive(Debug)]
pub enum OrchestratorEvent {
    QueueUpdated,
    Thought(Thought),
    DownloadCompleted(DownloadCompletedPayload),
    DownloadFailed { task_id: String, error: String },
}

/// Мысль оркестратора — отображается в шапке (typewriter) и на странице оркестратора
/// (лента). Несёт контекст задачи, а не рандомное слово.
#[derive(Debug, Serialize, Clone)]
pub struct Thought {
    /// Тип события: started | progress | completed | error | recovered | idle | info | chatter
    pub kind: String,
    /// Уже собранная фраза (бэкенд подставляет название/прогресс)
    pub text: String,
    /// Важность: "info" | "success" | "warn" | "error" | "muted"
    pub severity: String,
    /// Название задачи (если есть) — для контекста
    pub title: Option<String>,
    /// Прогресс 0..100 (для kind=progress)
    pub progress: Option<f32>,
    /// Человекочитаемое пояснение: что именно произошло и почему
    /// (показывается на странице оркестратора под текстом мысли)
    pub description: Option<String>,
    /// Unix-ms — для ленты на странице оркестратора
    pub ts: i64,
}

/// Полезная нагрузка события `DownloadFailed` (task_id + текст ошибки)
#[derive(Debug, Serialize, Clone)]
pub struct DownloadFailedPayload {
    pub task_id: String,
    pub error: String,
}

/// Полезная нагрузка события `DownloadCompleted` (task_id + опциональные метаданные)
#[derive(Debug, Serialize, Clone)]
pub struct DownloadCompletedPayload {
    pub task_id: String,
    pub title: Option<String>,
    pub file_path: Option<String>,
}

// ── «Голос» оркестратора ───────────────────────────────────────────────────
// Фиксированная, грамотная фраза на каждое событие.
// Названия видео в мыслях НЕ используются — только обобщённый статус задачи.
// Без рандома: стабильный, читаемый тон вместо прыгающих синонимов.

/// Собрать мысль с контекстом (прогресс, пояснение, таймстемп).
/// Название задачи намеренно не передаётся — в мыслях не должно быть
/// названий видео.
pub fn build(
    kind: &str,
    text: String,
    severity: &str,
    title: Option<String>,
    progress: Option<f32>,
    description: Option<String>,
) -> Thought {
    Thought {
        kind: kind.to_string(),
        text,
        severity: severity.to_string(),
        title,
        progress,
        description,
        ts: now_ms(),
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Человекочитаемая причина ошибки yt-dlp (без сырого stderr).
pub fn friendly_error(msg: &str) -> String {
    let m = msg.to_lowercase();
    let r = if m.contains("private") {
        "приватное видео"
    } else if m.contains("copyright") {
        "заблокировано по копирайту"
    } else if m.contains("not available") || m.contains("unavailable") {
        "недоступно в вашем регионе"
    } else if m.contains("deleted") || m.contains("removed") {
        "видео удалено"
    } else if m.contains("sign in") || m.contains("age") {
        "нужна авторизация (возраст)"
    } else if m.contains("quota") || m.contains("too many") {
        "YouTube ограничил запросы"
    } else if m.contains("proxy") {
        "не удалось через прокси"
    } else {
        msg
    };
    r.chars().take(120).collect()
}

// ── Конструкторы мыслей (единственный источник «голоса») ───────────────────

pub fn started() -> Thought {
    build(
        "started",
        "Начинаю загрузку".into(),
        "info",
        None,
        None,
        Some("Загрузка началась — прогресс в активных потоках.".into()),
    )
}

pub fn enqueued() -> Thought {
    build(
        "chatter",
        "Добавил в очередь".into(),
        "info",
        None,
        None,
        Some("Ждёт свободного слота в очереди.".into()),
    )
}

pub fn completed() -> Thought {
    build(
        "completed",
        "Готово".into(),
        "success",
        None,
        Some(100.0),
        Some("Файл добавлен в галерею.".into()),
    )
}

pub fn failed(reason: &str) -> Thought {
    build(
        "error",
        "Не вышло".into(),
        "error",
        None,
        None,
        Some(reason.to_string()),
    )
}

pub fn cancelled() -> Thought {
    build(
        "warning",
        "Отменено".into(),
        "warn",
        None,
        None,
        Some("Остановлено по запросу пользователя.".into()),
    )
}

pub fn fetching() -> Thought {
    build(
        "info",
        "Смотрю ссылку…".into(),
        "info",
        None,
        None,
        Some("Запрашиваю метаданные перед добавлением в очередь.".into()),
    )
}

pub fn idle() -> Thought {
    build(
        "idle",
        "Жду ссылку".into(),
        "muted",
        None,
        None,
        Some("Нечего делать — жду новых задач.".into()),
    )
}

pub fn recovered(n: usize) -> Thought {
    build(
        "recovered",
        format!("Восстановил {n} задач с прошлого раза"),
        "info",
        None,
        None,
        Some("Вернул незавершённые задачи из базы после перезапуска.".into()),
    )
}

/// Мысль на этапе прогресса (25/50/75%) — чтобы оркестратор «жил» во время загрузки.
pub fn milestone(pct: u8) -> Thought {
    build(
        "progress",
        format!("{pct}%"),
        "info",
        None,
        Some(pct as f32),
        Some("Этап загрузки.".into()),
    )
}

/// Короткое подтверждение действия пользователя (удаление, настройки, открытие).
pub fn chatter(msg: &str, severity: &str) -> Thought {
    build("chatter", msg.to_string(), severity, None, None, None)
}

/// То же, но с пояснением (описанием) под текстом.
pub fn chatter_with_desc(msg: &str, severity: &str, description: Option<String>) -> Thought {
    build(
        "chatter",
        msg.to_string(),
        severity,
        None,
        None,
        description,
    )
}

pub fn removed() -> Thought {
    chatter("Убрал", "muted")
}
pub fn history_deleted() -> Thought {
    chatter("Удалил из истории", "muted")
}
pub fn history_cleared() -> Thought {
    chatter("История очищена", "warn")
}
pub fn settings_changed(key: &str, old: Option<&str>, new: &str) -> Thought {
    let label = settings_label(key);
    let desc = match old {
        Some(o) if o != new => format!("{label}: {o} → {new}."),
        _ => format!("{label}: {new}."),
    };
    chatter_with_desc("Обновил настройки", "muted", Some(desc))
}

/// Человекочитаемая подпись настройки по ключу (для описания мысли).
fn settings_label(key: &str) -> &str {
    match key {
        "download_dir" => "папка загрузок",
        "save_dir_video" => "папка видео",
        "save_dir_audio" => "папка аудио",
        "save_dir_playlist" => "папка плейлистов",
        "save_dir_shorts" => "папка шортсов",
        "save_dir_trimmed" => "папка нарезок",
        "default_format" => "формат по умолчанию",
        "default_quality" => "качество по умолчанию",
        "default_container" => "контейнер по умолчанию",
        "max_concurrent" => "параллельные загрузки",
        "proxy" => "прокси",
        "default_fps" => "FPS",
        "default_bitrate" => "битрейт",
        "auto_merge" => "слияние потоков",
        "embed_subtitles" => "вшивание субтитров",
        "minimize_to_tray" => "сворачивание в трей",
        "ytdlp_extra_args" => "доп. аргументы yt-dlp",
        _ => key,
    }
}
pub fn file_opened() -> Thought {
    chatter("Открываю файл", "muted")
}
pub fn folder_opened() -> Thought {
    chatter("Открываю папку", "muted")
}
