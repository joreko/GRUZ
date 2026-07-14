pub mod events;

use crate::db::Database;
use crate::downloader::pipeline;
use crate::downloader::spec::build_spec;
use crate::error::{AppError, Result};
use crate::queue::{
    task::{DownloadTask, Priority, TaskState},
    Queue,
};
use crate::ytdlp::YtDlp;
use events::{DownloadCompletedPayload, DownloadFailedPayload, OrchestratorEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex, Notify};
use tracing::{error, info, warn};
use url::Url;

/// Тип хендла оркестратора — используется для self-ref в воркерах
pub type OrchestratorHandle = Arc<Mutex<Orchestrator>>;

// ── Защита от дублей мыслей ──────────────────────────────────────────────────
// Оркестратор может выпустить одну и ту же мысль несколько раз (разные пути
// эмита, повторный idle, гонки). Дедуп на фронте маскирует баг, поэтому
// защита — на источнике: одинаковая мысль (kind+text+title+progress) не
// улетает в WebView чаще чем раз в THOUGHT_DEDUP_MS.
struct ThoughtDedup {
    key: String,
    ts: i64,
}
static LAST_THOUGHT: StdMutex<Option<ThoughtDedup>> = StdMutex::new(None);
const THOUGHT_DEDUP_MS: i64 = 1500;

fn should_emit_thought(t: &events::Thought) -> bool {
    let key = format!(
        "{}\u{0}{}\u{0}{:?}\u{0}{:?}",
        t.kind, t.text, t.title, t.progress
    );
    let mut guard = LAST_THOUGHT.lock().unwrap();
    match guard.as_ref() {
        Some(prev) if prev.key == key && t.ts - prev.ts < THOUGHT_DEDUP_MS => false,
        _ => {
            *guard = Some(ThoughtDedup { key, ts: t.ts });
            true
        }
    }
}

