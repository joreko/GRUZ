// queue.svelte.ts — Svelte 5: экспортировать объект, не переприсваивать переменную
import { commands } from '$lib/bridge/commands'
import { onQueueUpdated, onDownloadProgress } from '$lib/bridge/events'
import type { DownloadTask, DownloadProgress } from '$lib/bridge/types'

export const queue = $state({ tasks: [] as DownloadTask[], loading: false })

let unlisten: (() => void)[] = []

export async function refresh() {
  queue.tasks = await commands.getQueue()
}

export async function initQueue() {
  await refresh()
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
  unlisten = [u1, u2]
}

export function destroyQueue() {
  unlisten.forEach(fn => fn())
  unlisten = []
}
