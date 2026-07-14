import { invoke } from '@tauri-apps/api/core'
import { openUrl as pluginOpenUrl } from '@tauri-apps/plugin-opener'
import { writeText as pluginWriteText, readText as pluginReadText } from '@tauri-apps/plugin-clipboard-manager'
import { open as pluginOpen } from '@tauri-apps/plugin-dialog'
import type { VideoInfo, DownloadTask, HistoryItem, Settings, StartDownloadRequest, ChannelPrefs, Session, Priority, Album, ShortcutInfo } from './types'

// Единственное место в коде где живёт invoke() и плагин-вызовы

export const commands = {
  fetchInfo: (url: string) =>
    invoke<VideoInfo>('fetch_info', { url }),

  startDownload: (req: StartDownloadRequest) =>
    invoke<DownloadTask>('start_download', { req }),

  cancelDownload: (taskId: string) =>
    invoke<void>('cancel_download', { taskId }),

  getQueue: () =>
    invoke<DownloadTask[]>('get_queue'),

  removeTask: (taskId: string) =>
    invoke<void>('remove_task', { taskId }),

  clearQueue: () =>
    invoke<void>('clear_queue'),

  setTaskPriority: (taskId: string, priority: Priority) =>
    invoke<void>('set_task_priority', { taskId, priority }),

  reorderTask: (taskId: string, newIndex: number) =>
    invoke<void>('reorder_task', { taskId, newIndex }),

  retryTask: (taskId: string) =>
    invoke<void>('retry_task', { taskId }),

  // Курсорная пагинация + фильтры (замена старого get_history)
  getHistory: (
    beforeId: string | null = null,
    limit = 1000,
    query: string | null = null,
    albumId: string | null = null,
    favorite: boolean | null = null,
  ) =>
    invoke<HistoryItem[]>('get_history', { beforeId, limit, query, albumId, favorite }),

  // Мягкое удаление (soft-delete). withFile=true — ещё и файл с диска.
  deleteHistoryItem: (id: string, withFile = false) =>
    invoke<void>('delete_history_item', { id, withFile }),

  deleteHistoryItemWithFile: (id: string) =>
    invoke<void>('delete_history_item', { id, withFile: true }),

  deleteHistoryItems: (ids: string[]) =>
    invoke<void>('delete_history_items', { ids }),

  restoreHistoryItem: (id: string) =>
    invoke<void>('restore_history_item', { id }),

  purgeDeleted: () =>
    invoke<void>('purge_deleted'),

  setFavorite: (ids: string[], value: boolean) =>
    invoke<void>('set_favorite', { ids, value }),

  clearHistory: () =>
    invoke<void>('clear_history'),

  // Альбомы
  createAlbum: (name: string, kind: string, query?: string) =>
    invoke<string>('create_album', { name, kind, query }),

  listAlbums: () =>
    invoke<Album[]>('list_albums'),

  renameAlbum: (id: string, name: string) =>
    invoke<void>('rename_album', { id, name }),

  deleteAlbum: (id: string) =>
    invoke<void>('delete_album', { id }),

  addToAlbum: (albumId: string, ids: string[]) =>
    invoke<void>('add_to_album', { albumId, ids }),

  removeFromAlbum: (albumId: string, ids: string[]) =>
    invoke<void>('remove_from_album', { albumId, ids }),

  getAlbumItems: (albumId: string, beforeId: string | null, limit: number) =>
    invoke<HistoryItem[]>('get_album_items', { albumId, beforeId, limit }),

  // Локальные превью и медиа-URL
  generateThumbnail: (id: string) =>
    invoke<void>('generate_thumbnail', { id }),

  backfillThumbnails: () =>
    invoke<void>('backfill_thumbnails'),

  getMediaUrl: (id: string) =>
    invoke<string>('get_media_url', { id }),

  openFile: (path: string) =>
    invoke<void>('open_file', { path }),

  openFolder: (path: string) =>
    invoke<void>('open_folder', { path }),

  getSettings: () =>
    invoke<Settings>('get_settings'),

  updateSetting: (key: string, value: string, silent?: boolean) =>
    invoke<void>('update_settings', { key, value, silent }),

  getSetting: (key: string) =>
    invoke<string | null>('get_setting', { key }),

  getFreeSpace: (path: string) =>
    invoke<number | null>('get_free_space', { path }),

  // Настройки каналов
  listChannelPrefs: () =>
    invoke<ChannelPrefs[]>('list_channel_prefs'),

  upsertChannelPrefs: (prefs: ChannelPrefs) =>
    invoke<void>('upsert_channel_prefs', { prefs }),

  deleteChannelPrefs: (channelId: string) =>
    invoke<void>('delete_channel_prefs', { channelId }),

  // Сессия
  getSession: () =>
    invoke<Session>('get_session'),

  updateSession: (session: Session) =>
    invoke<void>('update_session', { session }),
  // Обновление
  installVersion: (url: string) =>
    invoke<void>('install_version', { url }),

  // Поддержка
  uploadLog: () =>
    invoke<string>('upload_log'),

  setLogLevel: (level: string) =>
    invoke<void>('set_log_level', { level }),

  getLogHistory: () =>
    invoke<string[]>('get_log_history'),

  logFrontend: (message: string, level?: string) =>
    invoke<void>('log_frontend', { message, level }),

  // Плагины (bridge: не импортировать plugin-* в компонентах)
  openUrl: (url: string) =>
    pluginOpenUrl(url),

  writeText: (text: string) =>
    pluginWriteText(text),
  readText: () =>
    pluginReadText(),

  pickDirectory: (defaultPath?: string) =>
    pluginOpen({ directory: true, multiple: false, defaultPath }),

  pickFile: (filters?: { name: string; extensions: string[] }[]) =>
    pluginOpen({ multiple: false, filters }),

  // Ярлыки приложения (поиск, починка, настройка)
  listShortcuts: () =>
    invoke<ShortcutInfo[]>('list_shortcuts'),

  fixShortcut: (path: string) =>
    invoke<void>('fix_shortcut', { path }),

  removeShortcut: (path: string) =>
    invoke<void>('remove_shortcut', { path }),

  setShortcut: (location: string, enabled: boolean) =>
    invoke<void>('set_shortcut', { location, enabled }),
}
