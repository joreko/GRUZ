// queue.svelte.ts — Svelte 5: экспортировать объект, не переприсваивать переменную
import { commands } from '$lib/bridge/commands'
import { onQueueUpdated, onDownloadProgress, onDownloadCompleted, onDownloadFailed } from '$lib/bridge/events'
import { pushToast } from '$lib/stores/toast.svelte'
import type { DownloadTask, DownloadProgress } from '$lib/bridge/types'

export const queue = $state({ tasks: [] as DownloadTask[], loading: false, error: null as string | null })

let unlisten: (() => void)[] = []

// Защита от дублирующихся тостов: одна задача — один тост завершения/ошибки.
const toastedIds = new Set<string>()

export async function refresh() {
  queue.loading = true
  queue.error = null
  try {
    queue.tasks = await commands.getQueue()
  } catch (e) {
    queue.error = e instanceof Error ? e.message : String(e)
    commands.logFrontend(queue.error, 'error').catch(() => {})
  } finally {
    queue.loading = false
  }
}

// Счётчик поколений — защита от double initQueue race condition.
// Каждый вызов инкрементирует счётчик; после каждого await проверяем
// что счётчик не изменился — если изменился, старый вызов устарел.
let generation = 0

export async function initQueue() {
  // Очистить старые подписки перед созданием новых
  unlisten.forEach(fn => fn())
  unlisten = []

  // Захватываем текущее поколение
  const gen = ++generation

  await refresh()
  // Если пришёл новый вызов пока мы ждали — выходим
  if (gen !== generation) return

  const u1 = await onQueueUpdated(refresh)
  if (gen !== generation) { u1(); return }

  const u2 = await onDownloadProgress((p: DownloadProgress) => {
    const task = queue.tasks.find(t => t.id === p.task_id)
    if (task) {
      if (p.state === 'converting') {
        if (task.state !== 'converting') {
          task.state = 'converting'
          task.progress = 0
        } else {
          task.progress = p.progress
        }
      } else {
        task.progress = p.progress
      }
      task.speed = p.speed
      task.eta = p.eta
      task.stream_type = p.stream_type
    }
  })
  if (gen !== generation) { u1(); u2(); return }

  const u3 = await onDownloadCompleted((p) => {
    if (toastedIds.has(p.task_id)) return
    toastedIds.add(p.task_id)
    pushToast({
      type: 'success',
      title: 'Загрузка завершена',
      message: p.title ?? 'Файл готов',
      duration: 5000,
    })
  })
  if (gen !== generation) { u1(); u2(); u3(); return }

  const u4 = await onDownloadFailed((p) => {
    if (toastedIds.has(p.task_id)) return
    toastedIds.add(p.task_id)
    pushToast({
      type: 'error',
      title: 'Загрузка не удалась',
      message: p.error ?? 'Проверьте ссылку и попробуйте снова',
      duration: 6000,
    })
  })
  if (gen !== generation) { u1(); u2(); u3(); u4(); return }

  unlisten = [u1, u2, u3, u4]
}

export function destroyQueue() {
  generation++ // инвалидируем любой pending initQueue
  unlisten.forEach(fn => fn())
  unlisten = []
}
