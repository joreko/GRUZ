<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { getVersion } from '@tauri-apps/api/app'
  import { check } from '@tauri-apps/plugin-updater'
  import { onMount, onDestroy } from 'svelte'
  import { onOrchestratorThought, type OrchestratorThought } from '$lib/bridge/events'
  import { registerThoughtCallback } from '$lib/stores/thought.svelte'
  import {
    fetchChangelog,
    countNewChanges,
    getBaselineCounter,
    setBaselineCounter,
    getLatestCounter,
    displayVersion,
  } from '$lib/utils/changelog'

  import type { Route } from '$lib/bridge/types'

  let { title = '', route = $bindable<Route>('download') }: { title?: string; route?: Route } = $props()

  const win = getCurrentWindow()
  let maximized = $state(false)
  let version = $state('')
  let updateAvailable = $state(false)
  let updateVersion = $state('')
  let newChangesCount = $state(0)  // (+N) — изменений с момента установки
  let downloading = $state(false)
  let downloadProgress = $state(0)
  let downloaded = $state(false)
  let progressDone = $state(false) // линия зелёная, fade out

  function handleUpdateClick() {
    if (downloaded) { /* TODO: relaunch */ return }
    if (downloading) return
    if (!updateAvailable) {
      route = 'updates'
      return
    }
    downloading = true
    downloadProgress = 0
    progressDone = false
    const interval = setInterval(() => {
      downloadProgress += 2
      if (downloadProgress >= 100) {
        clearInterval(interval)
        downloadProgress = 100
        progressDone = true                          // линия → зелёная
        setTimeout(() => { downloaded = true; downloading = false }, 500)
      }
    }, 30)
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

    // Тихая проверка обновлений + загрузка changelog в фоне
    const [update, changelogs] = await Promise.allSettled([
      check(),
      fetchChangelog(),
    ])

    if (update.status === 'fulfilled' && update.value?.available) {
      updateAvailable = true
      updateVersion = update.value.version
    }

    if (changelogs.status === 'fulfilled') {
      const logs = changelogs.value
      const latest = getLatestCounter(logs)
      let baseline = getBaselineCounter()
      if (baseline === null) {
        // Первый запуск — запоминаем текущий счётчик как baseline
        baseline = latest
        setBaselineCounter(baseline)
      }
      newChangesCount = countNewChanges(logs, baseline)
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
    class:has-update={updateAvailable}
    class:is-downloading={downloading}
    class:is-downloaded={downloaded}
    class:active={updateAvailable || downloading || downloaded}
    class:clickable={!updateAvailable && !downloading && !downloaded}
    onclick={handleUpdateClick}
  >
    {#if downloading}
      <div class="progress-bar" class:done={progressDone} style="width:{downloadProgress}%"></div>
    {:else}
      <span class="version-text">{downloaded ? 'перезапустить' : updateAvailable ? `v${displayVersion(updateVersion)} доступна` : newChangesCount > 0 ? `v${displayVersion(version)} (+${newChangesCount})` : `v${displayVersion(version)}`}</span>
    {/if}
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
  .version-btn:hover { height: 24px; background: rgba(255,255,255,0.10); }
  .version-btn:active { background: rgba(255,255,255,0.03); transition: none; }
  .version-btn.active { height: 24px; cursor: pointer; }
  .version-btn.clickable:hover { cursor: pointer; }
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
  .version-btn:hover .version-text,
  .version-btn.active .version-text { opacity: 1; }
  .version-btn.has-update .version-text { color: var(--thought-info); }
  .progress-bar.done { background: var(--thought-success); box-shadow: 0 0 8px var(--thought-success), 0 0 20px var(--thought-success); opacity: 0; }
  .version-btn.is-downloaded .version-text { color: var(--thought-success); animation: fadeIn 0.4s ease both; }
  @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
  .progress-bar {
    position: absolute;
    left: 0; top: 0; bottom: 0;
    background: var(--thought-info);
    opacity: 0.6;
    box-shadow: 0 0 8px var(--thought-info), 0 0 20px var(--thought-info);
    transition: width 0.05s linear, background 0.3s, opacity 0.4s;
    pointer-events: none;
  }

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
