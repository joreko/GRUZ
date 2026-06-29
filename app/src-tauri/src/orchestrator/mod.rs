pub mod events;

use crate::db::{history::NewHistoryItem, settings::Settings, Database};
use crate::downloader::process;
use crate::error::{AppError, Result};
use crate::queue::{
    task::{DownloadTask, Priority, TaskState},
    Queue,
};
use crate::ytdlp::YtDlp;
use events::{OrchestratorEvent, Thought};
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex, Notify};
use tracing::{error, info};
use url::Url;

/// Тип хендла оркестратора — используется для self-ref в воркерах
pub type OrchestratorHandle = Arc<Mutex<Orchestrator>>;

// ── Пулы фраз ────────────────────────────────────────────────────────────────

const PHRASES_FETCH: &[(&str, &str)] = &[("смотрю...", "info"), ("изучаю.", "info")];
const PHRASES_ENQUEUE: &[(&str, &str)] = &[("добавил.", "info"), ("в очереди.", "info")];
const PHRASES_START: &[(&str, &str)] = &[("поехали.", "info"), ("забираю.", "info")];
const PHRASES_DONE: &[(&str, &str)] = &[
    ("готово.", "success"),
    ("есть.", "success"),
    ("сделано.", "success"),
];
const PHRASES_ERROR: &[(&str, &str)] = &[
    ("не вышло.", "error"),
    ("ошибка.", "error"),
    ("сломалось.", "error"),
];
const PHRASES_CANCEL: &[(&str, &str)] = &[("отменил.", "warning"), ("как скажешь.", "warning")];
const PHRASES_IDLE: &[(&str, &str)] = &[("жду ссылку...", "info"), ("скучаю.", "muted")];
const PHRASES_REMOVE: &[(&str, &str)] = &[("убрал.", "muted"), ("как хочешь.", "muted")];
const PHRASES_OPEN_FILE: &[(&str, &str)] = &[("открываю.", "info")];
const PHRASES_OPEN_FOLDER: &[(&str, &str)] = &[("смотрим.", "info")];
const PHRASES_SETTINGS: &[(&str, &str)] = &[("меняем.", "muted"), ("настраиваем.", "muted")];
const PHRASES_HISTORY_DELETE: &[(&str, &str)] = &[("удалил.", "muted")];
const PHRASES_HISTORY_CLEAR: &[(&str, &str)] = &[("всё стёр.", "warning"), ("чисто.", "warning")];

fn pick(pool: &[(&str, &str)]) -> Thought {
    let idx = rand::thread_rng().gen_range(0..pool.len());
    let text = pool[idx].0.to_string();
    Thought {
        text,
        color: pool[idx].1.into(),
        priority: 1,
    }
}

fn pick_chatter(pool: &[(&str, &str)]) -> Thought {
    let mut t = pick(pool);
    t.priority = 0;
    t
}

/// Центр всей системы. Владеет очередью, воркерами и БД.
pub struct Orchestrator {
    queue: Arc<Mutex<Queue>>,
    pub(crate) db: Arc<Database>,
    ytdlp: Arc<YtDlp>,
    handle: AppHandle,
    cancel_senders: HashMap<String, oneshot::Sender<()>>,
    max_concurrent: usize,
    idle_cancel: Option<oneshot::Sender<()>>,
    /// Уведомление воркеров о завершении — для запуска tick() извне
    pub tick_notify: Arc<Notify>,
}

impl Orchestrator {
    pub async fn new(db: Database, handle: AppHandle) -> Result<Self> {
        let settings = db.get_settings().await?;
        let ytdlp = YtDlp::new()?;
        let db = Arc::new(db);

        // Восстановить задачи из БД в очередь (waiting/paused)
        let queue = Arc::new(Mutex::new(Queue::new()));
        match db.load_pending_tasks().await {
            Ok(saved) => {
                let mut q = queue.lock().await;
                for saved_task in saved {
                    q.push(DownloadTask::from(saved_task));
                }
            }
            Err(e) => {
                tracing::error!("не удалось восстановить задачи из БД: {e}");
                // Отправляем событие, чтобы UI показал пустую очередь
                Self::emit_static(&handle, OrchestratorEvent::QueueUpdated);
            }
        }

        Ok(Self {
            queue,
            db,
            ytdlp: Arc::new(ytdlp),
            handle,
            cancel_senders: HashMap::new(),
            max_concurrent: settings.max_concurrent as usize,
            idle_cancel: None,
            tick_notify: Arc::new(Notify::new()),
        })
    }

