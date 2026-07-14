use crate::db::settings::Settings;
use crate::queue::task::DownloadTask;

/// Режим исполнения конвейера загрузки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanMode {
    /// Один вызов yt-dlp с готовым `-f` селектором (audio / video_only / video-copy).
    Single,
    /// Видео и аудио качаются отдельно, затем сливаются ffmpeg вручную
    /// (нужно при перекодировании / выборе кодека / ProRes).
    SeparateStreams,
}

/// Полная спецификация того, КАК скачать задачу: селекторы форматов,
/// аргументы постпроцессора yt-dlp, пути вывода, режим.
///
/// Чистая функция `build_spec` (без I/O) — покрывается юнит-тестами.
pub struct DownloadSpec {
    /// yt-dlp `-f` селектор (для Single — полный, для SeparateStreams — только видео).
    pub video_selector: String,
    /// Селектор аудиопотока (только для SeparateStreams).
    pub audio_selector: Option<String>,
    /// Аргументы постпроцессора yt-dlp (`--extract-audio`, `--merge-output-format` и т.п.).
    pub post_args: Vec<String>,
    /// Итоговый output template для yt-dlp.
    pub output_template: String,
    /// Output template для отдельного аудиопотока (только SeparateStreams).
    pub audio_template: Option<String>,
    /// Контейнер слияния (`mp4`/`mkv`/`webm`/`mov`).
    pub merge_fmt: &'static str,
    /// Аргументы ffmpeg для ручного слияния (только SeparateStreams).
    pub ffmpeg_merge_args: Option<String>,
    pub mode: PlanMode,
}

