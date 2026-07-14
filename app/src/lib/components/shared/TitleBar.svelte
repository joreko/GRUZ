<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getVersion } from '@tauri-apps/api/app'
  import { tooltip } from '$lib/utils/tooltip'

  import { onMount, onDestroy } from 'svelte'
  import { type OrchestratorThought } from '$lib/bridge/events'
  import { registerThoughtCallback } from '$lib/stores/thought.svelte'
  import {
    fetchChangelog,
    countNewChanges,
    versionCounter,
    displayVersion,
  } from '$lib/utils/changelog'

  import type { Route } from '$lib/bridge/types'

  let { title = '', route = $bindable<Route>('download') }: { title?: string; route?: Route } = $props()

  const win = getCurrentWindow()
  let maximized = $state(false)
  let version = $state('')
  let newChangesCount = $state(0)  // (+N) — изменений с момента установки

  function handleUpdateClick() {
    route = 'updates'
  }

  let unlistenResize: (() => void) | null = null

  // severity (новый контракт) → CSS-переменная дизайн-системы.
  // Хардкод hex запрещён — только переменные из theme.css.
  const severityColor: Record<string, string> = {
    muted:   'var(--thought-muted)',
    success: 'var(--thought-success)',
    error:   'var(--thought-error)',
    warn:    'var(--thought-warning)',
    info:    'var(--thought-info)',
  }

  interface QueueItem {
    text: string
    severity: string
    kind: string
    title: string | null
    ts: number
  }

  let displayText = $state('')
  let displayColor = $state('var(--thought-muted)')
  let showCursor = $state(false)
  let isAnimating = false  // true пока идёт typewriter или dwell
  let queue: QueueItem[] = []
  let current = $state<QueueItem | null>(null)
  let typewriterTimer: ReturnType<typeof setTimeout> | null = null
  let dwellTimer: ReturnType<typeof setTimeout> | null = null
  let cursorTimer: ReturnType<typeof setInterval> | null = null
  let charIdx = 0

  function clearTimers() {
    if (typewriterTimer) { clearTimeout(typewriterTimer); typewriterTimer = null }
    if (dwellTimer) { clearTimeout(dwellTimer); dwellTimer = null }
  }

  function startNext(interrupt = false) {
    if (interrupt) clearTimers()
    const item = queue.shift()
    if (!item) {
      current = null
      isAnimating = false
      return
    }
    current = item
    displayColor = severityColor[item.severity] ?? 'var(--thought-muted)'
    displayText = ''
    showCursor = false
    isAnimating = true
    charIdx = 0
    typeNext()
  }

  function typeNext() {
    if (!current) return
    if (charIdx < current.text.length) {
      showCursor = true
      displayText = current.text.slice(0, ++charIdx)
      typewriterTimer = setTimeout(typeNext, 25)
    } else {
      isAnimating = false
      showCursor = true
      const dwell = queue.length > 0 ? 350 : 2500
      dwellTimer = setTimeout(() => startNext(), dwell)
    }
  }

  // Важность severity → порядок в очереди (выше = раньше показываем).
  function severityRank(severity: string): number {
    switch (severity) {
      case 'error': return 3
      case 'warn':  return 2
      default:      return 0 // info / success / muted / chatter
    }
  }

  function enqueue(thought: OrchestratorThought) {
    const item: QueueItem = {
      text: thought.text,
      severity: thought.severity,
      kind: thought.kind,
      title: thought.title,
      ts: thought.ts,
    }

    const rank = severityRank(thought.severity)
    if (rank >= 2) {
      // error / warn — показываем вперёд и прерываем текущую анимацию.
      queue.unshift(item)
      startNext(true)
    } else {
      // Остальные — по порядку, с ограничением длины очереди.
      if (queue.length < 5) queue.push(item)
      if (!current) startNext()
    }
  }

  onMount(async () => {
    unlistenResize = await win.onResized(async () => {
      maximized = await win.isMaximized()
    })
    version = await getVersion().catch(() => {
      console.error('getVersion failed, using 0.0.0')
      return '0.0.0'
    })

    // Загрузка changelog в фоне
    const changelogs = await fetchChangelog().catch(() => null)
    if (changelogs) {
      // (+N) считаем относительно установленной версии: patch = счётчик коммита.
      // Так старые релизы никогда не попадают в счётчик «новых изменений».
      const currentCounter = versionCounter(version)
      newChangesCount = countNewChanges(changelogs, currentCounter)
    }

    // Подписка на мысли — store сам слушает событие и рассылает их сюда.
    registerThoughtCallback(enqueue)
    const prefersReducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches
    if (!prefersReducedMotion) {
      cursorTimer = setInterval(() => {
        if (current && !isAnimating) showCursor = !showCursor
        else if (!current) showCursor = false
      }, 530)
    }
    enqueue({ kind: 'info', text: 'привет, я Груз.', severity: 'success', title: null, progress: null, description: null, ts: Date.now() })
  })
  onDestroy(() => {
    clearTimers()
    if (cursorTimer) clearInterval(cursorTimer)
    unlistenResize?.()
  })
</script>

