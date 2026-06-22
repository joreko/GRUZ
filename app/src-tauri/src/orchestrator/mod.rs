pub mod events;

use crate::db::{history::NewHistoryItem, settings::Settings, Database};
use crate::downloader::{process, VideoInfo};
use crate::error::Result;
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

/// Тип хендла оркестратора — используется для self-ref в воркерах
pub type OrchestratorHandle = Arc<Mutex<Orchestrator>>;

// ── Личность ──────────────────────────────────────────────────────────────────
const NAME: &str = "Груз";

// ── Пулы фраз ────────────────────────────────────────────────────────────────

const PHRASES_FETCH:          &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_ENQUEUE:        &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_START:          &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_DONE:           &[(&str, &str)] = &[("привет, я {name}.", "success")];
const PHRASES_ERROR:          &[(&str, &str)] = &[("привет, я {name}.", "error")];
const PHRASES_CANCEL:         &[(&str, &str)] = &[("привет, я {name}.", "warning")];
const PHRASES_IDLE:           &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_REMOVE:         &[(&str, &str)] = &[("привет, я {name}.", "muted")];
const PHRASES_OPEN_FILE:      &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_OPEN_FOLDER:    &[(&str, &str)] = &[("привет, я {name}.", "info")];
const PHRASES_SETTINGS:       &[(&str, &str)] = &[("привет, я {name}.", "muted")];
const PHRASES_HISTORY_DELETE: &[(&str, &str)] = &[("привет, я {name}.", "muted")];
const PHRASES_HISTORY_CLEAR:  &[(&str, &str)] = &[("привет, я {name}.", "warning")];

