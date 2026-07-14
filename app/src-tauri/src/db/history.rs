use crate::db::{thumbs_dir, Database};
use crate::error::{AppError, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Колонки history, перечисленные явно, чтобы FromRow совпадал с SELECT.
/// Любое изменение схемы должно отражаться здесь и в add_history/SELECT.
const HISTORY_COLUMNS: &str = "id, url, video_id, platform, title, channel, channel_id, \
    thumbnail, duration, file_path, file_size, format, quality, container, \
    fps, source_fps, bitrate, audio_codec, video_codec, trim_start, trim_end, \
    playlist_id, playlist_index, created_at, \
    local_thumbnail, favorite, deleted_at, duration_real, width, height";

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoryItem {
    pub id: String,
    pub url: String,
    pub video_id: Option<String>,
    pub platform: String,
    pub title: String,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<i64>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub format: String,
    pub quality: String,
    pub container: String,
    pub fps: Option<i64>,
    pub source_fps: Option<i64>,
    pub bitrate: Option<i64>,
    pub audio_codec: Option<String>,
    pub video_codec: Option<String>,
    pub trim_start: Option<i64>,
    pub trim_end: Option<i64>,
    pub playlist_id: Option<String>,
    pub playlist_index: Option<i64>,
    pub created_at: i64,
    /// Локальное превью как готовый asset://localhost URL (NULL = нет/не сгенерировано)
    pub local_thumbnail: Option<String>,
    /// Избранное (по умолчанию false)
    pub favorite: bool,
    /// Время мягкого удаления; NULL = не удалено
    pub deleted_at: Option<i64>,
    /// Реальная длительность из ffmpeg (сек), если удалось извлечь
    pub duration_real: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// Альбом галереи (подборка истории).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Album {
    pub id: String,
    pub name: String,
    /// 'user' | 'smart'
    pub kind: String,
    /// Для smart-альбомов — правило/запрос (опц.)
    pub query: Option<String>,
    pub created_at: i64,
}

impl Database {
    /// Лента истории с курсорной пагинацией и фильтрами.
    /// - before_id: вернуть записи СТАРШЕ этого id (по created_at)
    /// - query: LIKE по title/channel/url (экранирование спецсимволов)
    /// - album_id: только входящие в альбом (сортировка по position)
    /// - favorite == Some(true): только избранное
    /// Мягко удалённые (deleted_at IS NOT NULL) исключаются всегда.
    pub async fn get_history(
        &self,
        before_id: Option<String>,
        limit: i64,
        query: Option<String>,
        album_id: Option<String>,
        favorite: Option<bool>,
    ) -> Result<Vec<HistoryItem>> {
        let mut sql = String::from("SELECT ");
        sql.push_str(HISTORY_COLUMNS);
        sql.push_str(" FROM downloads");

        if album_id.is_some() {
            sql.push_str(" INNER JOIN album_items ON album_items.history_id = downloads.id");
        }

        let mut conds: Vec<String> = Vec::new();
        // Галерея показывает только завершённые загрузки (единая таблица downloads).
        conds.push("downloads.state = 'completed'".to_string());
        conds.push("downloads.deleted_at IS NULL".to_string());
        if before_id.is_some() {
            conds.push(
                "downloads.created_at < (SELECT created_at FROM downloads WHERE id = ?)"
                    .to_string(),
            );
        }
        if favorite == Some(true) {
            conds.push("downloads.favorite = 1".to_string());
        }
        if query.as_deref().map_or(false, |q| !q.is_empty()) {
            conds.push(
                "(downloads.title LIKE ? ESCAPE '\\' OR downloads.channel LIKE ? ESCAPE '\\' OR downloads.url LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
        }
        if album_id.is_some() {
            conds.push("album_items.album_id = ?".to_string());
        }

        if !conds.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conds.join(" AND "));
        }

        if album_id.is_some() {
            sql.push_str(" ORDER BY album_items.position ASC");
        } else {
            sql.push_str(" ORDER BY downloads.created_at DESC");
        }
        sql.push_str(" LIMIT ?");

        let mut q = sqlx::query_as::<_, HistoryItem>(&sql);
        if let Some(bid) = &before_id {
            q = q.bind(bid);
        }
        if let Some(qstr) = &query {
            if !qstr.is_empty() {
                let pattern = like_pattern(qstr);
                // Владеющие копии — живут до fetch_all
                q = q.bind(pattern.clone()).bind(pattern.clone()).bind(pattern);
            }
        }
        if let Some(aid) = &album_id {
            q = q.bind(aid);
        }
        q = q.bind(limit);
        let items = q.fetch_all(&self.pool).await?;
        Ok(items)
    }

    /// Поиск по подстроке. Сохранён для совместимости/fuzzy-поиска на бэке.
    /// Мягко удалённые исключаются.
    #[allow(dead_code)]
    pub async fn search_history(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<HistoryItem>> {
        let pattern = like_pattern(query);
        let items = sqlx::query_as::<_, HistoryItem>(
            "SELECT id, url, video_id, platform, title, channel, channel_id,
             thumbnail, duration, file_path, file_size, format, quality, container,
             fps, source_fps, bitrate, audio_codec, video_codec, trim_start, trim_end,
             playlist_id, playlist_index, created_at,
               local_thumbnail, favorite, deleted_at, duration_real, width, height
               FROM downloads WHERE state = 'completed' AND deleted_at IS NULL AND
              (title LIKE ? ESCAPE '\\' OR channel LIKE ? ESCAPE '\\' OR url LIKE ? ESCAPE '\\')
              ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }

    /// Мягкое удаление: помечаем deleted_at, сами данные не теряем.
    pub async fn delete_history_item(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE downloads SET deleted_at = unixepoch() WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Восстановить мягко удалённую запись.
    pub async fn restore_history_item(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE downloads SET deleted_at = NULL WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Окончательно удалить все мягко удалённые записи.
    pub async fn purge_deleted(&self) -> Result<()> {
        sqlx::query("DELETE FROM downloads WHERE state = 'completed' AND deleted_at IS NOT NULL")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Массовое мягкое удаление по списку id.
    pub async fn delete_history_items(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql =
            format!("UPDATE downloads SET deleted_at = unixepoch() WHERE id IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for id in ids {
            q = q.bind(id);
        }
        q.execute(&self.pool).await?;
        Ok(())
    }

    /// Массово переключить избранное.
    pub async fn set_favorite(&self, ids: &[String], value: bool) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!("UPDATE downloads SET favorite = ? WHERE id IN ({placeholders})");
        let mut q = sqlx::query(&sql).bind(if value { 1i64 } else { 0i64 });
        for id in ids {
            q = q.bind(id);
        }
        q.execute(&self.pool).await?;
        Ok(())
    }

    // ── Альбомы ───────────────────────────────────────────────────────────────

    pub async fn create_album(
        &self,
        name: String,
        kind: String,
        query: Option<String>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO albums (id, name, kind, query, created_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&kind)
        .bind(&query)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_albums(&self) -> Result<Vec<Album>> {
        let albums = sqlx::query_as::<_, Album>(
            "SELECT id, name, kind, query, created_at FROM albums ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(albums)
    }

    pub async fn rename_album(&self, id: &str, name: &str) -> Result<()> {
        sqlx::query("UPDATE albums SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_album(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM album_items WHERE album_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM albums WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Добавить записи в альбом (позиция = конец списка). Идемпотентно (IGNORE).
    pub async fn add_to_album(&self, album_id: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for id in ids {
            sqlx::query(
                "INSERT OR IGNORE INTO album_items (album_id, history_id, position)
                 VALUES (?, ?, COALESCE((SELECT MAX(position) + 1 FROM album_items WHERE album_id = ?), 0))",
            )
            .bind(album_id)
            .bind(id)
            .bind(album_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_from_album(&self, album_id: &str, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "DELETE FROM album_items WHERE album_id = ? AND history_id IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(album_id);
        for id in ids {
            q = q.bind(id);
        }
        q.execute(&self.pool).await?;
        Ok(())
    }

    /// Элементы альбома с курсорной пагинацией по position.
    pub async fn get_album_items(
        &self,
        album_id: &str,
        before_id: Option<String>,
        limit: i64,
    ) -> Result<Vec<HistoryItem>> {
        let mut sql = String::from("SELECT ");
        sql.push_str(HISTORY_COLUMNS);
        sql.push_str(
            " FROM downloads INNER JOIN album_items ON album_items.history_id = downloads.id",
        );
        sql.push_str(" WHERE album_items.album_id = ? AND downloads.state = 'completed' AND downloads.deleted_at IS NULL");
        if before_id.is_some() {
            sql.push_str(
                " AND album_items.position < (SELECT position FROM album_items WHERE album_id = ? AND history_id = ?)",
            );
        }
        sql.push_str(" ORDER BY album_items.position ASC LIMIT ?");

        let mut q = sqlx::query_as::<_, HistoryItem>(&sql).bind(album_id);
        if let Some(bid) = &before_id {
            q = q.bind(album_id).bind(bid);
        }
        q = q.bind(limit);
        let items = q.fetch_all(&self.pool).await?;
        Ok(items)
    }

    /// Вернуть raw file_path для локального файла записи.
    /// Пустая строка, если запись/файл отсутствует.
    /// Фронтенд использует `convertFileSrc()` для конвертации в asset:// URL.
    pub async fn get_media_url(&self, id: &str) -> String {
        sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    /// Сгенерировать локальное превью + извлечь метаданные (ffmpeg).
    /// Максимально устойчивая функция: ЛЮБАЯ ошибка ffmpeg оставляет
    /// local_thumbnail = NULL и логирует warning — никогда не паникует и не
    /// ломает вызывающего (в т.ч. завершение загрузки).
    pub async fn generate_thumbnail(&self, id: &str) -> Result<()> {
        let file_path: Option<String> =
            sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        let Some(file_path) = file_path.filter(|p| !p.is_empty()) else {
            // Нет файла — превью не нужно, это не ошибка
            return Ok(());
        };

        let Some(ffmpeg) = ffmpeg_binary() else {
            tracing::warn!("ffmpeg не найден рядом с exe — превью не сгенерировано для {id}");
            return Ok(());
        };

        let thumbs_dir = thumbs_dir();
        // Создаём папку превью в блокирующем пуле (не держим async runtime)
        tokio::task::spawn_blocking({
            let thumbs_dir = thumbs_dir.clone();
            move || std::fs::create_dir_all(thumbs_dir)
        })
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("task join error: {e}")))?
        .map_err(AppError::Io)?;

        let out_path: PathBuf = thumbs_dir.join(format!("{id}.jpg"));

        // ffmpeg — внешний процесс: запускаем в блокирующем пуле
        let result = tokio::task::spawn_blocking({
            let ffmpeg = ffmpeg.clone();
            let file_path = file_path.clone();
            let out_path = out_path.clone();
            move || run_ffmpeg(&ffmpeg, &file_path, &out_path)
        })
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("ffmpeg task panicked: {e}")))?;

        match result {
            Ok((meta, thumb_ok)) => {
                let local_thumbnail = if thumb_ok && out_path.exists() {
                    Some(out_path.to_string_lossy().to_string())
                } else {
                    None
                };
                sqlx::query(
                    "UPDATE downloads SET local_thumbnail = ?, duration_real = ?, width = ?, height = ? WHERE id = ?",
                )
                .bind(&local_thumbnail)
                .bind(meta.duration)
                .bind(meta.width)
                .bind(meta.height)
                .bind(id)
                .execute(&self.pool)
                .await?;
            }
            Err(e) => {
                tracing::warn!("не удалось получить метаданные/превью для {id}: {e}");
            }
        }
        Ok(())
    }

    pub async fn clear_history(&self) -> Result<()> {
        // Очищаем только завершённые записи галереи (единая таблица downloads).
        // Активные/ожидающие задачи не трогаем.
        sqlx::query("DELETE FROM downloads WHERE state = 'completed'")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Скачанные записи, которым ещё не досчитаны превью/метаданные
    /// (для фонового пересоздания). Уже обработанные пропускаем — иначе
    /// при каждом заходе в галерею перегоняем весь архив впустую.
    pub async fn all_history_ids(&self) -> Result<Vec<String>> {
        let ids = sqlx::query_scalar(
            "SELECT id FROM downloads WHERE state = 'completed' AND deleted_at IS NULL AND file_path IS NOT NULL AND file_path != '' AND (local_thumbnail IS NULL OR width IS NULL OR height IS NULL)",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(ids)
    }
}

/// Экранировать спецсимволы LIKE и обернуть в %...%
fn like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('_', "\\_")
        .replace('%', "\\%");
    format!("%{escaped}%")
}

/// Путь к бандлу ffmpeg рядом с exe (как в ytdlp). None, если бинарь отсутствует.
fn ffmpeg_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let p = dir.join(name);
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// Результат разбора метаданных ffmpeg.
struct MediaMeta {
    duration: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
}

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ffmpeg — внешний процесс: на Windows прячем консольное окно, иначе при
// массовой генерации превью (backfillThumbnails) всплывает куча терминалов.
fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Запустить ffmpeg: (1) probe метаданных из stderr, (2) извлечь кадр ~25%.
/// Возвращает (метаданные, превью_успешно). Ошибка только если ffmpeg вообще
/// не запускается — тогда вызывающий оставляет всё NULL.
fn run_ffmpeg(
    ffmpeg: &Path,
    input: &str,
    out: &Path,
) -> std::result::Result<(MediaMeta, bool), String> {
    // 1. Probe: ffmpeg -i input -f null -  (метаданные в stderr)
    let mut probe_cmd = std::process::Command::new(ffmpeg);
    probe_cmd.args(["-hide_banner", "-i", input, "-f", "null", "-"]);
    no_window(&mut probe_cmd);
    let probe = probe_cmd
        .output()
        .map_err(|e| format!("не удалось запустить ffmpeg: {e}"))?;
    let stderr = String::from_utf8_lossy(&probe.stderr);
    let (duration, width, height, rotation) = parse_ffmpeg_metadata(&stderr);

    // 2. Извлечение кадра только если есть видеопоток
    let mut thumb_ok = false;
    if width.is_some() && height.is_some() {
        // Вертикальные видео хранятся как ландшафт + rotation-метаданные.
        // Чтобы превью и сетка были портретными, поворачиваем кадр до scale.
        let rot_norm = ((rotation.unwrap_or(0) % 360) + 360) % 360;
        let transpose: Option<&str> = match rot_norm {
            90 => Some("transpose=1"),
            270 => Some("transpose=2"),
            180 => Some("hflip,vflip"),
            _ => None,
        };
        let mut filter = String::new();
        if let Some(tf) = transpose {
            filter.push_str(tf);
            filter.push(',');
        }
        filter.push_str("scale='min(480,iw)':-2");

        let mut cmd = std::process::Command::new(ffmpeg);
        no_window(&mut cmd);
        cmd.arg("-y");
        if let Some(d) = duration {
            // 25% от длительности
            cmd.args(["-ss", &format!("{:.3}", (d as f64 * 0.25))]);
        }
        cmd.args(["-i", input, "-frames:v", "1", "-vf", &filter, "-q:v", "3"]);
        cmd.arg(out);
        match cmd.status() {
            Ok(status) if status.success() => thumb_ok = true,
            // Неудача извлечения не фатальна: метаданные уже получены
            Ok(status) => {
                tracing::warn!("ffmpeg extract non-zero exit {status} для {input}");
            }
            Err(e) => {
                tracing::warn!("не удалось запустить ffmpeg (кадр) для {input}: {e}");
            }
        }
    }

    // Размеры с учётом поворота: для 90/270 меняем w/h местами, чтобы
    // вертикальное видео в БД и сетке было портретным.
    let rot_norm = ((rotation.unwrap_or(0) % 360) + 360) % 360;
    let (out_w, out_h) = if matches!(rot_norm, 90 | 270) {
        (height, width)
    } else {
        (width, height)
    };
    let meta = MediaMeta {
        duration,
        width: out_w,
        height: out_h,
    };
    Ok((meta, thumb_ok))
}

/// Разобрать Duration и разрешение видеопотока из вывода ffmpeg.
fn parse_ffmpeg_metadata(stderr: &str) -> (Option<i64>, Option<i64>, Option<i64>, Option<i64>) {
    let mut duration = None;
    let mut width = None;
    let mut height = None;
    let mut rotation = None;

    for line in stderr.lines() {
        if duration.is_none() && line.contains("Duration:") {
            if let Some(pos) = line.find("Duration:") {
                let rest = line[pos + "Duration:".len()..].trim();
                duration = parse_duration(rest);
            }
        }
        if width.is_none() && line.contains("Video:") {
            if let Some((w, h)) = find_resolution(line) {
                width = Some(w);
                height = Some(h);
            }
        }
        if rotation.is_none() {
            rotation = find_rotation(line);
        }
    }
    (duration, width, height, rotation)
}

/// Найти поворот видео в строке вывода ffmpeg (метаданные rotation / displaymatrix).
fn find_rotation(line: &str) -> Option<i64> {
    // "rotate 90" в строке потока Video:
    if let Some(pos) = line.find("rotate") {
        let rest = line[pos + "rotate".len()..].trim_start();
        if let Ok(n) = rest.parse::<i64>() {
            return Some(n);
        }
    }
    // "Side data: displaymatrix: rotation of -90.00 degrees"
    if let Some(pos) = line.find("rotation of") {
        let rest = &line[pos + "rotation of".len()..];
        if let Some(end) = rest.find("degrees") {
            if let Ok(n) = rest[..end].trim().parse::<f64>() {
                return Some(n.round() as i64);
            }
        }
    }
    None
}

/// "00:01:23.45," -> секунды (округлённо)
fn parse_duration(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches(',');
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = parts[1].parse().ok()?;
    let sec: f64 = parts[2].parse().ok()?;
    Some((h * 3600.0 + m * 60.0 + sec).round() as i64)
}

/// Найти первое вхождение WxH (минимум 2 цифры) в строке потока.
fn find_resolution(line: &str) -> Option<(i64, i64)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'x' {
                let h_start = i + 1;
                let mut j = h_start;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > h_start {
                    let w: i64 = line[start..i].parse().ok()?;
                    let h: i64 = line[h_start..j].parse().ok()?;
                    if w >= 2 && h >= 2 {
                        return Some((w, h));
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    None
}
