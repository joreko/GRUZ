use crate::downloader::{DownloadProgress, VideoInfo};
use crate::error::{AppError, Result};
use crate::ytdlp::YtDlp;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use tracing::{debug, info};
use url::Url;

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

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

async fn fetch_channel_avatar(channel_url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    for _ in 0..2 {
        if let Some(avatar) = fetch_avatar_once(&client, channel_url).await {
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
pub async fn fetch_info(ytdlp: &YtDlp, url: &str, proxy: Option<&str>) -> Result<VideoInfo> {
    // Валидация: URL не должен начинаться с '-' (защита от инъекции аргументов yt-dlp)
    if url.starts_with('-') || url.is_empty() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }
    if Url::parse(url).is_err() {
        return Err(AppError::YtDlp("Некорректный URL".into()));
    }
    let mut cmd = Command::new(&ytdlp.path);
    no_window(&mut cmd);
    cmd.args(["--dump-json", "--no-playlist", "--no-warnings"]);

    if let Some(p) = proxy.filter(|p| !p.is_empty()) {
        cmd.args(["--proxy", p]);
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

/// Запустить загрузку, шлёт прогресс через callback
pub async fn download_video<F>(
    ytdlp: &YtDlp,
    task_id: String,
    url: &str,
    format_id: &str,
    output_template: &str,
    extra_args: &[String],
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

    let mut args = vec![
        "--format".to_string(), format_id.to_string(),
        "--output".to_string(), output_template.to_string(),
        "--newline".to_string(),
        "--progress-template".to_string(),
        "%(progress.status)s %(progress._percent_str)s %(progress._speed_str)s %(progress._eta_str)s %(progress.downloaded_bytes)s %(progress.total_bytes)s".to_string(),
        "--no-warnings".to_string(),
        "--encoding".to_string(), "utf-8".to_string(),
    ];

    // Передаём путь к ffmpeg если он есть — нужен для слияния видео+аудио
    if let Some(ffmpeg) = &ytdlp.ffmpeg_path {
        args.push("--ffmpeg-location".to_string());
        args.push(ffmpeg.to_string_lossy().to_string());
    }

    args.extend_from_slice(extra_args);
    args.push(url.to_string());

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
    let mut stderr_buf = String::new();
    let mut stderr_closed = false;

    loop {
        tokio::select! {
            line = stdout_lines.next_line() => {
                let l = match line {
                    Ok(Some(s)) => s,
                    Ok(None) => break,
                    Err(_) => break,
                };
                if let Some(path) = parse_destination_line(&l) {
                    file_path = Some(path);
                }
                if let Some(progress) = parse_progress_line(&tid, &l) {
                    on_progress(progress);
                }
            }
            line = stderr_lines.next_line(), if !stderr_closed => {
                match line {
                    Ok(Some(l)) => {
                        debug!("yt-dlp: {l}");
                        // Парсим прогресс ffmpeg из stderr: "time=HH:MM:SS.xx"
                        if let (Some(dur), Some(elapsed)) = (duration_secs, parse_ffmpeg_time(&l)) {
                            if dur > 0 {
                                let pct = ((elapsed / dur as f32) * 100.0).min(99.0);
                                on_progress(DownloadProgress {
                                    task_id: tid.clone(),
                                    state: "converting".to_string(),
                                    progress: pct,
                                    speed: parse_ffmpeg_speed(&l),
                                    eta: None,
                                    downloaded_bytes: None,
                                    total_bytes: None,
                                });
                            }
                        }
                        stderr_buf.push_str(&l);
                        stderr_buf.push('\n');
                    }
                    _ => stderr_closed = true,
                }
            }
            _ = &mut cancel_rx => {
                // KillOnDrop сделает kill при drop
                return Err(AppError::Cancelled);
            }
        }
    }

    // Дочитываем оставшийся stderr
    while let Ok(Some(l)) = stderr_lines.next_line().await {
        debug!("yt-dlp: {l}");
        stderr_buf.push_str(&l);
        stderr_buf.push('\n');
    }

    // Забираем child из guard (disarm) — KillOnDrop ничего не делает при drop
    let mut child = kill_guard.disarm().unwrap();
    let status = child.wait().await?;
    info!("yt-dlp exited: {status}, path: {file_path:?}");
    if !status.success() {
        return Err(AppError::DownloadFailed(parse_ytdlp_error(&stderr_buf)));
    }

    file_path.ok_or_else(|| {
        AppError::DownloadFailed("yt-dlp не указал путь к сохранённому файлу".into())
    })
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

fn parse_progress_line(task_id: &str, line: &str) -> Option<DownloadProgress> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let state = parts[0];
    if !matches!(state, "downloading" | "finished" | "error") {
        return None;
    }
    let progress = parts
        .get(1)
        .and_then(|s| s.trim_end_matches('%').parse::<f32>().ok())
        .unwrap_or(0.0);
    let not_na = |s: &&&str| !matches!(**s, "N/A" | "NA" | "");
    Some(DownloadProgress {
        task_id: task_id.to_string(),
        state: state.to_string(),
        progress,
        speed: parts.get(2).filter(not_na).map(|s| s.to_string()),
        eta: parts.get(3).filter(not_na).map(|s| s.to_string()),
        downloaded_bytes: parts.get(4).and_then(|s| s.parse().ok()),
        total_bytes: parts.get(5).and_then(|s| s.parse().ok()),
    })
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
