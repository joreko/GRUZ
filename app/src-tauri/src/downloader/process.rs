use crate::downloader::{DownloadProgress, VideoInfo};
use crate::error::{AppError, Result};
use crate::ytdlp::YtDlp;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use tracing::{debug, error, info, instrument};
use url::Url;

/// User-Agent для yt-dlp — YouTube чаще блокирует дефолтный UA yt-dlp, поэтому
/// используем браузерный. Применяется и к анализу ссылки, и к скачиванию (через этот модуль).
pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Singleton HTTP-клиент — инициализируется один раз, переиспользуется во всех запросах.
/// reqwest::Client внутри держит connection pool, создавать его на каждый запрос накладно.
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("reqwest::Client::builder() с валидными параметрами не может упасть")
    })
}

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Разбор строки доп. аргументов с учётом кавычек:
/// `--cookies "C:\My Videos\c.txt"` → ["--cookies", "C:\My Videos\c.txt"].
/// Обычный split_whitespace ломает пути с пробелами.
fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in s.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

async fn fetch_channel_avatar(channel_url: &str) -> Option<String> {
    let client = http_client();
    for _ in 0..2 {
        if let Some(avatar) = fetch_avatar_once(client, channel_url).await {
            return Some(avatar);
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    None
}

async fn fetch_avatar_once(client: &reqwest::Client, channel_url: &str) -> Option<String> {
    let html = client
        .get(channel_url)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    let marker = "\"avatar\":{\"thumbnails\":[{\"url\":\"";
    let pos = html.find(marker)?;
    let rest = &html[pos + marker.len()..];
    let rest = rest.trim_start_matches('\\').trim_start_matches('"');
    let end = rest.find('"')?;
    let img_url = rest[..end].replace("\\u0026", "&");
    if !img_url.starts_with("http") {
        return None;
    }

    let resp = client.get(&img_url).send().await.ok()?;
    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = resp.bytes().await.ok()?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{};base64,{}", mime, b64))
}

/// Получить метаданные видео (без загрузки)
///
/// `extra_args` — доп. аргументы yt-dlp из настроек (например `--cookies-from-browser`,
/// `--user-agent`), как и при скачивании. Раньше анализ их игнорировал — отсюда
/// ложные «заблокировано» для ссылок, которые скачиваются с cookies.
pub async fn fetch_info(
    ytdlp: &YtDlp,
    url: &str,
    proxy: Option<&str>,
    extra_args: Option<&str>,
) -> Result<VideoInfo> {
    // Валидация: URL не должен начинаться с '-' (защита от инъекции аргументов yt-dlp)
    if url.starts_with('-') || url.is_empty() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }
    if Url::parse(url).is_err() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }
    let mut cmd = Command::new(&ytdlp.path);
    no_window(&mut cmd);
    cmd.args([
        "--dump-json",
        "--no-playlist",
        "--no-warnings",
        "--user-agent",
        USER_AGENT,
    ]);

    if let Some(p) = proxy.filter(|p| !p.is_empty()) {
        cmd.args(["--proxy", p]);
    }
    if let Some(extra) = extra_args {
        for arg in split_args(extra) {
            cmd.arg(arg);
        }
    }

    cmd.arg(url);
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), cmd.output())
        .await
        .map_err(|_| AppError::YtDlp("yt-dlp не ответил за 30 секунд".into()))??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::YtDlp(parse_ytdlp_error(&stderr)));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::YtDlp(format!("failed to parse yt-dlp output: {e}")))?;

    let mut info = parse_video_info(json, url);

    // Получаем аватарку канала параллельно если есть uploader_url
    if let Some(ref channel_url) = info.uploader_url {
        info.channel_avatar = fetch_channel_avatar(channel_url).await;
        debug!("channel_avatar получен");
    }

    Ok(info)
}

