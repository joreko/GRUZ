use crate::downloader::{DownloadProgress, VideoInfo};
use crate::error::{AppError, Result};
use crate::ytdlp::YtDlp;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use tracing::{debug, info};

fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(target_os = "windows")]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Получить URL аватарки канала по URL страницы канала
async fn fetch_channel_avatar(channel_url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let html = client.get(channel_url).send().await.ok()?.text().await.ok()?;

    // Паттерн в ytInitialData: "avatar":{"thumbnails":[{"url":"https://..."}
    let marker = "\"avatar\":{\"thumbnails\":[{\"url\":\"";
    if let Some(pos) = html.find(marker) {
        let rest = &html[pos + marker.len()..];
        // Пропускаем \" если есть (экранирование в JSON-in-HTML)
        let rest = rest.trim_start_matches('\\').trim_start_matches('"');
        if let Some(end) = rest.find('"') {
            let img_url = rest[..end].replace("\\u0026", "&");
            if img_url.starts_with("http") {
                // Скачиваем и отдаём как base64 data URL — WebView не блокирует
                if let Ok(resp) = client.get(&img_url).send().await {
                    let mime = resp.headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("image/jpeg")
                        .to_string();
                    if let Ok(bytes) = resp.bytes().await {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                        return Some(format!("data:{};base64,{}", mime, b64));
                    }
                }
            }
        }
    }
    None
}

/// Получить метаданные видео (без загрузки)
pub async fn fetch_info(ytdlp: &YtDlp, url: &str) -> Result<VideoInfo> {
    let output = no_window(Command::new(&ytdlp.path)
        .args(["--dump-json", "--no-playlist", "--no-warnings", url]))
        .output()
        .await?;

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
        info!("channel_avatar: {:?}", info.channel_avatar.as_deref().map(|u| &u[..u.len().min(60)]));
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
) -> Result<String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
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

    let mut child = no_window(Command::new(&ytdlp.path)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped()))
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout был захвачен при spawn");
    let stderr  = child.stderr.take().expect("stderr был захвачен при spawn");
    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut stderr_lines = BufReader::new(stderr).lines();
    let tid = task_id.clone();

    let mut file_path: Option<String> = None;
    let mut stderr_buf = String::new();

    loop {
        tokio::select! {
            line = stdout_lines.next_line() => {
                let l = match line {
                    Ok(Some(s)) => s,
                    Ok(None) => break,
                    Err(_) => continue,
                };
                if let Some(path) = parse_destination_line(&l) {
                    file_path = Some(path);
                }
                if let Some(progress) = parse_progress_line(&tid, &l) {
                    on_progress(progress);
                }
            }
            line = stderr_lines.next_line() => {
                if let Ok(Some(l)) = line {
                    debug!("yt-dlp: {l}");
                    stderr_buf.push_str(&l);
                    stderr_buf.push('\n');
                }
            }
            _ = &mut cancel_rx => {
                child.kill().await.ok();
                return Err(AppError::DownloadFailed("cancelled".into()));
            }
        }
    }

    // Дочитываем оставшийся stderr
    while let Ok(Some(l)) = stderr_lines.next_line().await {
        debug!("yt-dlp: {l}");
        stderr_buf.push_str(&l);
        stderr_buf.push('\n');
    }

    let status = child.wait().await?;
    info!("yt-dlp exited: {status}, path: {file_path:?}");
    if !status.success() {
        return Err(AppError::DownloadFailed(parse_ytdlp_error(&stderr_buf)));
    }

    Ok(file_path.unwrap_or_else(|| output_template.to_string()))
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
    let vcodec = f["vcodec"].as_str().filter(|&v| v != "none").map(String::from);
    let acodec = f["acodec"].as_str().filter(|&v| v != "none").map(String::from);
    crate::downloader::VideoFormat {
        format_id: f["format_id"].as_str().unwrap_or_default().to_string(),
        ext: f["ext"].as_str().unwrap_or_default().to_string(),
        resolution: f["resolution"].as_str().map(String::from),
        fps: f["fps"].as_f64(),
        vcodec: vcodec.clone(),
        acodec: acodec.clone(),
        abr: f["abr"].as_f64(),
        vbr: f["vbr"].as_f64(),
        filesize: f["filesize"].as_i64().or_else(|| f["filesize_approx"].as_i64()),
        format_note: f["format_note"].as_str().map(String::from),
        is_audio_only: vcodec.is_none() && acodec.is_some(),
    }
}

/// Извлечь реальный путь файла из строк вывода yt-dlp.
/// Форматы:
///   [download] Destination: /path/to/file.mp4
///   Merging formats into "/path/to/file.mp4"
fn parse_destination_line(line: &str) -> Option<String> {
    // "[download] Destination: <path>"
    if let Some(rest) = line.strip_prefix("[download] Destination:") {
        let path = rest.trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    // "Merging formats into \"<path>\""
    if let Some(rest) = line.find("Merging formats into \"").map(|i| &line[i + 22..]) {
        if let Some(end) = rest.rfind('"') {
            let path = rest[..end].to_string();
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
    let progress = parts.get(1).and_then(|s| s.trim_end_matches('%').parse::<f32>().ok()).unwrap_or(0.0);
    Some(DownloadProgress {
        task_id: task_id.to_string(),
        state: state.to_string(),
        progress,
        speed: parts.get(2).map(|s| s.to_string()),
        eta: parts.get(3).map(|s| s.to_string()),
        downloaded_bytes: parts.get(4).and_then(|s| s.parse().ok()),
        total_bytes: parts.get(5).and_then(|s| s.parse().ok()),
    })
}

fn parse_ytdlp_error(stderr: &str) -> String {
    // Извлечь суть из многословного вывода yt-dlp
    for line in stderr.lines() {
        if line.contains("ERROR:") {
            return line.replace("ERROR:", "").trim().to_string();
        }
    }
    stderr.lines().last().unwrap_or("unknown error").to_string()
}