// ── «Голос» оркестратора вынесен в orchestrator::events (builders) ───────────

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
                let n = saved.len();
                let mut q = queue.lock().await;
                for saved_task in saved {
                    q.push(DownloadTask::from(saved_task));
                }
                if n > 0 {
                    Self::emit_static(&handle, OrchestratorEvent::Thought(events::recovered(n)));
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

    /// Запустить ожидающие задачи после инициализации (статический вариант)
    pub async fn start(orch: Arc<Mutex<Orchestrator>>) {
        {
            let o = orch.lock().await;
            o.emit(OrchestratorEvent::QueueUpdated);
        }
        Orchestrator::tick(Arc::clone(&orch)).await;
    }

    // ── Публичный API (вызывается из commands/) ──────────────────────────────

    /// Быстрое чтение настроек для fetch_info (без запуска yt-dlp).
    pub fn ytdlp(&self) -> Arc<YtDlp> {
        Arc::clone(&self.ytdlp)
    }

    pub async fn fetch_info_prepare(&self) -> (Option<String>, Arc<YtDlp>, String) {
        Self::emit_static(&self.handle, OrchestratorEvent::Thought(events::fetching()));
        let settings = self.db.get_settings().await.ok();
        let proxy = settings.as_ref().and_then(|s| {
            if s.proxy.is_empty() {
                None
            } else {
                Some(s.proxy.clone())
            }
        });
        let extra_args = settings
            .as_ref()
            .map(|s| s.ytdlp_extra_args.clone())
            .unwrap_or_default();
        (proxy, Arc::clone(&self.ytdlp), extra_args)
    }

    /// Добавить задачу в очередь.
    pub async fn enqueue(
        orch: Arc<Mutex<Orchestrator>>,
        url: String,
        format: String,
        quality: String,
        container: String,
        fps: Option<u32>,
        source_fps: Option<u32>,
        bitrate: Option<u32>,
        title: Option<String>,
        thumbnail: Option<String>,
        channel: Option<String>,
        duration: Option<f64>,
        is_playlist: bool,
        audio_codec: Option<String>,
        video_codec: Option<String>,
    ) -> Result<DownloadTask> {
        if let Some(br) = bitrate {
            let is_audio_fmt = matches!(
                container.as_str(),
                "mp3" | "aac" | "opus" | "flac" | "wav" | "m4a"
            ) || format == "audio";
            let max_bitrate = if is_audio_fmt { 320 } else { 100_000 };
            if br > max_bitrate {
                return Err(AppError::Validation(format!(
                    "Bitrate cannot exceed {max_bitrate} kbps"
                )));
            }
        }
        if url.starts_with('-') || url.is_empty() {
            return Err(AppError::Validation("Некорректный URL".into()));
        }
        if Url::parse(&url).is_err() {
            return Err(AppError::Validation("Некорректный URL".into()));
        }

        let (task, db) = {
            let o = orch.lock().await;
            {
                let q = o.queue.lock().await;
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
            task.source_fps = source_fps;
            task.bitrate = bitrate;
            task.title = title;
            task.thumbnail = thumbnail;
            task.channel = channel;
            task.duration = duration.and_then(|d| d.is_finite().then(|| d.round() as i64));
            task.is_playlist = is_playlist;
            task.audio_codec = audio_codec;
            task.video_codec = video_codec;
            task.state = TaskState::Waiting;
            let db = Arc::clone(&o.db);
            (task, db)
        };

        if let Err(e) = db.save_task(&task).await {
            tracing::error!("не удалось сохранить задачу в БД: {e}");
            return Err(e);
        }

        {
            let mut o = orch.lock().await;
            o.queue.lock().await.push(task.clone());
            o.emit(OrchestratorEvent::QueueUpdated);
            o.emit(OrchestratorEvent::Thought(events::enqueued()));
            o.reset_idle();
        }
        // Запуск задач — через фоновый tick-цикл (без удержания лока оркестратора)
        {
            let o = orch.lock().await;
            o.tick_notify.notify_waiters();
        }
        Ok(task)
    }

    /// Отменить задачу.
    pub async fn cancel(orch: Arc<Mutex<Orchestrator>>, task_id: &str) -> Result<()> {
        let (is_waiting, db) = {
            let mut o = orch.lock().await;
            let is_waiting = o
                .queue
                .lock()
                .await
                .get(task_id)
                .map_or(false, |t| matches!(t.state, TaskState::Waiting));

            if let Some(tx) = o.cancel_senders.remove(task_id) {
                let _ = tx.send(());
            }
            {
                let mut q = o.queue.lock().await;
                q.update_state(task_id, TaskState::Cancelled);
            }
            (is_waiting, Arc::clone(&o.db))
        };

        if is_waiting {
            if let Err(e) = db.delete_task(task_id).await {
                tracing::error!(%e, "db: cancel failed to delete task");
            }
        }
        {
            let mut o = orch.lock().await;
            o.emit(OrchestratorEvent::QueueUpdated);
            o.emit(OrchestratorEvent::Thought(events::cancelled()));
            o.reset_idle();
        }
        {
            let o = orch.lock().await;
            o.tick_notify.notify_waiters();
        }
        Ok(())
    }

    pub async fn get_queue(&self) -> Vec<DownloadTask> {
        let mut tasks = self.queue.lock().await.all().to_vec();
        tasks.sort_by(|a, b| match (a.ordering, b.ordering) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => b
                .priority
                .cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at)),
        });
        tasks
    }

    pub async fn remove_task(orch: Arc<Mutex<Orchestrator>>, task_id: &str) -> Result<()> {
        if let Some(tx) = {
            let mut o = orch.lock().await;
            o.cancel_senders.remove(task_id)
        } {
            let _ = tx.send(());
        }
        {
            let o = orch.lock().await;
            o.queue.lock().await.remove(task_id);
        }
        {
            let o = orch.lock().await;
            if let Err(e) = o.db.delete_task(task_id).await {
                tracing::error!(task_id, %e, "db: delete_task failed");
            }
            o.emit(OrchestratorEvent::QueueUpdated);
            o.tick_notify.notify_waiters();
        }
        Ok(())
    }

    pub async fn clear_queue(&mut self) -> Result<()> {
        let part_templates: Vec<String> = {
            let q = self.queue.lock().await;
            q.all().iter().filter_map(|t| t.file_path.clone()).collect()
        };

        for tx in self.cancel_senders.drain() {
            let _ = tx.1.send(());
        }

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

        for base_path in part_templates {
            cleanup_part_files(&base_path).await;
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
        let to_save = {
            let mut q = self.queue.lock().await;
            if !q.reorder(task_id, new_index) {
                return Err(AppError::Validation("Задача не найдена".into()));
            }
            q.renumber_ordering();
            q.all().iter().map(|t| t.clone()).collect::<Vec<_>>()
        };
        for t in &to_save {
            if let Err(e) = self.db.save_task(t).await {
                tracing::error!(task_id = %t.id, %e, "db: reorder save failed");
            }
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        Ok(())
    }

    /// Повторить упавшую/отменённую задачу: сброс состояния в Waiting и перезапуск.
    pub async fn retry_task(orch: Arc<Mutex<Orchestrator>>, task_id: &str) -> Result<()> {
        let (task, db) = {
            let o = orch.lock().await;
            let mut q = o.queue.lock().await;
            let t = q
                .get_mut(task_id)
                .ok_or_else(|| AppError::Validation("Задача не найдена".into()))?;
            if !matches!(t.state, TaskState::Failed | TaskState::Cancelled) {
                return Err(AppError::Validation(
                    "Повторить можно только Failed/Cancelled задачу".into(),
                ));
            }
            t.state = TaskState::Waiting;
            t.progress = 0.0;
            t.error = None;
            t.file_path = None;
            t.speed = None;
            t.eta = None;
            let db = Arc::clone(&o.db);
            (t.clone(), db)
        };

        db.save_task(&task).await?;

        {
            let mut o = orch.lock().await;
            o.emit(OrchestratorEvent::QueueUpdated);
            o.reset_idle();
        }
        {
            let o = orch.lock().await;
            o.tick_notify.notify_waiters();
        }
        Ok(())
    }

    pub async fn update_max_concurrent(&mut self, n: usize) {
        self.max_concurrent = n;
        self.tick_notify.notify_waiters();
    }

    /// Эмитировать мысль напрямую из команд (тонкий слой)
    pub fn thought_remove(&self) {
        self.emit(OrchestratorEvent::Thought(events::removed()));
    }
    pub fn thought_settings(&self, key: &str, old: Option<&str>, new: &str) {
        self.emit(OrchestratorEvent::Thought(events::settings_changed(
            key, old, new,
        )));
    }
    pub fn thought_history_delete(&self) {
        self.emit(OrchestratorEvent::Thought(events::history_deleted()));
    }
    pub fn thought_history_clear(&self) {
        self.emit(OrchestratorEvent::Thought(events::history_cleared()));
    }
    pub fn thought_open_file(&self) {
        self.emit(OrchestratorEvent::Thought(events::file_opened()));
    }
    pub fn thought_open_folder(&self) {
        self.emit(OrchestratorEvent::Thought(events::folder_opened()));
    }

    pub fn emit_history_updated(&self) {
        self.handle.emit("history:updated", ()).ok();
    }

    // ── Внутренняя логика ────────────────────────────────────────────────────

    /// Проверить, можно ли запустить новые задачи, и запустить их.
    /// Статический: не держит лок оркестратора во время БД-I/O и спавна воркеров.
    pub async fn tick(orch: Arc<Mutex<Orchestrator>>) {
        loop {
            // Короткая блокировка: очистить устаревшие cancel_senders + выбрать следующую.
            let next = {
                let mut o = orch.lock().await;
                let active_ids: Vec<String> = o
                    .queue
                    .lock()
                    .await
                    .all()
                    .iter()
                    .filter(|t| {
                        matches!(
                            t.state,
                            TaskState::Waiting | TaskState::Downloading | TaskState::Converting
                        )
                    })
                    .map(|t| t.id.clone())
                    .collect();
                o.cancel_senders.retain(|id, _| active_ids.contains(id));
                let q = o.queue.lock().await;
                let active = q.active_count();
                if active >= o.max_concurrent {
                    None
                } else {
                    q.next_waiting().map(|t| t.id.clone())
                }
            };
            let Some(task_id) = next else { break };
            Orchestrator::spawn_worker(Arc::clone(&orch), task_id).await;
        }
    }

    async fn spawn_worker(orch: Arc<Mutex<Orchestrator>>, task_id: String) {
        info!(task_id = %task_id, "spawning worker");

        // Снимок задачи + перевод в Downloading под коротким локом.
        let task_snapshot = {
            let o = orch.lock().await;
            let mut q = o.queue.lock().await;
            q.update_state(&task_id, TaskState::Downloading);
            q.get(&task_id).cloned()
        };

        // Клоны Arc, нужные воркеру — чтобы не держать лок оркестратора.
        let (db, ytdlp, handle, tick_notify, queue) = {
            let o = orch.lock().await;
            (
                Arc::clone(&o.db),
                Arc::clone(&o.ytdlp),
                o.handle.clone(),
                Arc::clone(&o.tick_notify),
                Arc::clone(&o.queue),
            )
        };

        // Персистировать состояние Downloading (без удержания лока оркестратора).
        if let Some(ref t) = task_snapshot {
            if let Err(e) = db.save_task(t).await {
                tracing::warn!("не удалось сохранить состояние задачи в БД: {e}");
            }
        }
        Orchestrator::emit_static(&handle, OrchestratorEvent::QueueUpdated);
        Orchestrator::emit_static(&handle, OrchestratorEvent::Thought(events::started()));
        // Честный idle: сбрасываем таймер при старте загрузки, чтобы «скучаю»
        // не вылетало посреди активной работы.
        orch.lock().await.reset_idle();

        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        {
            let mut o = orch.lock().await;
            o.cancel_senders.insert(task_id.clone(), cancel_tx);
        }

        let settings = match db.get_settings().await {
            Ok(s) => s,
            Err(e) => {
                error!("не удалось прочитать настройки: {e}");
                let mut q = queue.lock().await;
                if let Some(t) = q.get_mut(&task_id) {
                    t.state = TaskState::Failed;
                    t.error = Some(format!("Ошибка чтения настроек: {e}"));
                }
                Orchestrator::emit_static(&handle, OrchestratorEvent::QueueUpdated);
                return;
            }
        };

        let Some(task) = task_snapshot else { return };

        let spec = build_spec(&task, &settings);
        let orch_clone = Arc::clone(&orch);

        tokio::spawn(async move {
            let result = pipeline::run_pipeline(
                spec,
                ytdlp,
                queue.clone(),
                handle.clone(),
                cancel_rx,
                task.clone(),
                &settings,
            )
            .await;
            match result {
                Ok(file_path) => {
                    info!(task_id = %task_id, file_path = %file_path, "download completed successfully");
                    let mut q = queue.lock().await;
                    let (task_title, task_file_path) = if let Some(t) = q.get_mut(&task_id) {
                        t.state = TaskState::Completed;
                        t.file_path = Some(file_path.clone());
                        t.progress = 100.0;
                        (t.title.clone(), t.file_path.clone())
                    } else {
                        (None, None)
                    };
                    let task_for_history = q.get(&task_id).cloned();
                    drop(q);

                    if let Some(t) = task_for_history {
                        let file_size = tokio::fs::metadata(&file_path).await.ok().map(|m| {
                            let sz = m.len() as i64;
                            info!(task_id = %task_id, file_size = %sz, "got file metadata");
                            sz
                        });
                        let mut finalized = false;
                        for attempt in 0..3u32 {
                            match db
                                .finalize_download(&task_id, &file_path, file_size.or(t.file_size))
                                .await
                            {
                                Ok(_) => {
                                    finalized = true;
                                    info!(task_id = %task_id, "download finalized");
                                    break;
                                }
                                Err(e) => {
                                    warn!(task_id = %task_id, attempt, %e, "finalize failed, retrying");
                                    if attempt < 2 {
                                        tokio::time::sleep(std::time::Duration::from_millis(200))
                                            .await;
                                    } else {
                                        error!(
                                            "не удалось финализировать загрузку после 3 попыток: {e}"
                                        );
                                    }
                                }
                            }
                        }
                        if finalized {
                            let id = task_id.clone();
                            let db_thumb = Arc::clone(&db);
                            let handle_thumb = handle.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = db_thumb.generate_thumbnail(&id).await {
                                    tracing::warn!(id = %id, %e, "фоновая генерация превью не удалась");
                                } else {
                                    handle_thumb.emit("history:updated", ()).ok();
                                }
                            });
                        } else {
                            warn!(task_id = %task_id, "finalize failed, marking task as Failed");
                            let mut q = queue.lock().await;
                            if let Some(t) = q.get_mut(&task_id) {
                                t.state = TaskState::Failed;
                                t.error = Some("не удалось сохранить в галерею".into());
                            }
                        }
                    } else {
                        warn!(task_id = %task_id, "task already removed from queue, cleaning DB");
                        if let Err(e) = db.delete_task(&task_id).await {
                            error!(task_id = %task_id, %e, "db: delete failed (edge case)");
                        }
                    }

                    let trimmed = queue.lock().await.trim_completed();
                    if !trimmed.is_empty() {
                        info!(task_id = %task_id, trimmed = ?trimmed, "trimmed completed tasks from queue");
                    }

                    handle.emit("queue:updated", ()).ok();
                    Orchestrator::emit_static(
                        &handle,
                        OrchestratorEvent::DownloadCompleted(DownloadCompletedPayload {
                            task_id: task_id.clone(),
                            title: task_title.clone(),
                            file_path: task_file_path,
                        }),
                    );
                    Orchestrator::emit_static(
                        &handle,
                        OrchestratorEvent::Thought(events::completed()),
                    );
                    orch_clone.lock().await.reset_idle();
                    tick_notify.notify_waiters();
                }
                Err(e) => {
                    let cancelled = matches!(e, AppError::Cancelled);
                    let err_msg = e.to_string();
                    if cancelled {
                        info!(task_id = %task_id, "download cancelled by user");
                    } else {
                        error!(task_id = %task_id, error = %err_msg, "download failed");
                    }
                    {
                        let mut q = queue.lock().await;
                        if let Some(t) = q.get_mut(&task_id) {
                            if cancelled {
                                t.state = TaskState::Cancelled;
                                t.error = None;
                            } else {
                                t.state = TaskState::Failed;
                                t.error = Some(err_msg.clone());
                            }
                        }
                    }

                    if let Err(e) = db.delete_task(&task_id).await {
                        error!(task_id = %task_id, %e, "db: delete failed after error/cancel");
                    }

                    queue.lock().await.trim_completed();

                    handle.emit("queue:updated", ()).ok();
                    if !cancelled {
                        Orchestrator::emit_static(
                            &handle,
                            OrchestratorEvent::DownloadFailed {
                                task_id: task_id.clone(),
                                error: err_msg.clone(),
                            },
                        );
                        let reason = events::friendly_error(&err_msg);
                        Orchestrator::emit_static(
                            &handle,
                            OrchestratorEvent::Thought(events::failed(&reason)),
                        );
                    }
                    orch_clone.lock().await.reset_idle();
                    tick_notify.notify_waiters();
                }
            }
        });
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
                if should_emit_thought(&t) {
                    handle.emit("orchestrator:thought", t).ok();
                }
            }
            OrchestratorEvent::DownloadCompleted(payload) => {
                handle.emit("download:completed", &payload).ok();
            }
            OrchestratorEvent::DownloadFailed { task_id, error } => {
                handle
                    .emit("download:failed", &DownloadFailedPayload { task_id, error })
                    .ok();
            }
        }
    }

    /// Сбросить idle-таймер: отменить предыдущий и запустить новый (30с)
    pub fn reset_idle(&mut self) {
        drop(self.idle_cancel.take());

        let (tx, mut rx) = oneshot::channel::<()>();
        self.idle_cancel = Some(tx);
        let handle = self.handle.clone();
        let queue = Arc::clone(&self.queue);

        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(30)) => {
                    // Честный idle: стреляем только если реально ничего не происходит
                    // (нет активных загрузок И нет задач в ожидании).
                    let (active, pending) = {
                        let q = queue.lock().await;
                        (
                            q.active_count(),
                            q.all().iter().any(|t| matches!(t.state, TaskState::Waiting)),
                        )
                    };
                    if active == 0 && !pending {
                        handle
                            .emit("orchestrator:thought", events::idle())
                            .ok();
                    }
                }
                _ = &mut rx => {}
            }
        });
    }

    /// Graceful shutdown: отправить cancel всем активным воркерам.
    pub fn shutdown(&mut self) {
        for (_id, tx) in self.cancel_senders.drain() {
            let _ = tx.send(());
        }
        drop(self.idle_cancel.take());
    }
}

/// Удалить .part файлы оставленные yt-dlp после отмены загрузки.
/// yt-dlp создаёт: `{path}.part` и `{path}.fXXX.part` (для DASH-сегментов).
/// Ошибки игнорируются — это best-effort cleanup.
async fn cleanup_part_files(base_path: &str) {
    use std::path::Path;

    let path = Path::new(base_path);
    let parent = match path.parent() {
        Some(p) if p != Path::new("") => p,
        _ => return,
    };
    let stem = match path.file_name().and_then(|n| n.to_str()) {
        Some(s) => s.to_string(),
        None => return,
    };

    let Ok(mut dir) = tokio::fs::read_dir(parent).await else {
        return;
    };

    while let Ok(Some(entry)) = dir.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&stem) && name_str.ends_with(".part") {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}
