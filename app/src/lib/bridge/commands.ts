import { invoke } from '@tauri-apps/api/core'
import type { VideoInfo, DownloadTask, HistoryItem, Settings, StartDownloadRequest, ChannelPrefs, Session } from './types'

// Единственное место в коде где живёт invoke()

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

  setTaskPriority: (taskId: string, priority: string) =>
    invoke<void>('set_task_priority', { taskId, priority }),

  getHistory: (limit = 50, offset = 0, query?: string) =>
    invoke<HistoryItem[]>('get_history', { limit, offset, query }),

  searchHistory: (query: string, limit = 50, offset = 0) =>
    invoke<HistoryItem[]>('search_history', { query, limit, offset }),

  deleteHistoryItem: (id: string) =>
    invoke<void>('delete_history_item', { id }),

  clearHistory: () =>
    invoke<void>('clear_history'),

  openFile: (path: string) =>
    invoke<void>('open_file', { path }),

  openFolder: (path: string) =>
    invoke<void>('open_folder', { path }),

  getSettings: () =>
    invoke<Settings>('get_settings'),

  updateSetting: (key: string, value: string) =>
    invoke<void>('update_settings', { key, value }),

  getFreeSpace: (path: string) =>
    invoke<number>('get_free_space', { path }),

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
}
