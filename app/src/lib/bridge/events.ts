import { listen } from '@tauri-apps/api/event'
import type { DownloadProgress } from './types'

// Единственное место где живут listen()

export function onQueueUpdated(cb: () => void) {
  return listen<void>('queue:updated', cb)
}

export function onDownloadProgress(cb: (p: DownloadProgress) => void) {
  return listen<DownloadProgress>('download:progress', (e) => cb(e.payload))
}

export interface OrchestratorThought {
  text: string
  color: string  // hex: "#a1a1aa" серый, "#4ade80" зелёный, "#f87171" красный, "#fbbf24" жёлтый
  priority: number  // 0=CHATTER, 1=EVENT, 2=CRITICAL
}

export function onOrchestratorThought(cb: (t: OrchestratorThought) => void) {
  return listen<OrchestratorThought>('orchestrator:thought', (e) => cb(e.payload))
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