<header class="titlebar" data-tauri-drag-region>
  <button
    class="version-btn"
    class:has-changes={newChangesCount > 0}
    onclick={handleUpdateClick}
    use:tooltip={{ text: 'Обновления', placement: 'bottom' }}
  >
    <span class="version-text">{newChangesCount > 0 ? `v${displayVersion(version)} (+${newChangesCount})` : `v${displayVersion(version)}`}</span>
  </button>
   <button class="middle-bar"
    onclick={() => route = 'orchestrator'}
    title={current?.title ?? ''}
    use:tooltip={{ text: 'Оркестратор', placement: 'bottom' }}
  >
    <span class="dash" style="color:{displayColor}">——</span>
    <span class="thought" style="color:{displayColor}">{displayText}{#if showCursor}<span class="cursor">▌</span>{/if}</span>
    <span class="dash" style="color:{displayColor}">——</span>
  </button>
  <div class="controls">
    <button class="btn btn-minimize" onclick={() => win.minimize()} aria-label="Свернуть"></button>
    <button class="btn btn-maximize" onclick={() => win.toggleMaximize()} aria-label="Развернуть"></button>
    <button class="btn btn-close" onclick={() => win.close()} aria-label="Закрыть"></button>
  </div>
</header>

<style>
  .titlebar {
    flex: 1;
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    background: transparent;
    flex-shrink: 0;
    user-select: none;
  }

  .middle-bar {
    flex: 1;
    height: 24px;
    margin: 0 3px;
    backdrop-filter: blur(8px);
    background: rgba(255,255,255,0.055);
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.18),
      inset 1px 0 0 rgba(150,150,150,0.07),
      inset -1px 0 0 rgba(0,0,0,0.15);
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    overflow: hidden;
    border: none;
    cursor: pointer;
    transition: background 150ms ease;
  }
  .middle-bar:hover { background: rgba(255,255,255,0.09); }

  .thought {
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.02em;
    white-space: nowrap;
    pointer-events: none;
  }

  .dash {
    font-size: 13px;
    font-weight: 500;
    pointer-events: none;
    flex-shrink: 0;
    transition: color var(--transition-slow);
    padding: 0 8px;
  }

  .cursor {
    opacity: 1;
  }

  .version-btn {
    margin-left: 118px;
    width: 190px;
    height: 12px;
    flex-shrink: 0;
    border-radius: 48px var(--radius-sm) 0 0;
    background: rgba(255,255,255,0.055);
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.18),
      inset 1px 0 0 rgba(0,0,0,0.15),
      inset -1px 0 0 rgba(150,150,150,0.07);
    transition: height var(--transition-default) cubic-bezier(0.34, 1.56, 0.64, 1), background var(--transition-fast);
    cursor: default;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    padding: 0;
    overflow: hidden;
    position: relative;
    -webkit-app-region: no-drag;
  }
  .version-btn:hover { height: 24px; background: rgba(255,255,255,0.10); cursor: pointer; }
  .version-btn:active { background: rgba(255,255,255,0.03); transition: none; }
  .version-text {
    font-size: 10px;
    font-weight: 600;
    color: rgba(255,255,255,0.35);
    letter-spacing: 0.06em;
    opacity: 0;
    transition: opacity var(--transition-fast);
    pointer-events: none;
    user-select: none;
    white-space: nowrap;
  }
  .version-btn.has-changes { height: 24px; }
  .version-btn.has-changes .version-text { color: var(--thought-info); }
  .version-btn:hover .version-text,
  .version-btn.has-changes .version-text { opacity: 1; }

  .controls {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    padding-right: 118px;
    -webkit-app-region: no-drag;
  }

  .btn {
    border: none;
    padding: 0;
    cursor: pointer;
    display: block;
    height: 24px;
    width: 56px;
    backdrop-filter: blur(8px);
    background: rgba(255,255,255,0.055);
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.18),
      inset 1px 0 0 rgba(150,150,150,0.07),
      inset -1px 0 0 rgba(0,0,0,0.15);
    transition: background var(--transition-fast), box-shadow var(--transition-fast);
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
  }

  .btn-close {
    width: 72px;
    border-radius: var(--radius-sm) 48px 0 0;
    background: color-mix(in srgb, var(--accent) 60%, transparent);
    box-shadow:
      inset 0 1px 0 rgba(255,140,140,0.35),
      inset 1px 0 0 rgba(255,80,80,0.18),
      inset -1px 0 0 rgba(0,0,0,0.15);
  }

  .btn-close:hover {
    background: color-mix(in srgb, var(--accent) 82%, transparent);
    box-shadow:
      inset 0 1px 0 rgba(255,150,150,0.35),
      inset 1px 0 0 rgba(255,80,80,0.18),
      inset -1px 0 0 rgba(0,0,0,0.1);
  }

  .btn:hover {
    background: rgba(255,255,255,0.10);
    box-shadow:
      inset 0 -1px 0 rgba(220,220,220,0.18),
      inset 1px 0 0 rgba(180,180,180,0.10),
      inset -1px 0 0 rgba(0,0,0,0.1);
  }
  .btn:active {
    background: rgba(255,255,255,0.03);
    transition: none;
  }


@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