/// Запустить загрузку, шлёт прогресс через callback.
///
/// ВАЖНО: yt-dlp пишет строки `[download] ...` и `[download] Destination:` в STDERR
/// (stdout зарезервирован под `--print` JSON, если бы он использовался). Поэтому ОБА
/// потока (stdout и stderr) пропускаются через один и тот же парсер — см. `handle_yt_dlp_line`.
#[instrument(skip(ytdlp, on_progress, cancel_rx), fields(role = role.unwrap_or("single")))]
pub async fn download_video<F>(
    ytdlp: &YtDlp,
    task_id: String,
    url: &str,
    format_id: &str,
    output_template: &str,
    extra_args: &[String],
    role: Option<&'static str>,
    on_progress: F,
    mut cancel_rx: oneshot::Receiver<()>,
    duration_secs: Option<i64>,
) -> Result<String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    // Валидация URL
    if url.starts_with('-') || url.is_empty() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }
    if Url::parse(url).is_err() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }

    // Числовой progress-template БЕЗ ANSI — процент/скорость/eta как числа,
    // чтобы парсер был устойчив к локализации и цветам. `--no-color` — страховка.
    let mut args = vec![
        "--format".to_string(),
        format_id.to_string(),
        "--output".to_string(),
        output_template.to_string(),
        "--newline".to_string(),
        "--progress-template".to_string(),
        "%(progress.status)s %(progress.percent).1f %(progress.speed).1f %(progress.eta).0f %(progress.downloaded_bytes)d %(progress.total_bytes)d"
            .to_string(),
        "--no-color".to_string(),
        "--no-warnings".to_string(),
        "--encoding".to_string(),
        "utf-8".to_string(),
    ];

    // Передаём путь к ffmpeg если он есть — нужен для слияния видео+аудио
    if let Some(ffmpeg) = &ytdlp.ffmpeg_path {
        args.push("--ffmpeg-location".to_string());
        args.push(ffmpeg.to_string_lossy().to_string());
    }

    args.extend_from_slice(extra_args);
    args.push(url.to_string());

    info!(
        task_id = %task_id,
        stream = ?role,
        format = %format_id,
        "starting yt-dlp process"
    );

    let mut child = no_window(
        Command::new(&ytdlp.path)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped()),
    )
    .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::DownloadFailed("не удалось захватить stdout yt-dlp".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::DownloadFailed("не удалось захватить stderr yt-dlp".into()))?;
    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut stderr_lines = BufReader::new(stderr).lines();

    // KillOnDrop — гарантирует kill при premature exit (error/cancel)
    let mut kill_guard = KillOnDrop(Some(child));

    let tid = task_id.clone();
    let mut file_path: Option<String> = None;
    let mut destination_paths: Vec<String> = Vec::new();
    let mut stderr_buf = String::new();
    let mut stderr_closed = false;
    // Флаг явной отмены: true только если cancel_rx получил реальный сигнал (.send(()))
    let mut explicitly_cancelled = false;
    // Флаг: уже отправили первый converting event — чтобы не дублировать
    let mut sent_converting_signal = false;
    // Счётчик [download] Destination: — 1=видео, 2=аудио и т.д.
    let mut stream_count: u32 = 0;
    // Определённый по расширению последнего destination: "video" / "audio"
    // Для SeparateStreams роль потока известна заранее (role) — тогда
    // current_stream_type фиксируется ею и не зависит от расширения файла
    // (у YouTube аудиопоток часто .webm, его нельзя отличить по расширению
    // от видеопотока vp9). Для Single роль None → классификация по порядку
    // destination-ов: 1=видео, 2=аудио.
    let mut current_stream_type: Option<&'static str> = role;

    // Основной цикл: ОБА потока парсятся через один и тот же парсер.
    loop {
        tokio::select! {
            line = stdout_lines.next_line() => {
                match line {
                    Ok(Some(s)) => handle_yt_dlp_line(
                        &tid, &s,
                        &mut file_path, &mut destination_paths,
                        &mut current_stream_type, &mut stream_count,
                        &mut sent_converting_signal, duration_secs,
                        &on_progress, false, &mut stderr_buf, role,
                    ),
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
            line = stderr_lines.next_line(), if !stderr_closed => {
                match line {
                    Ok(Some(s)) => handle_yt_dlp_line(
                        &tid, &s,
                        &mut file_path, &mut destination_paths,
                        &mut current_stream_type, &mut stream_count,
                        &mut sent_converting_signal, duration_secs,
                        &on_progress, true, &mut stderr_buf, role,
                    ),
                    _ => stderr_closed = true,
                }
            }
            result = &mut cancel_rx => {
                // result = Ok(()) означает реальный .send(()) от оркестратора
                // result = Err(_) означает дроп sender'а без сигнала (нормальное завершение)
                if result.is_ok() {
                    explicitly_cancelled = true;
                }
                break;
            }
        }
    }

    // Дочитываем оставшийся stderr — тоже с поддержкой отмены
    loop {
        tokio::select! {
            line = stderr_lines.next_line() => {
                match line {
                    Ok(Some(l)) => handle_yt_dlp_line(
                        &tid, &l,
                        &mut file_path, &mut destination_paths,
                        &mut current_stream_type, &mut stream_count,
                        &mut sent_converting_signal, duration_secs,
                        &on_progress, true, &mut stderr_buf, role,
                    ),
                    _ => break,
                }
            }
            result = &mut cancel_rx => {
                if result.is_ok() {
                    explicitly_cancelled = true;
                }
                break;
            }
        }
    }

    // Явная отмена: KillOnDrop убьёт yt-dlp при drop kill_guard, чистим частичные файлы.
    if explicitly_cancelled {
        cleanup_partial_files(&destination_paths, &file_path).await;
        return Err(AppError::Cancelled);
    }

    // Ждём завершения процесса — тоже с поддержкой отмены
    // cancel_rx зарезолвлен, поэтому используем отдельный таймаут вместо него
    let mut child = kill_guard.disarm().unwrap();
    let status = tokio::time::timeout(std::time::Duration::from_secs(30), child.wait())
        .await
        .map_err(|_| AppError::DownloadFailed("yt-dlp не завершился за 30 секунд".into()))??;

    info!(task_id = %tid, exit_code = ?status.code(), "yt-dlp process exited");

    if !status.success() {
        let err_msg = parse_ytdlp_error(&stderr_buf);
        error!(task_id = %tid, error = %err_msg, "yt-dlp exited with error");
        // Чистим осиротевшие частичные файлы (безопасно: удаляет только .part
        // и сами файлы, если они частичные — завершённый файл не трогается).
        cleanup_partial_files(&destination_paths, &file_path).await;
        return Err(AppError::DownloadFailed(err_msg));
    }

    info!(task_id = %tid, file_path = ?file_path, "returning download result");
    file_path.ok_or_else(|| {
        error!(task_id = %tid, "yt-dlp did not output destination file path");
        AppError::DownloadFailed("yt-dlp не указал путь к сохранённому файлу".into())
    })
}

/// Общий обработчик строки вывода yt-dlp — используется для ОБОИХ потоков (stdout+stderr).
/// Парсит Destination, прогресс и признаки ffmpeg, эмитит события прогресса.
#[allow(clippy::too_many_arguments)]
fn handle_yt_dlp_line<F: Fn(DownloadProgress) + Send + 'static>(
    tid: &str,
    raw_line: &str,
    file_path: &mut Option<String>,
    destination_paths: &mut Vec<String>,
    current_stream_type: &mut Option<&'static str>,
    stream_count: &mut u32,
    sent_converting_signal: &mut bool,
    duration_secs: Option<i64>,
    on_progress: &F,
    is_stderr: bool,
    stderr_buf: &mut String,
    role: Option<&'static str>,
) {
    // Защитное удаление ANSI escape-последовательностей (на случай если yt-dlp добавит цвет)
    let line = strip_ansi(raw_line);

    // [download] Destination: / [ExtractAudio] Destination: / [ffmpeg] Destination: / Merging
    if let Some(path) = parse_destination_line(&line) {
        info!(task_id = %tid, path = %path, "yt-dlp: destination file");
        destination_paths.push(path.clone());
        *file_path = Some(path.clone());
        if line.starts_with("[ExtractAudio] Destination:") {
            // Аудио-режим (--extract-audio): один файл — это аудиопоток.
            *current_stream_type = Some("audio");
        } else if line.starts_with("[download] Destination:") {
            *stream_count += 1;
            // Роль задана явно (SeparateStreams) — не переопределяем по
            // расширению (у YouTube аудио часто .webm, неотличимо от видео).
            if role.is_none() {
                // Single-режим: видеопоток идёт первым, аудиопоток — вторым.
                *current_stream_type = Some(match *stream_count {
                    1 => "video",
                    2 => "audio",
                    _ => current_stream_type.unwrap_or("video"),
                });
            }
            info!(
                task_id = %tid,
                stream_count = *stream_count,
                stream_type = ?*current_stream_type,
                "stream detected"
            );
        }
    }

    // Прогресс загрузки (числовые поля из --progress-template)
    if let Some(mut progress) = parse_progress_line(tid, &line) {
        progress.stream_type = current_stream_type.map(|s| s.to_string());
        if progress.state == "finished" {
            info!(
                task_id = %tid,
                progress = %progress.progress,
                speed = ?progress.speed,
                eta = ?progress.eta,
                downloaded_bytes = ?progress.downloaded_bytes,
                total_bytes = ?progress.total_bytes,
                stream_type = ?progress.stream_type,
                "yt-dlp: stream finished"
            );
        }
        // Построчный debug-спам прогресса убран: каждый тик (≈50мс) душил лог,
        // а живой прогресс и так уходит на фронт через on_progress().
        on_progress(progress);
    }

    // ffmpeg (слияние/перекодирование) — детект из ОБОИХ потоков
    if is_ffmpeg_line(&line) && !*sent_converting_signal {
        *sent_converting_signal = true;
        info!(
            task_id = %tid,
            line = %line,
            "ffmpeg start detected (converting)"
        );
        on_progress(DownloadProgress {
            task_id: tid.to_string(),
            state: "converting".to_string(),
            progress: 0.0,
            speed: None,
            eta: None,
            downloaded_bytes: None,
            total_bytes: None,
            stream_type: Some("converting".to_string()),
        });
    }
    if let Some(elapsed) = parse_ffmpeg_time(&line) {
        let pct = duration_secs
            .filter(|&d| d > 0)
            .map(|d| ((elapsed / d as f32) * 100.0).min(99.0))
            .unwrap_or(0.0);
        on_progress(DownloadProgress {
            task_id: tid.to_string(),
            state: "converting".to_string(),
            progress: pct,
            speed: parse_ffmpeg_speed(&line),
            eta: None,
            downloaded_bytes: None,
            total_bytes: None,
            stream_type: Some("converting".to_string()),
        });
    }

    if is_stderr {
        stderr_buf.push_str(&line);
        stderr_buf.push('\n');
    }
}

