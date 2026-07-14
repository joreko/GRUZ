use crate::downloader::process;
use crate::downloader::spec::{add_base_args, add_settings_args, DownloadSpec, PlanMode};
use crate::downloader::DownloadProgress;
use crate::error::{AppError, Result};
use crate::orchestrator::events;
use crate::queue::task::{DownloadTask, TaskState};
use crate::queue::Queue;
use crate::ytdlp::YtDlp;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{info, instrument};

/// Запустить загрузку по готовой спецификации.
///
/// Единый агрегатор прогресса (один на всю загрузку) слушает mpsc-канал и
/// обновляет очередь + шлёт `download:progress`. Это устраняет дублирование
/// прогресс-задач, которое было в двух ветках старого `run_download`.
#[instrument(skip(ytdlp, queue, handle, cancel_rx, settings, spec, task), fields(url = %task.url))]
pub async fn run_pipeline(
    spec: DownloadSpec,
    ytdlp: Arc<YtDlp>,
    queue: Arc<Mutex<Queue>>,
    handle: AppHandle,
    cancel_rx: oneshot::Receiver<()>,
    task: DownloadTask,
    settings: &crate::db::settings::Settings,
) -> Result<String> {
    let (progress_tx, agg_handle) = spawn_progress_aggregator(queue.clone(), handle.clone());

    let result = match spec.mode {
        PlanMode::Single => {
            let post = add_settings_args(spec.post_args, settings);
            let tx = progress_tx.clone();
            process::download_video(
                &ytdlp,
                task.id.clone(),
                &task.url,
                &spec.video_selector,
                &spec.output_template,
                &post,
                None,
                move |progress| {
                    let _ = tx.send(progress);
                },
                cancel_rx,
                task.duration,
            )
            .await
        }
        PlanMode::SeparateStreams => {
            run_separate_streams(
                spec,
                ytdlp,
                queue,
                handle,
                cancel_rx,
                &task,
                settings,
                progress_tx.clone(),
            )
            .await
        }
    };

    // Все клоны sender'а (внутри download_video / run_ffmpeg_merge) дропнулись
    // по завершении — дропаем последний и ждём, пока агрегатор сбросит
    // финальные события прогресса в очередь/фронт.
    drop(progress_tx);
    let _ = agg_handle.await;

    result
}

/// Видео и аудио качаются отдельно, затем сливаются ffmpeg вручную.
/// Отмена форвардится во все три фазы (видео/аудио/мердж).
async fn run_separate_streams(
    spec: DownloadSpec,
    ytdlp: Arc<YtDlp>,
    queue: Arc<Mutex<Queue>>,
    handle: AppHandle,
    cancel_rx: oneshot::Receiver<()>,
    task: &DownloadTask,
    settings: &crate::db::settings::Settings,
    progress_tx: mpsc::UnboundedSender<DownloadProgress>,
) -> Result<String> {
    let base_args = add_base_args(Vec::new(), settings);

    let mut cleanup_guard = CleanupGuard(Vec::new());

    let (v_tx, v_rx) = oneshot::channel::<()>();
    let (a_tx, a_rx) = oneshot::channel::<()>();
    let (f_tx, f_rx) = oneshot::channel::<()>();
    let cancel_forward = tokio::spawn(async move {
        if cancel_rx.await.is_ok() {
            let _ = v_tx.send(());
            let _ = a_tx.send(());
            let _ = f_tx.send(());
        }
    });

    let v_tx2 = progress_tx.clone();
    let video_path = process::download_video(
        &ytdlp,
        task.id.clone(),
        &task.url,
        &spec.video_selector,
        &spec.output_template,
        &base_args,
        Some("video"),
        move |progress| {
            let _ = v_tx2.send(progress);
        },
        v_rx,
        task.duration,
    )
    .await?;
    cleanup_guard.push(video_path.clone());

    let a_tx2 = progress_tx.clone();
    let audio_path = process::download_video(
        &ytdlp,
        task.id.clone(),
        &task.url,
        spec.audio_selector.as_deref().unwrap_or("bestaudio/best"),
        spec.audio_template
            .as_deref()
            .unwrap_or(&spec.output_template),
        &base_args,
        Some("audio"),
        move |progress| {
            let _ = a_tx2.send(progress);
        },
        a_rx,
        task.duration,
    )
    .await?;
    cleanup_guard.push(audio_path.clone());

    // Временный выход ffmpeg (уникальное имя), затем переименовываем в итоговый путь.
    let final_path = process::derive_output_path(&video_path, spec.merge_fmt);
    let video_p = Path::new(&video_path);
    let parent = video_p.parent().unwrap_or(Path::new(""));
    let stem = video_p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let tmp_out = parent
        .join(format!("{}.merge.{}", stem, spec.merge_fmt))
        .to_string_lossy()
        .into_owned();

    let f_tx2 = progress_tx.clone();
    let merged = process::run_ffmpeg_merge(
        &ytdlp,
        &task.id,
        &video_path,
        &audio_path,
        &tmp_out,
        spec.ffmpeg_merge_args.as_deref().unwrap_or(""),
        task.duration,
        move |progress| {
            let _ = f_tx2.send(progress);
        },
        f_rx,
    )
    .await?;

    // Мердж завершён — сбрасываем форвардер отмены (иначе f_tx дропался бы
    // раньше и run_ffmpeg_merge висел на 0% конвертации).
    cancel_forward.abort();

    tokio::fs::rename(&merged, &final_path).await.map_err(|e| {
        AppError::DownloadFailed(format!("не удалось переименовать результат: {e}"))
    })?;

    // Успех: промежуточные потоки уже удалены внутри run_ffmpeg_merge —
    // разоружаем гвард, чтобы не удалять итоговый файл.
    cleanup_guard.disarm();
    info!(task_id = %task.id, file_path = %final_path, "transcode result");
    let _ = (&queue, &handle);
    Ok(final_path)
}

