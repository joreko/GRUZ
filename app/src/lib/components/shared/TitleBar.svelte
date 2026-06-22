<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getVersion } from '@tauri-apps/api/app'

  import { onMount, onDestroy } from 'svelte'
  import { onOrchestratorThought, type OrchestratorThought } from '$lib/bridge/events'
  import { registerThoughtCallback } from '$lib/stores/thought.svelte'
  import {
    fetchChangelog,
    countNewChanges,
    getBaselineCounter,
    setBaselineCounter,
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

  // ── Система мыслей ───────────────────────────────────────────────────────

  // Семантические цвета → CSS-переменные дизайн-системы
  const colorMap: Record<string, string> = {
    muted:   'var(--thought-muted)',
    success: 'var(--thought-success)',
    error:   'var(--thought-error)',
    warning: 'var(--thought-warning)',
    info:    'var(--thought-info)',
    pink:    'var(--thought-pink)',
  }

  interface QueueItem { text: string; color: string; priority: number }

  let displayText = $state('')
  let displayColor = $state('var(--thought-muted)')
  let showCursor = $state(false)
  let isAnimating = false  // true пока идёт typewriter или dwell
  let queue: QueueItem[] = []
  let current: QueueItem | null = null
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
    const c = colorMap[item.color] ?? 'var(--thought-muted)'
    displayColor = c
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

  function enqueue(thought: OrchestratorThought) {
    if (current && current.text === thought.text) return

    if (thought.priority >= 2) {
      queue = queue.filter(e => e.priority >= 1)
      queue.unshift(thought)
      startNext(true)
    } else if (thought.priority === 1) {
      const lastEvent = queue.reduce((idx, e, i) => e.priority >= 1 ? i : idx, -1)
      queue.splice(lastEvent + 1, 0, thought)
      if (!current) startNext()
    } else {
      if (queue.filter(e => e.priority === 0).length < 3) queue.push(thought)
      if (!current) startNext()
    }
  }

  let unlisten: (() => void) | null = null
  onMount(async () => {
    unlistenResize = await win.onResized(async () => {
      maximized = await win.isMaximized()
    })
    version = await getVersion()

    // Загрузка changelog в фоне
    const changelogs = await fetchChangelog().catch(() => null)
    if (changelogs) {
      let baseline = getBaselineCounter()
      if (baseline === null) {
        // Первый запуск — baseline = счётчик текущей версии бинарника
        // version = "0.0.117" → patch = 117 = счётчик коммита
        const parts = version.split('.')
        baseline = parseInt(parts[parts.length - 1], 10) || 0
        setBaselineCounter(baseline)
      }
      newChangesCount = countNewChanges(changelogs, baseline)
    }

    registerThoughtCallback(enqueue)
    unlisten = await onOrchestratorThought(enqueue)
    cursorTimer = setInterval(() => {
      if (!isAnimating) showCursor = !showCursor
    }, 530)
    enqueue({ text: 'привет, я Груз.', color: 'success', priority: 0 })
  })
  onDestroy(() => {
    clearTimers()
    if (cursorTimer) clearInterval(cursorTimer)
    unlisten?.()
    unlistenResize?.()
  })
</script>

<header class="titlebar" data-tauri-drag-region>
  <button
    class="version-btn"
    class:has-changes={newChangesCount > 0}
    onclick={handleUpdateClick}
  >
    <span class="version-text">{newChangesCount > 0 ? `v${displayVersion(version)} (+${newChangesCount})` : `v${displayVersion(version)}`}</span>
  </button>
  <div class="middle-bar" data-tauri-drag-region>
    <span class="dash" style="color:{displayColor}">——</span>
    <span class="thought" style="color:{displayColor}">{displayText}{#if showCursor}<span class="cursor">▌</span>{/if}</span>
    <span class="dash" style="color:{displayColor}">——</span>
  </div>
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
    overflow: hidden;
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
    border-radius: 6px 6px 0 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    overflow: hidden;
  }

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
    transition: color 0.3s;
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
    border-radius: 48px 6px 0 0;
    background: rgba(255,255,255,0.055);
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.18),
      inset 1px 0 0 rgba(0,0,0,0.15),
      inset -1px 0 0 rgba(150,150,150,0.07);
    transition: height 0.2s cubic-bezier(0.34, 1.56, 0.64, 1), background 0.15s;
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
    transition: opacity 0.15s;
    pointer-events: none;
    user-select: none;
    white-space: nowrap;
  }
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
    transition: background 0.15s, box-shadow 0.15s;
    border-radius: 6px 6px 0 0;
  }

  .btn-close {
    width: 72px;
    border-radius: 6px 48px 0 0;
    background: rgba(255,61,61,0.6);
    box-shadow:
      inset 0 1px 0 rgba(255,140,140,0.35),
      inset 1px 0 0 rgba(255,80,80,0.18),
      inset -1px 0 0 rgba(0,0,0,0.15);
  }

  .btn-close:hover {
    background: rgba(255,61,61,0.82);
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


</style>
