// Глобальные тосты (snackbar) — для неблокирующих уведомлений и undo.
// Единственный стор; компонент Toasts монтируется в App.svelte.

import { commands } from '$lib/bridge/commands'

export type ToastType = 'success' | 'error' | 'info' | 'warning'

export interface Toast {
  id: number
  type?: ToastType
  title?: string
  message: string
  actionLabel?: string
  onAction?: () => void
  duration?: number
}

export const toasts = $state<Toast[]>([])

let nextId = 1

export function pushToast(t: Omit<Toast, 'id'>) {
  const id = nextId++
  toasts.push({ id, ...t })

  const label = t.title ? `${t.title}: ${t.message}` : t.message
  const level = t.type === 'error' ? 'error' : t.type === 'warning' ? 'warn' : 'info'
  commands.logFrontend(label, level).catch(() => {})

  const dur = t.duration ?? 5000
  if (dur > 0) {
    setTimeout(() => dismissToast(id), dur)
  }
  return id
}

export function dismissToast(id: number) {
  const i = toasts.findIndex((x) => x.id === id)
  if (i >= 0) toasts.splice(i, 1)
}