/// Единый агрегатор прогресса: один mpsc-канал → обновление очереди + фронт.
fn spawn_progress_aggregator(
    queue: Arc<Mutex<Queue>>,
    handle: AppHandle,
) -> (
    mpsc::UnboundedSender<DownloadProgress>,
    tokio::task::JoinHandle<()>,
) {
    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<DownloadProgress>();
    let handle_task = tokio::spawn(async move {
        // Milestone-мысли (25/50/75%) — чтобы оркестратор «жил» во время загрузки.
        // Одна серия на задачу: после видео-фазы карта уже на шаге 3, повторные
        // фазы (аудио/мердж) не дублируют мысли.
        let mut milestones: HashMap<String, u8> = HashMap::new();
        while let Some(progress) = progress_rx.recv().await {
            let mut q = queue.lock().await;
            if let Some(t) = q.get_mut(&progress.task_id) {
                if progress.state == "converting" {
                    if t.state != TaskState::Converting {
                        t.state = TaskState::Converting;
                        t.progress = 0.0;
                    } else {
                        t.progress = progress.progress;
                    }
                } else {
                    t.progress = progress.progress;
                }
                t.speed = progress.speed.clone();
                t.eta = progress.eta.clone();
            }
            drop(q);
            handle.emit("download:progress", &progress).ok();

            if progress.state == "downloading"
                && progress.progress > 0.0
                && progress.progress < 100.0
            {
                let step = (progress.progress / 25.0).floor() as u8; // 1,2,3
                if step > milestones.get(&progress.task_id).copied().unwrap_or(0) {
                    milestones.insert(progress.task_id.clone(), step);
                    let _ = handle.emit("orchestrator:thought", events::milestone(step * 25));
                }
            }
        }
    });
    (progress_tx, handle_task)
}

/// RAII-гвард: удаляет перечисленные файлы (и их `.part`-сиблингов) при drop,
/// если не разоружён через `disarm()`. Используется в транскод-ветке, чтобы не
/// оставлять промежуточные потоки (видео/аудио), если загрузка или мердж упали.
struct CleanupGuard(Vec<String>);
impl CleanupGuard {
    fn push(&mut self, path: String) {
        self.0.push(path);
    }
    fn disarm(&mut self) {
        self.0.clear();
    }
}
impl Drop for CleanupGuard {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
            let _ = std::fs::remove_file(format!("{}.part", p));
        }
    }
}