fn pick(pool: &[(&str, &str)]) -> Thought {
    let idx = rand::thread_rng().gen_range(0..pool.len());
    let text = pool[idx].0.replace("{name}", NAME);
    Thought { text, color: pool[idx].1.into(), priority: 1 }
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
            Err(e) => tracing::warn!("не удалось восстановить задачи из БД: {e}"),
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

    // ── Публичный API (вызывается из commands/) ──────────────────────────────

    pub async fn fetch_info(&mut self, url: String) -> Result<VideoInfo> {
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_FETCH)));
        let result = process::fetch_info(&self.ytdlp, &url).await;
        self.reset_idle();
        result
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
        let mut task = DownloadTask::new(url, format, quality, container);
        task.fps         = fps;
        task.bitrate     = bitrate;
        task.title       = title;
        task.thumbnail   = thumbnail;
        task.channel     = channel;
        task.duration    = duration.map(|d| d as i64);
        task.is_playlist = is_playlist;
        task.audio_codec = audio_codec;
        task.video_codec = video_codec;
        {
            let mut q = self.queue.lock().await;
            let mut t = task.clone();
            t.state = TaskState::Waiting;
            q.push(t);
            task.state = TaskState::Waiting;
        }
        // Персистировать задачу в БД (fire-and-forget, не блокируем очередь)
        {
            let db = Arc::clone(&self.db);
            let t = task.clone();
            tokio::spawn(async move {
                if let Err(e) = db.save_task(&t).await {
                    tracing::warn!("не удалось сохранить задачу в БД: {e}");
                }
            });
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_ENQUEUE)));
        self.reset_idle();
        self.tick().await;
        Ok(task)
    }

    pub async fn cancel(&mut self, task_id: &str) -> Result<()> {
        if let Some(tx) = self.cancel_senders.remove(task_id) {
            let _ = tx.send(());
        }
        {
            let mut q = self.queue.lock().await;
            q.update_state(task_id, TaskState::Cancelled);
        }
        self.emit(OrchestratorEvent::QueueUpdated);
        self.emit(OrchestratorEvent::Thought(pick(PHRASES_CANCEL)));
        self.reset_idle();
        self.tick().await;
        Ok(())
    }

    pub async fn get_queue(&self) -> Vec<DownloadTask> {
        self.queue.lock().await.all().to_vec()
    }

    pub async fn remove_task(&mut self, task_id: &str) -> Result<()> {
        if let Some(tx) = self.cancel_senders.remove(task_id) {
            let _ = tx.send(());
        }
        self.queue.lock().await.remove(task_id);
        self.emit(OrchestratorEvent::QueueUpdated);
        self.tick().await;
        Ok(())
    }

    pub async fn set_priority(&mut self, task_id: &str, priority: Priority) -> Result<()> {
        self.queue.lock().await.set_priority(task_id, priority);
        self.emit(OrchestratorEvent::QueueUpdated);
        Ok(())
    }

    pub async fn update_max_concurrent(&mut self, n: usize) {
        self.max_concurrent = n;
        self.tick().await;
    }

    /// Эмитировать мысль напрямую из команд (тонкий слой)
    pub fn thought_remove(&self)         { self.emit(OrchestratorEvent::Thought(pick(PHRASES_REMOVE))); }
    pub fn thought_settings(&self)       { self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_SETTINGS))); }
    pub fn thought_history_delete(&self) { self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_HISTORY_DELETE))); }
    pub fn thought_history_clear(&self)  { self.emit(OrchestratorEvent::Thought(pick(PHRASES_HISTORY_CLEAR))); }
    pub fn thought_open_file(&self)      { self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_OPEN_FILE))); }
    pub fn thought_open_folder(&self)    { self.emit(OrchestratorEvent::Thought(pick_chatter(PHRASES_OPEN_FOLDER))); }

    // ── Внутренняя логика ────────────────────────────────────────────────────

    /// Проверить, можно ли запустить новые задачи, и запустить их
    pub async fn tick(&mut self) {
        loop {
            let (active, next_id) = {
                let q = self.queue.lock().await;
                let active = q.active_count();
                let next = q.next_waiting().map(|t| t.id.clone());
                (active, next)
            };

            if active >= self.max_concurrent || next_id.is_none() {
                break;
            }

            let task_id = next_id.unwrap();
            self.spawn_worker(task_id).await;
        }
    }

    async fn spawn_worker(&mut self, task_id: String) {
        {
            let mut q = self.queue.lock().await;
            q.update_state(&task_id, TaskState::Downloading);
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
                return;
            }
        };

        let queue = Arc::clone(&self.queue);
        let db = Arc::clone(&self.db);
        let ytdlp = Arc::clone(&self.ytdlp);
        let handle = self.handle.clone();
        let tick_notify = Arc::clone(&self.tick_notify);

        let task = {
            let q = self.queue.lock().await;
            q.get(&task_id).cloned()
        };

        let Some(task) = task else { return };

        tokio::spawn(async move {
            let result = run_download(task.clone(), ytdlp, queue.clone(), handle.clone(), cancel_rx, settings).await;
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
                        tokio::spawn(async move {
                            let _ = db2.add_history(NewHistoryItem {
                                url: t.url,
                                video_id: t.video_id,
                                platform: t.platform,
                                title: t.title.unwrap_or_else(|| "Unknown".into()),
                                channel: t.channel,
                                channel_id: t.channel_id,
                                thumbnail: t.thumbnail,
                                duration: t.duration,
                                file_path: fp,
                                file_size: t.file_size,
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
                            }).await;
                            let _ = db2.delete_task(&tid).await;
                        });
                    }
                    handle.emit("queue:updated", ()).ok();
                    handle.emit("orchestrator:thought", pick(PHRASES_DONE)).ok();
                    tick_notify.notify_one();
                }
                Err(e) => {
                    error!("download failed for {task_id}: {e:#}");
                    let mut q = queue.lock().await;
                    if let Some(t) = q.get_mut(&task_id) {
                        t.state = TaskState::Failed;
                        t.error = Some(e.to_string());
                    }
                    handle.emit("queue:updated", ()).ok();
                    handle.emit("orchestrator:thought", pick(PHRASES_ERROR)).ok();
                    tick_notify.notify_one();
                }
            }
        });

        // Запускаем idle-таймер: 30с тишины после старта воркера.
        // Если придёт новая задача — reset_idle() отменит его.
        self.reset_idle();
    }

    fn emit(&self, event: OrchestratorEvent) {
        match event {
            OrchestratorEvent::QueueUpdated => {
                self.handle.emit("queue:updated", ()).ok();
            }
            OrchestratorEvent::Thought(t) => {
                self.handle.emit("orchestrator:thought", t).ok();
            }
        }
    }

    /// Сбросить idle-таймер: отменить предыдущий и запустить новый (30с)
    fn reset_idle(&mut self) {
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

/// Определяет тип контента по задаче и возвращает полный output template для yt-dlp.
/// Если save_dir_X пуст — используется базовый download_dir.
fn resolve_output_template(settings: &Settings, task: &DownloadTask) -> String {
    let (dir, tpl) = if task.format == "audio" {
        (&settings.save_dir_audio, &settings.save_tpl_audio)
    } else if task.url.contains("/shorts/") {
        (&settings.save_dir_shorts, &settings.save_tpl_shorts)
    } else if task.is_playlist {
        (&settings.save_dir_playlist, &settings.save_tpl_playlist)
    } else {
        (&settings.save_dir_video, &settings.save_tpl_video)
    };

    let base = if dir.is_empty() { &settings.download_dir } else { dir };
    let base = if base.is_empty() { "." } else { base.as_str() };
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
    let fps_filter = task.fps.map(|f| format!("[fps<={}]", f)).unwrap_or_default();

    let (format_arg, post_args): (String, Vec<String>) = match task.format.as_str() {
        "audio" => {
            let ext = match task.container.as_str() {
                "mp3" => "mp3", "m4a" => "m4a", "opus" => "opus", "flac" => "flac", _ => "mp3",
            };
            (
                "bestaudio/best".to_string(),
                vec![
                    "--extract-audio".to_string(),
                    "--audio-format".to_string(), ext.to_string(),
                    "--audio-quality".to_string(), "0".to_string(),
                ],
            )
        }
        "video_only" => {
            let sel = format!("bestvideo{}{}/bestvideo", task.quality, fps_filter);
            let merge_fmt = match task.container.as_str() {
                "mkv" => "mkv", "webm" => "webm", "mov" => "mov", _ => "mp4",
            };
            let mut post = vec!["--merge-output-format".to_string(), merge_fmt.to_string()];
            // video_only тоже поддерживает перекодирование
            if let Some(ref vc) = task.video_codec {
                let vlib = match vc.as_str() {
                    "h264" => "libx264", "h265" => "libx265", "vp9" => "libvpx-vp9",
                    "av1" => "libaom-av1", "prores" => "prores_ks", _ => "copy",
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
                    "aac"  => "m4a",
                    "opus" => "webm",
                    _      => "",
                };
                if ext.is_empty() { "bestaudio/best".to_string() }
                else { format!("bestaudio[ext={}]/bestaudio/best", ext) }
            } else {
                "bestaudio/best".to_string()
            };
            let sel = format!("{}{}+{}", task.quality, fps_filter, audio_sel);
            let merge_fmt = match task.container.as_str() {
                "mkv" => "mkv", "webm" => "webm", "mov" => "mov", _ => "mp4",
            };

            // Собираем ffmpeg аргументы в одну строку чтобы не конфликтовали
            let vlib = task.video_codec.as_deref().map(|c| match c {
                "h264"   => "libx264",
                "h265"   => "libx265",
                "vp9"    => "libvpx-vp9",
                "av1"    => "libaom-av1",
                // ProRes работает только в MOV — принудительно переключаем контейнер
                "prores" => "prores_ks",
                _        => "copy",
            });
            let alib = task.audio_codec.as_deref().map(|c| match c {
                "aac"  => "aac",
                "opus" => "libopus",
                "mp3"  => "libmp3lame",
                "flac" => "flac",
                "ac3"  => "ac3",
                _      => "copy",
            });

            // ProRes только в MOV
            let effective_merge_fmt = if task.video_codec.as_deref() == Some("prores") { "mov" } else { merge_fmt };

            let mut post = vec!["--merge-output-format".to_string(), effective_merge_fmt.to_string()];

            // Строим единую строку ffmpeg аргументов
            let need_transcode = vlib.is_some() || alib.is_some() || effective_merge_fmt == "mov";
            if need_transcode {
                let vc = vlib.unwrap_or(if effective_merge_fmt == "mov" { "libx264" } else { "copy" });
                let ac = alib.unwrap_or(if effective_merge_fmt == "mov" { "aac" } else { "copy" });
                // Битрейт встраиваем сюда же чтобы не дублировать --postprocessor-args
                let br_part = task.bitrate.map(|b| format!(" -b:v {}k", b)).unwrap_or_default();
                let mut ffargs = format!("-c:v {}{} -c:a {}", vc, br_part, ac);
                if effective_merge_fmt == "mov" { ffargs.push_str(" -movflags +faststart"); }
                post.extend_from_slice(&[
                    "--postprocessor-args".to_string(),
                    format!("ffmpeg:{}", ffargs),
                ]);
                return Ok(process::download_video(
                    &ytdlp, task.id.clone(), &task.url, &sel,
                    &output_template, &post,
                    move |progress| {
                        let q = queue.clone(); let h = handle_clone.clone(); let tid = task_id.clone();
                        tokio::spawn(async move {
                            let mut q = q.lock().await;
                            if let Some(t) = q.get_mut(&tid) {
                                t.progress = progress.progress; t.speed = progress.speed.clone(); t.eta = progress.eta.clone();
                            }
                            h.emit("download:progress", progress).ok();
                        });
                    },
                    cancel_rx,
                ).await?);
            }

            // Без перекодирования — добавляем битрейт если нужен
            if let Some(br) = task.bitrate {
                post.extend_from_slice(&[
                    "--postprocessor-args".to_string(),
                    format!("ffmpeg:-b:v {}k", br),
                ]);
            }
            (sel, post)
        }
    };

    process::download_video(
        &ytdlp,
        task.id.clone(),
        &task.url,
        &format_arg,
        &output_template,
        &post_args,
        move |progress| {
            // Обновить состояние в очереди и эмитнуть событие
            let q = queue.clone();
            let h = handle_clone.clone();
            let tid = task_id.clone();
            tokio::spawn(async move {
                let mut q = q.lock().await;
                if let Some(t) = q.get_mut(&tid) {
                    t.progress = progress.progress;
                    t.speed = progress.speed.clone();
                    t.eta = progress.eta.clone();
                    // finished = загрузка завершена, ffmpeg начинает обработку
                    if progress.state == "finished" {
                        t.state = TaskState::Converting;
                    }
                }
                h.emit("download:progress", progress).ok();
            });
        },
        cancel_rx,
    ).await
}
