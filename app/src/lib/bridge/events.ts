import { listen } from '@tauri-apps/api/event'
import type { DownloadProgress } from './types'

// Единственное место где живут listen()

export function onQueueUpdated(cb: () => void) {
  return listen<void>('queue:updated', cb)
}

export function onHistoryUpdated(cb: () => void) {
  return listen<void>('history:updated', cb)
}

export function onDownloadProgress(cb: (p: DownloadProgress) => void) {
  return listen<DownloadProgress>('download:progress', (e) => cb(e.payload))
}

export interface DownloadCompletedPayload {
  task_id: string
  title: string | null
  file_path: string | null
}

export function onDownloadCompleted(cb: (p: DownloadCompletedPayload) => void) {
  return listen<DownloadCompletedPayload>('download:completed', (e) => cb(e.payload))
}

export interface DownloadFailedPayload {
  task_id: string
  error: string | null
}

export function onDownloadFailed(cb: (p: DownloadFailedPayload) => void) {
  return listen<DownloadFailedPayload>('download:failed', (e) => cb(e.payload))
}

export interface OrchestratorThought {
  kind: string // "started" | "progress" | "completed" | "error" | "recovered" | "idle" | "info" | "chatter"
  text: string // собранная фраза (бэкенд подставляет название/прогресс)
  severity: string // "info" | "success" | "warn" | "error" | "muted"
  title: string | null // название задачи (если есть)
  progress: number | null // 0..100 для kind=progress
  description: string | null // человеческое пояснение: что и почему
  ts: number // unix-ms для ленты на странице оркестратора
}

export function onOrchestratorThought(cb: (t: OrchestratorThought) => void) {
  return listen<OrchestratorThought>('orchestrator:thought', (e) => cb(e.payload))
}

// Живой лог: каждая строка лога бэкенда прилетает как событие `log:line`.
export function onLogLine(cb: (line: string) => void) {
  return listen<string>('log:line', (e) => cb(e.payload))
}

export interface UpdateProgress {
  downloaded: number
  total: number | null
  pct: number | null
  done: boolean
}

export function onUpdateProgress(cb: (p: UpdateProgress) => void) {
  return listen<UpdateProgress>('update:progress', (e) => cb(e.payload))
}
