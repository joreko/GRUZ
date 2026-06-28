// queue.svelte.ts — Svelte 5: экспортировать объект, не переприсваивать переменную
import { commands } from '$lib/bridge/commands'
import { onQueueUpdated, onDownloadProgress } from '$lib/bridge/events'
import type { DownloadTask, DownloadProgress } from '$lib/bridge/types'

export const queue = $state({ tasks: [] as DownloadTask[], loading: false, error: null as string | null })

let unlisten: (() => void)[] = []

export async function refresh() {
  queue.loading = true
  queue.error = null
  try {
    queue.tasks = await commands.getQueue()
  } catch (e) {
    queue.error = e instanceof Error ? e.message : String(e)
  } finally {
    queue.loading = false
  }
}

let destroyed = false

export async function initQueue() {
  // Очистить старые подписки перед созданием новых (избежать race condition)
  unlisten.forEach(fn => fn())
  unlisten = []
  destroyed = false
  await refresh()
  if (destroyed) return
  const u1 = await onQueueUpdated(refresh)
  const u2 = await onDownloadProgress((p: DownloadProgress) => {
    const task = queue.tasks.find(t => t.id === p.task_id)
    if (task) {
      task.progress = p.progress
      task.speed = p.speed
      task.eta = p.eta
      if (p.state === 'finished') task.state = 'converting'
    }
  })
  if (destroyed) { u1(); u2(); return }
  unlisten = [u1, u2]
}

export function destroyQueue() {
  destroyed = true
  unlisten.forEach(fn => fn())
  unlisten = []
}