/// Прямое ffmpeg-слияние двух потоков (видео + аудио) в целевой контейнер.
/// Используется вместо ручного постпроцессора yt-dlp, чтобы гарантированно оставить
/// исходные потоки до мерджа и иметь контроль над отменой (см. 0.3).
pub async fn run_ffmpeg_merge<F>(
    ytdlp: &YtDlp,
    task_id: &str,
    video_path: &str,
    audio_path: &str,
    output_path: &str,
    ffmpeg_args: &str,
    duration_secs: Option<i64>,
    on_progress: F,
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    let ffmpeg_path = ytdlp
        .ffmpeg_path
        .as_deref()
        .ok_or_else(|| AppError::DownloadFailed("ffmpeg не найден".into()))?;

    info!(
        task_id = %task_id,
        input_video = %video_path,
        input_audio = %audio_path,
        output_path = %output_path,
        ffmpeg_args = %ffmpeg_args,
        "ffmpeg merge: start"
    );

    // Первый converting event — с progress=0.0 для shimmer
    on_progress(DownloadProgress {
        task_id: task_id.to_string(),
        state: "converting".to_string(),
        progress: 0.0,
        speed: None,
        eta: None,
        downloaded_bytes: None,
        total_bytes: None,
        stream_type: Some("converting".to_string()),
    });

    let mut cmd = Command::new(ffmpeg_path);
    no_window(&mut cmd);
    cmd.arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-i")
        .arg(audio_path);
    for arg in ffmpeg_args.split_whitespace() {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }
    cmd.arg(output_path);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::DownloadFailed("ffmpeg stderr capture failed".into()))?;
    let mut lines = BufReader::new(stderr).lines();
    let mut kill_guard = KillOnDrop(Some(child));
    let mut stderr_buf = String::new();

    // Читаем ffmpeg stderr — реальный прогресс транскодирования + ошибки.
    // Отмена НЕ игнорируется: при сигнале — убиваем ffmpeg (KillOnDrop) и возвращаем Err(Cancelled).
    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        stderr_buf.push_str(&l);
                        stderr_buf.push('\n');
                        if let Some(elapsed) = parse_ffmpeg_time(&l) {
                            let pct = duration_secs
                                .filter(|&d| d > 0)
                                .map(|d| ((elapsed / d as f32) * 100.0).min(99.0))
                                .unwrap_or(0.0);
                            on_progress(DownloadProgress {
                                task_id: task_id.to_string(),
                                state: "converting".to_string(),
                                progress: pct,
                                speed: parse_ffmpeg_speed(&l),
                                eta: None,
                                downloaded_bytes: None,
                                total_bytes: None,
                                stream_type: Some("converting".to_string()),
                            });
                        }
                    }
                    _ => break,
                }
            }
            result = &mut cancel_rx => {
                if result.is_ok() {
                    // Явная отмена: KillOnDrop убьёт ffmpeg при drop kill_guard.
                    // НЕ разоружаем kill до фактического завершения.
                    let _ = tokio::fs::remove_file(output_path).await;
                    return Err(AppError::Cancelled);
                }
                // Err(Canceled) — sender дропнут (cancel_forward.abort() после загрузки
                // видео/аудио). Не отмена, а сигнал что cancel-канал закрыт — продолжаем.
                break;
            }
        }
    }

    let mut child = kill_guard.disarm().unwrap();
    let status = child
        .wait()
        .await
        .map_err(|e| AppError::DownloadFailed(format!("ffmpeg wait error: {e}")))?;

    if !status.success() {
        let _ = tokio::fs::remove_file(output_path).await;
        error!(task_id = %task_id, stderr = %stderr_buf, "ffmpeg merge failed");
        return Err(AppError::DownloadFailed(format!(
            "ffmpeg перекодирование не удалось: {}",
            stderr_buf.lines().last().unwrap_or("неизвестная ошибка")
        )));
    }

    // Удаляем исходные потоковые файлы (временные, больше не нужны)
    let _ = tokio::fs::remove_file(video_path).await;
    let _ = tokio::fs::remove_file(audio_path).await;

    info!(task_id = %task_id, output_path = %output_path, "ffmpeg merge finished");
    Ok(output_path.to_string())
}