    /// Запустить ожидающие задачи после инициализации
    pub async fn start(&mut self) {
        self.emit(OrchestratorEvent::QueueUpdated);
        self.tick().await;
    }

    // ── Публичный API (вызывается из commands/) ──────────────────────────────

    /// Быстрое чтение настроек для fetch_info (без запуска yt-dlp).
    /// Возвращает proxy и Arc<YtDlp>; собственно fetch_info_exec вызывается вне блокировки.
    pub async fn fetch_info_prepare(&self) -> (Option<String>, Arc<YtDlp>) {
        Self::emit_static(
            &self.handle,
            OrchestratorEvent::Thought(pick(PHRASES_FETCH)),
        );
        let proxy = self.db.get_settings().await.ok().and_then(|s| {
            if s.proxy.is_empty() {
                None
            } else {
                Some(s.proxy.clone())
            }
        });
        (proxy, Arc::clone(&self.ytdlp))
    }

    pub async fn enqueue(
        &mut self,
        url: String,
        format: String,
        quality: String,
        container: String,
        fps: Option<u32>,
        bitrate: Option<u32>,
        title: Option<String>,
        thumbnail: Option<String>,
        channel: Option<String>,
        duration: Option<f64>,
        is_playlist: bool,
        audio_codec: Option<String>,
        video_codec: Option<String>,
    ) -> Result<DownloadTask> {
        // Bitrate domain validation
        if let Some(br) = bitrate {
            if br > 0 && br > 320000 {
                return Err(AppError::Validation(format!(
                    "Bitrate cannot exceed {} kbps",
                    320000
                )));
            }
        }
        // Валидация URL
        if url.starts_with('-') || url.is_empty() {
            return Err(AppError::Validation("Некорректный URL".into()));
        }
        if Url::parse(&url).is_err() {
            return Err(AppError::Validation("Некорректный URL".into()));
        }

        // Проверка дубликата: та же ссылка + те же параметры уже в активной очереди
        {
            let q = self.queue.lock().await;
            let duplicate = q.all().iter().any(|t| {
                t.url == url
                    && t.format == format
                    && t.quality == quality
                    && t.container == container
                    && t.fps == fps
                    && t.audio_codec == audio_codec
                    && t.video_codec == video_codec
                    && matches!(
                        t.state,
                        TaskState::Waiting | TaskState::Downloading | TaskState::Converting
                    )
            });
            if duplicate {
                return Err(crate::error::AppError::Validation(
                    "Такая задача уже есть в очереди".into(),
                ));
            }
        }
        let mut task = DownloadTask::new(url, format, quality, container);
        task.fps = fps;
        task.bitrate = bitrate;
        task.title = title;
        task.thumbnail = thumbnail;
        task.channel = channel;
        task.duration = duration.and_then(|d| d.is_finite().then(|| d.round() as i64));
        task.is_playlist = is_playlist;
        task.audio_codec = audio_codec;
        task.video_codec = video_codec;
        task.state = TaskState::Waiting;
        // Сначала сохраняем в БД (persist), потом добавляем в очередь
        // Порядок важен: при краше между save и push задача восстановится при старте
        if let Err(e) = self.db.save_task(&task).await {
            tracing::error!("не удалось сохранить задачу в БД: {e}");
            return Err(e);
        }
        {
            let mut q = self.queue.lock().await;
            q.push(task.clone());
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_ENQUEUE)));
        self.reset_idle();
        self.tick().await;
        Ok(task)
    }

    pub async fn cancel(&mut self, task_id: &str) -> Result<()> {
        let is_waiting = self
            .queue
            .lock()
            .await
            .get(task_id)
            .map_or(false, |t| matches!(t.state, TaskState::Waiting));

        if let Some(tx) = self.cancel_senders.remove(task_id) {
            let _ = tx.send(());
        }
        {
            let mut q = self.queue.lock().await;
            q.update_state(task_id, TaskState::Cancelled);
        }
        // Waiting-задача не имеет воркера — удаляем из БД здесь.
        // Для Downloading воркер удалит сам по завершении.
        if is_waiting {
            if let Err(e) = self.db.delete_task(task_id).await {
                tracing::error!(%e, "db: cancel failed to delete task");
            }
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_CANCEL)));
        self.reset_idle();
        self.tick().await;
        Ok(())
    }

    pub async fn get_queue(&self) -> Vec<DownloadTask> {
        let mut tasks = self.queue.lock().await.all().to_vec();
        tasks.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    pub async fn remove_task(&mut self, task_id: &str) -> Result<()> {
        if let Some(tx) = self.cancel_senders.remove(task_id) {
            let _ = tx.send(());
        }
        self.queue.lock().await.remove(task_id);
        if let Err(e) = self.db.delete_task(task_id).await {
            tracing::error!(task_id, %e, "db: delete_task failed");
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.tick().await;
        Ok(())
    }

    pub async fn clear_queue(&mut self) -> Result<()> {
        // Отменить все активные загрузки
        for tx in self.cancel_senders.drain() {
            let _ = tx.1.send(());
        }
        // Удалить все задачи из памяти и БД одним lock-ом
        let ids: Vec<String> = {
            let mut q = self.queue.lock().await;
            let ids = q.all().iter().map(|t| t.id.clone()).collect();
            q.clear();
            ids
        };
        for id in ids {
            if let Err(e) = self.db.delete_task(&id).await {
                tracing::error!(task_id = %id, %e, "db: clear_queue delete_task failed");
            }
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        Ok(())
    }

    pub async fn set_priority(&mut self, task_id: &str, priority: Priority) -> Result<()> {
        if !self.queue.lock().await.set_priority(task_id, priority) {
            return Err(AppError::Validation("Задача не найдена".into()));
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        Ok(())
    }

    pub async fn reorder_task(&mut self, task_id: &str, new_index: usize) -> Result<()> {
        if !self.queue.lock().await.reorder(task_id, new_index) {
            return Err(AppError::Validation("Задача не найдена".into()));
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        Ok(())
    }

    pub async fn update_max_concurrent(&mut self, n: usize) {
        self.max_concurrent = n;
        self.tick().await;
    }

    /// Эмитировать мысль напрямую из команд (тонкий слой)
    pub fn thought_remove(&self) {
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_REMOVE)));
    }
    pub fn thought_settings(&self) {
        self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_SETTINGS)));
    }
    pub fn thought_history_delete(&self) {
        self.emit(OrchestratorEvent::Thought(pick_chatter(
            PHRASES_HISTORY_DELETE,
        )));
    }
    pub fn thought_history_clear(&self) {
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_HISTORY_CLEAR)));
    }
    pub fn thought_open_file(&self) {
        self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_OPEN_FILE)));
    }
    pub fn thought_open_folder(&self) {
        self.emit(OrchestratorEvent::Thought(pick_chatter(
            PHRASES_OPEN_FOLDER,
        )));
    }

    // ── Внутренняя логика ────────────────────────────────────────────────────

    /// Проверить, можно ли запустить новые задачи, и запустить их
    pub async fn tick(&mut self) {
        // Очистить устаревшие cancel_senders (завершённые/отменённые задачи)
        {
            let q = self.queue.lock().await;
            self.cancel_senders.retain(|id, _| {
                q.get(id).map_or(false, |t| {
                    matches!(
                        t.state,
                        TaskState::Waiting | TaskState::Downloading | TaskState::Converting
                    )
                })
            });
        }

        loop {
            let (active, next_id) = {
                let q = self.queue.lock().await;
                let active = q.active_count();
                let next = q.next_waiting().map(|t| t.id.clone());
                (active, next)
            };

            if active >= self.max_concurrent {
                break;
            }
            let Some(task_id) = next_id else { break };
            self.spawn_worker(task_id).await;
        }
    }

    async fn spawn_worker(&mut self, task_id: String) {
        // Обновляем состояние и сразу клонируем задачу одним lock-ом
        let task_snapshot = {
            let mut q = self.queue.lock().await;
            q.update_state(&task_id, TaskState::Downloading);
            q.get(&task_id).cloned()
        };
        // Персистировать состояние Downloading — при краше задача будет помечена Failed
        if let Some(ref t) = task_snapshot {
            let db = Arc::clone(&self.db);
            let t = t.clone();
            tokio::spawn(async move {
                if let Err(e) = db.save_task(&t).await {
                    tracing::warn!("не удалось сохранить состояние задачи в БД: {e}");
                }
            });
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_START)));

        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        self.cancel_senders.insert(task_id.clone(), cancel_tx);

        // Читаем настройки до spawn — не тащим db внутрь async move без нужды
        let settings = match self.db.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                error!("не удалось прочитать настройки: {e}");
                {
                    let mut q = self.queue.lock().await;
                    if let Some(t) = q.get_mut(&task_id) {
                        t.state = TaskState::Failed;
                        t.error = Some(format!("Ошибка чтения настроек: {e}"));
                    }
                }
                self.emit(OrchestratorEvent::QueueUpdated);
                return;
            }
        };

        let queue = Arc::clone(&self.queue);
        let db = Arc::clone(&self.db);
        let ytdlp = Arc::clone(&self.ytdlp);
        let handle = self.handle.clone();
        let tick_notify = Arc::clone(&self.tick_notify);

        let Some(task) = task_snapshot else { return };

        tokio::spawn(async move {
            let result = run_download(
                task.clone(),
                ytdlp,
                queue.clone(),
                handle.clone(),
                cancel_rx,
                settings,
            )
            .await;
            match result {
                Ok(file_path) => {
                    info!("download completed for {task_id}, file: {file_path}");
                    let mut q = queue.lock().await;
                    if let Some(t) = q.get_mut(&task_id) {
                        t.state = TaskState::Completed;
                        t.file_path = Some(file_path.clone());
                        t.progress = 100.0;
                    }
                    // Перенести в историю + удалить из персистентной очереди
                    if let Some(t) = q.get(&task_id).cloned() {
                        drop(q);
                        let db2 = Arc::clone(&db);
                        let tid = task_id.clone();
                        let fp = file_path.clone();
                        // Получаем реальный размер файла
                        let file_size = tokio::fs::metadata(&fp).await.ok().map(|m| m.len() as i64);
                        let history_item = NewHistoryItem {
                            url: t.url,
                            video_id: t.video_id,
                            platform: t.platform,
                            title: t.title.unwrap_or_else(|| "Unknown".into()),
                            channel: t.channel,
                            channel_id: t.channel_id,
                            thumbnail: t.thumbnail,
                            duration: t.duration,
                            file_path: fp,
                            file_size: file_size.or(t.file_size),
                            format: t.format,
                            quality: t.quality,
                            container: t.container,
                            fps: t.fps.map(|v| v as i64),
                            bitrate: t.bitrate.map(|v| v as i64),
                            audio_codec: t.audio_codec,
                            video_codec: t.video_codec,
                            trim_start: t.trim_start,
                            trim_end: t.trim_end,
                            playlist_id: None,
                            playlist_index: None,
                        };
                        tokio::spawn(async move {
                            // Сохраняем в историю с retry (3 попытки, 200ms между)
                            for attempt in 0..3 {
                                match db2.add_history(history_item.clone()).await {
                                    Ok(_) => break,
                                    Err(e) => {
                                        if attempt < 2 {
                                            tokio::time::sleep(std::time::Duration::from_millis(
                                                200,
                                            ))
                                            .await;
                                        } else {
                                            error!("не удалось сохранить в историю после 3 попыток: {e}");
                                        }
                                    }
                                }
                            }
                            if let Err(e) = db2.delete_task(&tid).await {
                                error!("не удалось удалить задачу из БД: {e}");
                            }
                        });
                    } else {
                        // Если задача не найдена (редкий случай), освобождаем лок
                        drop(q);
                    }
                    // Удаляем завершённые задачи из in-memory очереди
                    let trimmed = queue.lock().await.trim_completed();
                    for tid in trimmed {
                        // Completed уже удалена воркером выше; Failed/Cancelled — удаляем здесь
                        if tid != task_id {
                            if let Err(e) = db.delete_task(&tid).await {
                                error!(task_id = %tid, %e, "db: trim_completed delete failed");
                            }
                        }
                    }
                    handle.emit("queue:updated", ()).ok();
                    handle.emit("orchestrator:thought", pick(PHRASES_DONE)).ok();
                    tick_notify.notify_waiters();
                }
                Err(e) => {
                    let cancelled = matches!(e, AppError::Cancelled);
                    if !cancelled {
                        error!("download failed for {task_id}: {e}");
                    }
                    let mut q = queue.lock().await;
                    if let Some(t) = q.get_mut(&task_id) {
                        if cancelled {
                            t.state = TaskState::Cancelled;
                            t.error = None;
                        } else {
                            t.state = TaskState::Failed;
                            t.error = Some(e.to_string());
                        }
                    }
                    drop(q); // Освобождаем лок перед trim_completed
                             // Удаляем завершённые/отменённые задачи из памяти и БД
                    let trimmed = queue.lock().await.trim_completed();
                    for tid in trimmed {
                        if let Err(e) = db.delete_task(&tid).await {
                            error!(task_id = %tid, %e, "db: trim_completed delete failed");
                        }
                    }
                    handle.emit("queue:updated", ()).ok();
                    if !cancelled {
                        handle
                            .emit("orchestrator:thought", pick(PHRASES_ERROR))
                            .ok();
                    }
                    tick_notify.notify_waiters();
                }
            }
        });

        // Запускаем idle-таймер только один раз в enqueue/cancel, не здесь
    }

    fn emit(&self, event: OrchestratorEvent) {
        Self::emit_static(&self.handle, event);
    }

    fn emit_static(handle: &AppHandle, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::QueueUpdated => {
                handle.emit("queue:updated", ()).ok();
            }
            OrchestratorEvent::Thought(t) => {
                handle.emit("orchestrator:thought", t).ok();
            }
        }
    }

    /// Сбросить idle-таймер: отменить предыдущий и запустить новый (30с)
    pub fn reset_idle(&mut self) {
        // Отменить предыдущий
        drop(self.idle_cancel.take());

        let (tx, mut rx) = oneshot::channel::<()>();
        self.idle_cancel = Some(tx);
        let handle = self.handle.clone();

        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    handle.emit("orchestrator:thought", pick_chatter(PHRASES_IDLE)).ok();
                }
                _ = &mut rx => {}
            }
        });
    }
}

