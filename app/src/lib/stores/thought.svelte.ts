// Единый источник правды «мыслей» оркестратора.
//
// Держит кольцевой буфер последних мыслей (читается страницей оркестратора)
// и рассылает каждую новую мысль подписчикам (TitleBar — для typewriter).
//
// Подписка на событие — ровно одна (guard-флаг), сколько бы раз ни вызвали
// registerThoughtCallback. Это исключает дублирующие listen() в компонентах.

import { onOrchestratorThought, type OrchestratorThought } from '$lib/bridge/events'

const MAX_THOUGHTS = 200

// Реактивный кольцевой буфер — читается страницей оркестратора.
export const thoughts = $state<OrchestratorThought[]>([])

type ThoughtCallback = (t: OrchestratorThought) => void
const callbacks = new Set<ThoughtCallback>()

let listenerStarted = false
let unlisten: (() => void) | null = null

// Старый контракт emitThought передавал «color» — маппим в новый «severity».
function colorToSeverity(color: string): string {
  switch (color) {
    case 'error':   return 'error'
    case 'warning': return 'warn'
    case 'success': return 'success'
    case 'info':    return 'info'
    case 'pink':    return 'info' // отдельного severity нет — трактуем как info
    case 'muted':
    default:        return 'muted'
  }
}

function pushThought(t: OrchestratorThought) {
  // Кольцевой буфер: держим только последние MAX_THOUGHTS записей.
  // Мутируем массив на месте — $state-массивы в Svelte 5 глубоко реактивны,
  // поэтому переприсваивать сам binding не нужно (и нельзя — он const).
  if (thoughts.length >= MAX_THOUGHTS) thoughts.shift()
  thoughts.push(t)
  // Рассылка подписчикам (typewriter в TitleBar и т.п.).
  callbacks.forEach(cb => cb(t))
}

// Единственная подписка на событие. Guard-флаг защищает от дублей
// при многократном registerThoughtCallback.
function ensureListener() {
  if (listenerStarted) return
  listenerStarted = true
  onOrchestratorThought((t) => pushThought(t)).then((u) => { unlisten = u })
}

// Обратная совместимость с TitleBar: он подписывается на каждую новую мысль
// для typewriter-анимации. При первой регистрации запускаем слушатель.
export function registerThoughtCallback(cb: ThoughtCallback) {
  callbacks.add(cb)
  ensureListener()
}

// Обратная совместимость: DownloadPage шлёт мысли через старый контракт
// (text, color, priority). Маппим в OrchestratorThought и пушим в буфер,
// чтобы и лента, и typewriter их получили.
export function emitThought(text: string, color: string, _priority: number) {
  const severity = colorToSeverity(color)
  const kind =
    severity === 'error'   ? 'error' :
    severity === 'success' ? 'completed' :
    'info'
  pushThought({
    kind,
    text,
    severity,
    title: null,
    progress: null,
    description: null,
    ts: Date.now(),
  })
}