/// Гарантирует kill дочернего процесса при преждевременном выходе (error/cancel)
struct KillOnDrop(Option<tokio::process::Child>);
impl KillOnDrop {
    fn disarm(&mut self) -> Option<tokio::process::Child> {
        self.0.take()
    }
}
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.start_kill();
        }
    }
}

fn parse_video_info(json: Value, url: &str) -> VideoInfo {
    let formats = json["formats"]
        .as_array()
        .map(|arr| arr.iter().map(parse_format).collect())
        .unwrap_or_default();

    VideoInfo {
        id: json["id"].as_str().unwrap_or_default().to_string(),
        url: url.to_string(),
        title: json["title"].as_str().unwrap_or("Unknown").to_string(),
        channel: json["channel"].as_str().map(String::from),
        channel_avatar: None,
        channel_followers: json["channel_follower_count"].as_i64(),
        uploader_url: json["uploader_url"].as_str().map(String::from),
        thumbnail: json["thumbnail"].as_str().map(String::from),
        duration: json["duration"].as_i64(),
        formats,
        is_playlist: false,
        playlist_count: None,
    }
}

fn parse_format(f: &Value) -> crate::downloader::VideoFormat {
    let vcodec = f["vcodec"]
        .as_str()
        .filter(|&v| v != "none")
        .map(String::from);
    let acodec = f["acodec"]
        .as_str()
        .filter(|&v| v != "none")
        .map(String::from);
    crate::downloader::VideoFormat {
        format_id: f["format_id"].as_str().unwrap_or_default().to_string(),
        ext: f["ext"].as_str().unwrap_or_default().to_string(),
        resolution: f["resolution"].as_str().map(String::from),
        fps: f["fps"].as_f64(),
        vcodec: vcodec.clone(),
        acodec: acodec.clone(),
        abr: f["abr"].as_f64(),
        vbr: f["vbr"].as_f64(),
        filesize: f["filesize"]
            .as_i64()
            .or_else(|| f["filesize_approx"].as_i64()),
        format_note: f["format_note"].as_str().map(String::from),
        is_audio_only: vcodec.is_none() && acodec.is_some(),
    }
}