/// Добавить прокси и ytdlp_extra_args (без embed-subs) — для transcode-ветки
fn add_base_args(mut args: Vec<String>, settings: &Settings) -> Vec<String> {
    if !settings.proxy.is_empty() {
        args.push("--proxy".into());
        args.push(settings.proxy.clone());
    }
    for arg in settings.ytdlp_extra_args.split_whitespace() {
        if !arg.is_empty() {
            args.push(arg.to_string());
        }
    }
    args
}

fn add_settings_args(args: Vec<String>, settings: &Settings) -> Vec<String> {
    let mut args = add_base_args(args, settings);
    if settings.embed_subtitles {
        args.push("--embed-subs".into());
        args.push("--sub-langs".into());
        args.push("all,-live_chat".into());
    }
    args
}

/// Определяет тип контента по задаче и возвращает полный output template для yt-dlp.
/// Если save_dir_X пуст — используется базовый download_dir.
fn resolve_output_template(settings: &Settings, task: &DownloadTask) -> String {
    let (dir, tpl) = if task.format == "audio" {
        (&settings.save_dir_audio, &settings.save_tpl_audio)
    } else if task.trim_start.is_some() || task.trim_end.is_some() {
        (&settings.save_dir_trimmed, &settings.save_tpl_trimmed)
    } else if task.url.contains("/shorts/") {
        (&settings.save_dir_shorts, &settings.save_tpl_shorts)
    } else if task.is_playlist {
        (&settings.save_dir_playlist, &settings.save_tpl_playlist)
    } else {
        (&settings.save_dir_video, &settings.save_tpl_video)
    };

    let base = if dir.is_empty() {
        &settings.download_dir
    } else {
        dir
    };
    if base.is_empty() {
        let fallback = dirs_next::home_dir()
            .map(|p| p.join("Downloads"))
            .unwrap_or_else(|| {
                tracing::warn!("home_dir недоступен, использую текущую директорию");
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
        return format!("{}/{}", fallback.to_string_lossy(), tpl);
    }
    format!("{}/{}", base, tpl)
}

async fn run_download(
    task: DownloadTask,
    ytdlp: Arc<YtDlp>,
    queue: Arc<Mutex<Queue>>,
    handle: AppHandle,
    cancel_rx: oneshot::Receiver<()>,
    settings: Settings,
) -> Result<String> {
    let output_template = resolve_output_template(&settings, &task);
    let handle_clone = handle.clone();
    let task_id = task.id.clone();

    // Выбираем format_id и постпроцессинг
    let fps_filter = task
        .fps
        .map(|f| format!("[fps<={}]", f))
        .unwrap_or_default();

    let (format_arg, post_args): (String, Vec<String>) = match task.format.as_str() {
        "audio" => {
            let ext = match task.container.as_str() {
                "mp3" => "mp3",
                "m4a" => "m4a",
                "opus" => "opus",
                "flac" => "flac",
                _ => "mp3",
            };
            // quality для аудио — битрейт-фильтр вида "[abr<=320]" или пусто
            let audio_fmt = if task.quality.starts_with('[') {
                format!("bestaudio{}/bestaudio/best", task.quality)
            } else {
                "bestaudio/best".to_string()
            };
            let mut post = vec![
                "--extract-audio".to_string(),
                "--audio-format".to_string(),
                ext.to_string(),
            ];
            if let Some(br) = task.bitrate.filter(|b| *b > 0) {
                post.extend_from_slice(&[
                    "--postprocessor-args".to_string(),
                    format!("ffmpeg:-b:a {}k", br),
                ]);
            } else {
                post.extend_from_slice(&["--audio-quality".to_string(), "0".to_string()]);
            }
            (audio_fmt, post)
        }
        "video_only" => {
            let sel = format!("bestvideo{}{}/bestvideo", task.quality, fps_filter);
            let merge_fmt = match task.container.as_str() {
                "mkv" => "mkv",
                "webm" => "webm",
                "mov" => "mov",
                _ => "mp4",
            };
            let mut post = if settings.auto_merge {
                vec!["--merge-output-format".to_string(), merge_fmt.to_string()]
            } else {
                Vec::new()
            };
            // video_only тоже поддерживает перекодирование
            if let Some(ref vc) = task.video_codec {
                let vlib = match vc.as_str() {
                    "h264" => "libx264",
                    "h265" => "libx265",
                    "vp9" => "libvpx-vp9",
                    "av1" => "libaom-av1",
                    "prores" => "prores_ks",
                    _ => "copy",
                };
                post.extend_from_slice(&[
                    "--postprocessor-args".to_string(),
                    format!("ffmpeg:-c:v {}", vlib),
                ]);
            }
            (sel, post)
        }
        _ => {
            // Аудио-поток: fallback если конкретный ext недоступен
            let audio_sel = if let Some(ref codec) = task.audio_codec {
                let ext = match codec.as_str() {
                    "aac" => "m4a",
                    "opus" => "webm",
                    _ => "",
                };
                if ext.is_empty() {
                    "bestaudio/best".to_string()
                } else {
                    format!("bestaudio[ext={}]/bestaudio/best", ext)
                }
            } else {
                "bestaudio/best".to_string()
            };
            let sel = format!("{}{}+{}", task.quality, fps_filter, audio_sel);
            let merge_fmt = match task.container.as_str() {
                "mkv" => "mkv",
                "webm" => "webm",
                "mov" => "mov",
                _ => "mp4",
            };

            // Собираем ffmpeg аргументы в одну строку чтобы не конфликтовали
            let vlib = task.video_codec.as_deref().map(|c| match c {
                "h264" => "libx264",
                "h265" => "libx265",
                "vp9" => "libvpx-vp9",
                "av1" => "libaom-av1",
                // ProRes работает только в MOV — принудительно переключаем контейнер
                "prores" => "prores_ks",
                _ => "copy",
            });
            let alib = task.audio_codec.as_deref().map(|c| match c {
                "aac" => "aac",
                "opus" => "libopus",
                "mp3" => "libmp3lame",
                "flac" => "flac",
                "ac3" => "ac3",
                _ => "copy",
            });

            // ProRes только в MOV
            let effective_merge_fmt = if task.video_codec.as_deref() == Some("prores") {
                "mov"
            } else {
                merge_fmt
            };

            let mut post = if settings.auto_merge {
                vec![
                    "--merge-output-format".to_string(),
                    effective_merge_fmt.to_string(),
                ]
            } else {
                Vec::new()
            };

            // Строим единую строку ffmpeg аргументов
            let need_transcode = vlib.is_some() || alib.is_some() || effective_merge_fmt == "mov";
            if need_transcode {
                let vc = vlib.unwrap_or(if effective_merge_fmt == "mov" {
                    "libx264"
                } else {
                    "copy"
                });
                let ac = alib.unwrap_or(if effective_merge_fmt == "mov" {
                    "aac"
                } else {
                    "copy"
                });
                // Битрейт встраиваем сюда же чтобы не дублировать --postprocessor-args
                let br_part = task
                    .bitrate
                    .filter(|b| *b > 0)
                    .map(|b| format!(" -b:v {}k", b))
                    .unwrap_or_default();
                let mut ffargs = format!("-c:v {}{} -c:a {}", vc, br_part, ac);
                if effective_merge_fmt == "mov" {
                    ffargs.push_str(" -movflags +faststart");
                }
                post.extend_from_slice(&[
                    "--postprocessor-args".to_string(),
                    format!("ffmpeg:{}", ffargs),
                ]);
                let base_args = add_base_args(post, &settings);
                return Ok(process::download_video(
                    &ytdlp,
                    task.id.clone(),
                    &task.url,
                    &sel,
                    &output_template,
                    &base_args,
                    move |progress| {
                        let q = queue.clone();
                        let h = handle_clone.clone();
                        let tid = task_id.clone();
                        tokio::spawn(async move {
                            let mut q = q.lock().await;
                            if let Some(t) = q.get_mut(&tid) {
                                t.progress = progress.progress;
                                t.speed = progress.speed.clone();
                                t.eta = progress.eta.clone();
                                if progress.state == "finished" || progress.state == "converting" {
                                    t.state = TaskState::Converting;
                                }
                            }
                            h.emit("download:progress", progress).ok();
                        });
                    },
                    cancel_rx,
                    task.duration,
                )
                .await?);
            }

            // Без явного перекодирования: форсируем -c copy чтобы ffmpeg не пытался
            // конвертировать несовместимые потоки (VP9 в mp4 без -c copy → Conversion failed!)
            // Если задан битрейт — нужно перекодировать, иначе -b:v игнорируется при copy
            let copy_args = if let Some(br) = task.bitrate.filter(|b| *b > 0) {
                format!("ffmpeg:-c:v libx264 -b:v {}k -c:a copy", br)
            } else {
                "ffmpeg:-c copy".to_string()
            };
            post.extend_from_slice(&["--postprocessor-args".to_string(), copy_args]);
            (sel, post)
        }
    };

    let post = add_settings_args(post_args, &settings);
    process::download_video(
        &ytdlp,
        task.id.clone(),
        &task.url,
        &format_arg,
        &output_template,
        &post,
        move |progress| {
            let q = queue.clone();
            let h = handle_clone.clone();
            let tid = task_id.clone();
            tokio::spawn(async move {
                let mut q = q.lock().await;
                if let Some(t) = q.get_mut(&tid) {
                    t.progress = progress.progress;
                    t.speed = progress.speed.clone();
                    t.eta = progress.eta.clone();
                    if progress.state == "finished" || progress.state == "converting" {
                        t.state = TaskState::Converting;
                    }
                }
                h.emit("download:progress", progress).ok();
            });
        },
        cancel_rx,
        task.duration,
    )
    .await
}
