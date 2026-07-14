// Типы, зеркалящие Rust-структуры. Обновлять синхронно с src-tauri/src/

export type Route = 'download' | 'gallery' | 'settings' | 'save-settings' | 'updates' | 'editor' | 'storage' | 'scheduler' | 'channels' | 'orchestrator' | 'graph' | 'debug'

export type TaskState =
  | 'waiting' | 'downloading' | 'converting'
  | 'completed' | 'failed' | 'cancelled'

export type Priority = 'low' | 'normal' | 'high'

export interface DownloadTask {
  id: string
  url: string
  title: string | null
  thumbnail: string | null
  channel: string | null
  duration: number | null
  is_playlist: boolean
  format: string
  quality: string
  container: string
  fps: number | null
  source_fps: number | null
  bitrate: number | null
  video_codec: string | null
  audio_codec: string | null
  video_id: string | null
  channel_id: string | null
  platform: string
  trim_start: number | null
  trim_end: number | null
  state: TaskState
  priority: Priority
  progress: number
  speed: string | null
  eta: string | null
  error: string | null
  file_path: string | null
  file_size: number | null
  created_at: string
  stream_type: string | null
}

export interface VideoInfo {
  id: string
  url: string
  title: string
  channel: string | null
  channel_avatar: string | null
  channel_followers: number | null
  uploader_url: string | null
  thumbnail: string | null
  duration: number | null
  formats: VideoFormat[]
  is_playlist: boolean
  playlist_count: number | null
}

export interface VideoFormat {
  format_id: string
  ext: string
  resolution: string | null
  fps: number | null
  vcodec: string | null
  acodec: string | null
  abr: number | null
  vbr: number | null
  filesize: number | null
  format_note: string | null
  is_audio_only: boolean
}

export interface HistoryItem {
  id: string
  url: string
  title: string
  channel: string | null
  thumbnail: string | null
  duration: number | null
  file_path: string
  file_size: number | null
  format: string
  quality: string
  container: string
  video_id: string | null
  platform: string
  channel_id: string | null
  fps: number | null
  source_fps: number | null
  bitrate: number | null
  audio_codec: string | null
  video_codec: string | null
  trim_start: number | null
  trim_end: number | null
  playlist_id: string | null
  playlist_index: number | null
  created_at: number  // Unix timestamp (секунды)
  // v2-поля (локальные превью, избранное, soft-delete, реальные метаданные)
  local_thumbnail: string | null
  favorite: boolean
  deleted_at: number | null
  duration_real: number | null
  width: number | null
  height: number | null
}

export interface Album {
  id: string
  name: string
  kind: string  // 'user' | 'smart'
  query: string | null
  created_at: number
}

export interface ChannelPrefs {
  channel_id: string
  channel_name: string
  platform: string
  format: string | null
  quality: string | null
  container: string | null
  audio_codec: string | null
  video_codec: string | null
  fps: number | null
  download_dir: string | null
  updated_at: number
}

export interface Session {
  id: number
  last_url: string | null
  last_dir: string | null
  window_x: number | null
  window_y: number | null
  window_w: number | null
  window_h: number | null
  updated_at: number
}

export interface Settings {
  download_dir: string
  max_concurrent: number
  default_format: string
  default_quality: string
  default_container: string
  default_fps: number | null
  default_bitrate: number | null
  default_video_codec: string | null
  default_audio_codec: string | null
  auto_merge: boolean
  embed_subtitles: boolean
  proxy: string
  ytdlp_extra_args: string
  theme: string
  minimize_to_tray: boolean
  // Папки сохранения по типу контента (пусто = download_dir)
  save_dir_video: string
  save_dir_audio: string
  save_dir_playlist: string
  save_dir_shorts: string
  save_dir_trimmed: string
  // Шаблоны имён файлов по типу контента
  save_tpl_video: string
  save_tpl_audio: string
  save_tpl_playlist: string
  save_tpl_shorts: string
  save_tpl_trimmed: string
}

export interface ShortcutInfo {
  name: string
  path: string
  target: string
  // 'start_menu' | 'desktop'
  location: string
  // указывает на gruz.exe
  is_gruz: boolean
  // указывает на gruz.exe, но имя отличается от «Груз» (поломанное)
  is_broken: boolean
}

export interface StartDownloadRequest {
  url: string
  format: string
  quality: string
  fps: number | null
  source_fps: number | null
  bitrate: number | null
  container: string
  title: string | null
  thumbnail: string | null
  channel: string | null
  duration: number | null
  is_playlist: boolean
  audio_codec: string | null
  video_codec: string | null
}

export interface DownloadProgress {
  task_id: string
  state: 'downloading' | 'finished' | 'error' | 'converting'
  progress: number
  speed: string | null
  eta: string | null
  downloaded_bytes: number | null
  total_bytes: number | null
  stream_type: string | null
}