/// Вывести итоговый путь файла: убрать из yt-dlp дестинейшена суффикс `.fXXX`
/// и заменить расширение на container (mp4, mkv…). Пример:
/// `video.Title.f400.mp4` + container=`mp4` → `video.Title.mp4`
pub fn derive_output_path(downloaded_path: &str, container: &str) -> String {
    let path = Path::new(downloaded_path);
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return downloaded_path.to_string(),
    };
    // Отрезаем `.fXXX` суффикс (формат-id, добавляемый yt-dlp при DASH)
    let clean_stem = match stem.rfind(".f") {
        Some(pos) => &stem[..pos],
        None => stem,
    };
    parent
        .join(format!("{}.{}", clean_stem, container))
        .to_string_lossy()
        .into_owned()
}

/// Извлечь реальный путь файла из строк вывода yt-dlp.
fn parse_destination_line(line: &str) -> Option<String> {
    // "[download] Destination: <path>"
    if let Some(rest) = line.strip_prefix("[download] Destination:") {
        let path = rest.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    // "[ExtractAudio] Destination: <path>" (аудио-режим)
    if let Some(rest) = line.strip_prefix("[ExtractAudio] Destination:") {
        let path = rest.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    // "[ffmpeg] Destination: <path>" (ffmpeg постпроцессор)
    if let Some(rest) = line.strip_prefix("[ffmpeg] Destination:") {
        let path = rest.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    // "[Merger] Merging formats into \"<path>\"" или "Merging formats into \"<path>\""
    if let Some(rest) = line
        .find("Merging formats into \"")
        .map(|i| &line[i + 22..])
    {
        if let Some(end) = rest.rfind('"') {
            let path = rest[..end].to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    // "[download] <path> has already been downloaded"
    if let Some(rest) = line.strip_prefix("[download] ") {
        if let Some(end) = rest.find(" has already been downloaded") {
            let path = rest[..end].trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// Парсит прогресс из числового `--progress-template`.
/// Поля: `status percent speed eta downloaded_bytes total_bytes`
///   percent — f32 без '%'
///   speed  — f64 байт/сек (или "NA")
///   eta    — f64 секунд (или "NA")
///   downloaded_bytes / total_bytes — i64 (или "NA")
fn parse_progress_line(task_id: &str, line: &str) -> Option<DownloadProgress> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let state = parts[0];
    if !matches!(state, "downloading" | "finished" | "error") {
        return None;
    }
    // percent без '%'. yt-dlp часто отдаёт "NA" вместо числа (особенно для
    // DASH-потоков) — тогда считаем сами из downloaded/total байт.
    let progress = if state == "finished" {
        100.0
    } else {
        let raw = parts.get(1).and_then(|s| s.parse::<f32>().ok());
        if let Some(p) = raw.filter(|p| *p > 0.0) {
            p
        } else {
            let d = parts
                .get(4)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let t = parts
                .get(5)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            if t > 0 {
                (d as f32 / t as f32) * 100.0
            } else {
                0.0
            }
        }
    };
    let speed = parts.get(2).and_then(|s| parse_speed_human(s));
    let eta = parts.get(3).and_then(|s| parse_eta_human(s));
    let downloaded_bytes = parts.get(4).and_then(|s| s.parse::<i64>().ok());
    let total_bytes = parts.get(5).and_then(|s| s.parse::<i64>().ok());
    Some(DownloadProgress {
        task_id: task_id.to_string(),
        state: state.to_string(),
        progress,
        speed,
        eta,
        downloaded_bytes,
        total_bytes,
        stream_type: None,
    })
}

/// Форматирует скорость (байт/сек) в человекочитаемую строку "X.X MiB/s" (KiB/B).
fn parse_speed_human(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("NA") || s.eq_ignore_ascii_case("None") {
        return None;
    }
    let bps: f64 = s.parse().ok()?;
    if !bps.is_finite() || bps <= 0.0 {
        return None;
    }
    let (val, unit) = if bps >= (1u64 << 30) as f64 {
        (bps / (1u64 << 30) as f64, "GiB/s")
    } else if bps >= (1u64 << 20) as f64 {
        (bps / (1u64 << 20) as f64, "MiB/s")
    } else if bps >= (1u64 << 10) as f64 {
        (bps / (1u64 << 10) as f64, "KiB/s")
    } else {
        (bps, "B/s")
    };
    Some(format!("{:.1} {}", val, unit))
}

/// Форматирует ETA (секунды) в строку "MM:SS" (None если неизвестно).
fn parse_eta_human(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("NA") || s.eq_ignore_ascii_case("None") {
        return None;
    }
    let secs: f64 = s.parse().ok()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    let total = secs as u64;
    let m = total / 60;
    let sec = total % 60;
    Some(format!("{:02}:{:02}", m, sec))
}

/// Удаляет ANSI escape-последовательности (CSI: ESC [ ... <final>) из строки.
/// Защитная мера — yt-dlp запускается с `--no-color`, но надёжнее очистить явно.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI-последовательность: ESC [ ... <final byte 0x40..=0x7e>
            if chars.peek() == Some(&'[') {
                chars.next(); // съедаем '['
                while let Some(&n) = chars.peek() {
                    let b = n as u8;
                    if (0x40..=0x7e).contains(&b) {
                        chars.next();
                        break;
                    }
                    chars.next();
                }
            } else {
                // Прочие ESC-последовательности: пропускаем следующий символ
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn parse_ytdlp_error(stderr: &str) -> String {
    // Собираем все ERROR: строки — берём последнюю как наиболее конкретную
    let errors: Vec<&str> = stderr.lines().filter(|l| l.contains("ERROR:")).collect();
    if let Some(last) = errors.last() {
        return last.replace("ERROR:", "").trim().to_string();
    }
    stderr
        .lines()
        .last()
        .unwrap_or("неизвестная ошибка")
        .to_string()
}

/// Парсит "time=HH:MM:SS.xx" из строки ffmpeg и возвращает секунды
fn parse_ffmpeg_time(line: &str) -> Option<f32> {
    let pos = line.find("time=")?;
    let s = &line[pos + 5..];
    let end = s
        .find(|c: char| !c.is_ascii_digit() && c != ':' && c != '.')
        .unwrap_or(s.len());
    let ts = &s[..end];
    let parts: Vec<&str> = ts.splitn(3, ':').collect();
    if parts.len() == 3 {
        let h: f32 = parts[0].parse().ok()?;
        let m: f32 = parts[1].parse().ok()?;
        let sec: f32 = parts[2].parse().ok()?;
        Some(h * 3600.0 + m * 60.0 + sec)
    } else {
        None
    }
}

/// Парсит "speed=2.5x" из строки ffmpeg
fn parse_ffmpeg_speed(line: &str) -> Option<String> {
    let pos = line.find("speed=")?;
    let s = &line[pos + 6..].trim_start();
    let end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let val = &s[..end];
    if val.is_empty() || val == "N/A" {
        None
    } else {
        Some(val.to_string())
    }
}

/// Проверяет, содержит ли строка stderr признаки работы ffmpeg
fn is_ffmpeg_line(line: &str) -> bool {
    line.starts_with("[Merger]")
        || line.starts_with("[ExtractAudio]")
        || line.starts_with("[ffmpeg]")
        || line.contains("time=")
}

/// Удаляет осиротевшие частичные файлы после отмены загрузки.
/// Удаляет сами destination-файлы (промежуточные потоки) и их `.part`-сиблингов,
/// а также `.part` итогового `file_path` (и сам итоговый файл, если он частичный).
async fn cleanup_partial_files(destination_paths: &[String], file_path: &Option<String>) {
    for p in destination_paths {
        let _ = tokio::fs::remove_file(p).await;
        let _ = tokio::fs::remove_file(format!("{}.part", p)).await;
    }
    if let Some(fp) = file_path {
        let part = format!("{}.part", fp);
        let _ = tokio::fs::remove_file(&part).await;
        // Удаляем итоговый файл только если он частичный (есть .part-сиблинг),
        // чтобы не стереть уже завершённую загрузку при гонке отмены.
        if Path::new(&part).exists() {
            let _ = tokio::fs::remove_file(fp).await;
        }
    }
}