/// Построить спецификацию загрузки из задачи и настроек.
/// Без сети и диска — чистая логика выбора формата и сборки аргументов.
pub fn build_spec(task: &DownloadTask, settings: &Settings) -> DownloadSpec {
    let output_template = resolve_output_template(settings, task);
    let fps_filter = task
        .fps
        .map(|f| format!("[fps<={}]", f))
        .unwrap_or_default();

    match task.format.as_str() {
        "audio" => {
            let ext = match task.container.as_str() {
                "mp3" => "mp3",
                "m4a" => "m4a",
                "opus" => "opus",
                "flac" => "flac",
                _ => "mp3",
            };
            let audio_fmt = if task.quality.is_empty() {
                "bestaudio/best".to_string()
            } else {
                format!("{}/bestaudio/best", task.quality)
            };
            let mut post = vec![
                "--extract-audio".to_string(),
                "--audio-format".to_string(),
                ext.to_string(),
            ];
            let mut ff = String::new();
            if let Some(br) = task.bitrate.filter(|b| *b > 0) {
                ff.push_str(&format!("-b:a {}k", br));
            }
            if let Some(ref codec) = task.audio_codec {
                let alib = match codec.as_str() {
                    "aac" => "aac",
                    "opus" => "libopus",
                    "mp3" => "libmp3lame",
                    "flac" => "flac",
                    "ac3" => "ac3",
                    _ => "",
                };
                if !alib.is_empty() {
                    if !ff.is_empty() {
                        ff.push(' ');
                    }
                    ff.push_str(&format!("-c:a {}", alib));
                }
            }
            if ff.is_empty() {
                post.extend_from_slice(&["--audio-quality".to_string(), "0".to_string()]);
            } else {
                post.extend_from_slice(&["--postprocessor-args".to_string(), ff]);
            }
            DownloadSpec {
                video_selector: audio_fmt,
                audio_selector: None,
                post_args: post,
                output_template,
                audio_template: None,
                merge_fmt: "mp3",
                ffmpeg_merge_args: None,
                mode: PlanMode::Single,
            }
        }
        "video_only" => {
            let sel = if task.quality.is_empty() {
                format!("bestvideo{}", fps_filter)
            } else {
                format!(
                    "{}{}/bestvideo{}/best",
                    task.quality, fps_filter, fps_filter
                )
            };
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
            let vlib = match (task.video_codec.as_deref(), task.bitrate.filter(|b| *b > 0)) {
                (Some("h264"), _) => Some("libx264"),
                (Some("h265"), _) => Some("libx265"),
                (Some("vp9"), _) => Some("libvpx-vp9"),
                (Some("av1"), _) => Some("libaom-av1"),
                (Some("prores"), _) => Some("prores_ks"),
                (Some(_), _) => Some("copy"),
                (None, Some(_)) => Some("libx264"),
                (None, None) => None,
            };
            if let Some(vlib) = vlib {
                let mut ff = format!("-c:v {}", vlib);
                if let Some(br) = task.bitrate.filter(|b| *b > 0) {
                    ff.push_str(&format!(" -b:v {}k", br));
                }
                post.extend_from_slice(&["--postprocessor-args".to_string(), ff]);
            }
            DownloadSpec {
                video_selector: sel,
                audio_selector: None,
                post_args: post,
                output_template,
                audio_template: None,
                merge_fmt,
                ffmpeg_merge_args: None,
                mode: PlanMode::Single,
            }
        }
        _ => {
            // Аудио-поток: fallback если конкретный ext недоступен
            let audio_sel = audio_selector(task);
            // Основной выбор + fallback. Конкретные format_id на YouTube
            // часто ротируются → "Requested format is not available". Цепочка
            // альтернатив слева направо: точный выбор, затем bestvideo+bestaudio, затем best.
            let primary = if task.quality.is_empty() {
                format!("bestvideo{}", fps_filter)
            } else {
                format!("{}{}", task.quality, fps_filter)
            };
            let sel = format!("{}+{}", primary, audio_sel);
            let sel = format!("{}/bestvideo{}+bestaudio/best", sel, fps_filter);
            let merge_fmt = match task.container.as_str() {
                "mkv" => "mkv",
                "webm" => "webm",
                "mov" => "mov",
                _ => "mp4",
            };

            let vlib = task.video_codec.as_deref().map(|c| match c {
                "h264" => "libx264",
                "h265" => "libx265",
                "vp9" => "libvpx-vp9",
                "av1" => "libaom-av1",
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
                let br_part = task
                    .bitrate
                    .filter(|b| *b > 0)
                    .map(|b| format!(" -b:v {}k", b))
                    .unwrap_or_default();
                let preset = match vc {
                    "libx264" | "libx265" => " -preset veryfast",
                    "libvpx-vp9" => " -deadline good -cpu-used 4",
                    "libaom-av1" => " -cpu-used 6",
                    _ => "",
                };
                let mut ffargs = format!("-c:v {}{}{} -c:a {}", vc, br_part, preset, ac);
                if effective_merge_fmt == "mov" {
                    ffargs.push_str(" -movflags +faststart");
                }

                let video_sel = format!(
                    "{}{}/bestvideo{}/best",
                    task.quality, fps_filter, fps_filter
                );
                let audio_tpl = derive_audio_template(&output_template);
                DownloadSpec {
                    video_selector: video_sel,
                    audio_selector: Some(audio_sel),
                    post_args: Vec::new(),
                    output_template,
                    audio_template: Some(audio_tpl),
                    merge_fmt: effective_merge_fmt,
                    ffmpeg_merge_args: Some(ffargs),
                    mode: PlanMode::SeparateStreams,
                }
            } else {
                // Без явного перекодирования: форсируем -c copy.
                let copy_args = if let Some(br) = task.bitrate.filter(|b| *b > 0) {
                    format!("ffmpeg:-c:v libx264 -b:v {}k -c:a copy", br)
                } else {
                    "ffmpeg:-c copy".to_string()
                };
                post.extend_from_slice(&["--postprocessor-args".to_string(), copy_args]);
                DownloadSpec {
                    video_selector: sel,
                    audio_selector: None,
                    post_args: post,
                    output_template,
                    audio_template: None,
                    merge_fmt,
                    ffmpeg_merge_args: None,
                    mode: PlanMode::Single,
                }
            }
        }
    }
}

/// Единый выбор аудиопотока (раньше дублировался в двух ветках).
fn audio_selector(task: &DownloadTask) -> String {
    if let Some(ref codec) = task.audio_codec {
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
    }
}

/// Суффикс `.audio` для отдельного аудиопотока, чтобы не конфликтовать с видео-файлом.
fn derive_audio_template(output_template: &str) -> String {
    if output_template.contains(".%(ext)s") {
        output_template.replace(".%(ext)s", ".audio.%(ext)s")
    } else {
        match output_template.rfind('.') {
            Some(pos) => format!(
                "{}.audio{}",
                &output_template[..pos],
                &output_template[pos..]
            ),
            None => format!("{}.audio", output_template),
        }
    }
}

/// Разбор строки доп. аргументов с учётом кавычек (совпадает с downloader::process).
pub fn split_args(s: &str) -> Vec<String> {
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

/// Добавить прокси и ytdlp_extra_args (без embed-subs) — для отдельных потоков.
pub fn add_base_args(mut args: Vec<String>, settings: &Settings) -> Vec<String> {
    if !settings.proxy.is_empty() {
        args.push("--proxy".into());
        args.push(settings.proxy.clone());
    }
    for arg in split_args(&settings.ytdlp_extra_args) {
        args.push(arg);
    }
    args
}

/// Добавить embed-subs поверх base-аргументов (для single-загрузки).
pub fn add_settings_args(args: Vec<String>, settings: &Settings) -> Vec<String> {
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
        return std::path::Path::new(&fallback)
            .join(tpl.as_str())
            .to_string_lossy()
            .into_owned();
    }
    std::path::Path::new(base.as_str())
        .join(tpl.as_str())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::settings::Settings;
    use crate::queue::task::DownloadTask;

    fn base_settings() -> Settings {
        // Settings без Default — собираем минимальный экземпляр с разумными
        // значениями по умолчанию (как в get_settings при отсутствии строки).
        Settings {
            download_dir: String::new(),
            max_concurrent: 3,
            default_format: "video".into(),
            default_quality: "best".into(),
            default_container: "mp4".into(),
            default_fps: None,
            default_bitrate: None,
            default_video_codec: None,
            default_audio_codec: None,
            auto_merge: true,
            embed_subtitles: false,
            proxy: String::new(),
            ytdlp_extra_args: String::new(),
            theme: "dark".into(),
            minimize_to_tray: true,
            save_dir_video: String::new(),
            save_dir_audio: String::new(),
            save_dir_playlist: String::new(),
            save_dir_shorts: String::new(),
            save_dir_trimmed: String::new(),
            save_tpl_video: "%(title)s.%(ext)s".into(),
            save_tpl_audio: "%(title)s.%(ext)s".into(),
            save_tpl_playlist: "%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s".into(),
            save_tpl_shorts: "Shorts/%(title)s.%(ext)s".into(),
            save_tpl_trimmed: "%(title)s [trimmed].%(ext)s".into(),
        }
    }

    fn task(format: &str, container: &str) -> DownloadTask {
        let mut t = DownloadTask::new(
            "https://youtube.com/watch?v=abc".into(),
            format.into(),
            String::new(),
            container.into(),
        );
        t.title = Some("Тестовое видео".into());
        t
    }

    #[test]
    fn audio_mp3_uses_extract_audio() {
        let s = build_spec(&task("audio", "mp3"), &base_settings());
        assert_eq!(s.mode, PlanMode::Single);
        assert!(s.post_args.iter().any(|a| a == "--extract-audio"));
        assert!(s.post_args.iter().any(|a| a == "mp3"));
    }

    #[test]
    fn video_default_is_single_copy() {
        let s = build_spec(&task("video", "mp4"), &base_settings());
        // Нет кодека/битрейта → single с -c copy
        assert_eq!(s.mode, PlanMode::Single);
        assert!(s.post_args.iter().any(|a| a.contains("-c copy")));
    }

    #[test]
    fn video_h264_separate_streams() {
        let mut t = task("video", "mp4");
        t.video_codec = Some("h264".into());
        let s = build_spec(&t, &base_settings());
        assert_eq!(s.mode, PlanMode::SeparateStreams);
        assert!(s.audio_selector.is_some());
        assert!(s.ffmpeg_merge_args.is_some());
        assert!(s.ffmpeg_merge_args.as_ref().unwrap().contains("libx264"));
    }

    #[test]
    fn shorts_template_chosen() {
        let mut t = task("video", "mp4");
        t.url = "https://youtube.com/shorts/xyz".into();
        let s = build_spec(&t, &base_settings());
        assert!(
            s.output_template.contains("shorts") || s.output_template.contains("Short"),
            "ожидался shorts-шаблон: {}",
            s.output_template
        );
    }
}
