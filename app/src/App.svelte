<script lang="ts">
  import '$lib/design/theme.css'
  import { onMount, setContext } from 'svelte'
  import { fly } from 'svelte/transition'
  import { initQueue, destroyQueue } from '$lib/stores/queue.svelte'
  import { loadSettings } from '$lib/stores/settings.svelte'
  import Sidebar from '$lib/components/shared/Sidebar.svelte'
  import TitleBar from '$lib/components/shared/TitleBar.svelte'
  import DownloadPage from '$lib/components/download/DownloadPage.svelte'
  import GalleryPage from '$lib/components/gallery/GalleryPage.svelte'
  import SettingsPage from '$lib/components/settings/SettingsPage.svelte'
  import SaveSettingsPage from '$lib/components/save-settings/SaveSettingsPage.svelte'
  import UpdatesPage from '$lib/components/updates/UpdatesPage.svelte'
  import EditorPage from '$lib/components/editor/EditorPage.svelte'
  import StoragePage from '$lib/components/storage/StoragePage.svelte'
  import SchedulerPage from '$lib/components/scheduler/SchedulerPage.svelte'
  import ChannelsPage from '$lib/components/channels/ChannelsPage.svelte'
  import OrchestratorPage from '$lib/components/orchestrator/OrchestratorPage.svelte'
  import GraphPage from '$lib/components/graph/GraphPage.svelte'
  import DebugPage from '$lib/components/debug/DebugPage.svelte'
  import QueueColumn from '$lib/components/shared/QueueColumn.svelte'
  import Toasts from '$lib/components/shared/Toasts.svelte'
  import ErrorModal from '$lib/components/shared/ErrorModal.svelte'
  import ConfirmModal from '$lib/components/shared/ConfirmModal.svelte'
  import DeleteModal from '$lib/components/shared/DeleteModal.svelte'
  import { showDeleteDialog } from '$lib/stores/delete.svelte'
  import ContextMenu from '$lib/components/shared/ContextMenu.svelte'
  import { reportCrash } from '$lib/stores/crash.svelte'
  import { showConfirm } from '$lib/stores/confirm.svelte'
  import type { ContextMenuGroup } from '$lib/components/shared/ContextMenu.svelte'
  import { commands } from '$lib/bridge/commands'

  import { tip } from '$lib/stores/tooltip.svelte'
  import { tooltip } from '$lib/utils/tooltip'
  import { dl } from '$lib/stores/download.svelte'
  import type { Route } from '$lib/bridge/types'

  // Глобальный перехват runtime-ошибок — складывает в crash-стор,
  // который рендерит ErrorModal поверх всего приложения.
  // Само логирование в бэкенд делает reportCrash() (единый источник).
  function dumpError(type: string, err: unknown) {
    reportCrash(type, err)
  }
  window.onerror = (_msg, _source, _line, _col, err) => dumpError('ERR', err)
  window.addEventListener('unhandledrejection', (e) => dumpError('PROMISE', e.reason))

  // Подхватываем ошибки, случившиеся до монтирования Svelte (буферизованы в index.html),
  // и отдаём их в ту же модалку. Дальше инлайн-буфер отключаем.
  interface EarlyCrash { type: string; text: string }
  const earlyWin = window as unknown as { __pendingCrash?: EarlyCrash; __gruzMounted?: boolean }
  if (earlyWin.__pendingCrash) {
    reportCrash(earlyWin.__pendingCrash.type, earlyWin.__pendingCrash.text)
  }
  earlyWin.__gruzMounted = true

  let route = $state<Route>('download')
  function navigateTo(r: Route) { route = r }
  setContext('navigate', navigateTo)

  // История навигации для боковых кнопок мыши вперёд/назад.
  // hist + histPos — plain let (не $state), эффект зависит только от route.
  // histPos не реактивен — все потребители (navigateBack/Forward) читают его
  // напрямую, а route меняется через $state, что триггерит эффект.
  let hist: Route[] = ['download']
  let histPos = 0
  $effect(() => {
    const r = route
    if (r === hist[histPos]) return
    if (histPos < hist.length - 1) hist = hist.slice(0, histPos + 1)
    hist.push(r)
    histPos = hist.length - 1
  })
  function navigateBack() {
    if (histPos > 0) { histPos--; route = hist[histPos] }
  }
  function navigateForward() {
    if (histPos < hist.length - 1) { histPos++; route = hist[histPos] }
  }
  let fabOpen = $state(false)
  let logState = $state<'idle' | 'loading' | 'done' | 'error'>('idle')
  let logUrl = $state('')

  async function sendLog() {
    if (logState === 'loading') return
    logState = 'loading'
    try {
      logUrl = await commands.uploadLog()
      await commands.writeText(logUrl)
      logState = 'done'
    } catch {
      logState = 'error'
    }
  }

  function closeFab() {
    fabOpen = false
    logState = 'idle'
    logUrl = ''
  }

  let prefersReducedMotion = $state(false)

  // Контекстное меню
  let ctxMenu = $state<{ x: number; y: number; groups: ContextMenuGroup[] } | null>(null)

  function closeCtxMenu() { ctxMenu = null }

  function onMouseNav(e: MouseEvent) {
    if (e.button === 3) { navigateBack(); e.preventDefault() }
    else if (e.button === 4) { navigateForward(); e.preventDefault() }
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault()
    const groups: ContextMenuGroup[] = []
    const target = e.target as HTMLElement

    // --- Текстовый контекст (input/textarea) ---
    const inputEl = target.closest('input, textarea') as HTMLInputElement | HTMLTextAreaElement | null
    const isEditable = inputEl || target.closest('[contenteditable]')
    const savedSelection = window.getSelection()?.toString().trim() || ''

    const textItems: typeof groups[0]['items'] = []
    if (savedSelection) {
      textItems.push({ label: 'Копировать', icon: 'copy', action: () => commands.writeText(savedSelection) })
    }
    if (isEditable) {
      textItems.push({ label: 'Вставить', icon: 'paste', action: () => {
        commands.readText().then(text => {
          if (inputEl) {
            const start = inputEl.selectionStart ?? inputEl.value.length
            const end   = inputEl.selectionEnd   ?? inputEl.value.length
            inputEl.value = inputEl.value.slice(0, start) + text + inputEl.value.slice(end)
            inputEl.selectionStart = inputEl.selectionEnd = start + text.length
            inputEl.dispatchEvent(new Event('input', { bubbles: true }))
          }
        }).catch(() => {})
      }})
      textItems.push({ label: 'Выделить всё', icon: 'select-all', action: () => { if (inputEl) inputEl.select() }})
    }
    if (textItems.length) groups.push({ items: textItems })

    // --- Контекст: видео-инфо на странице загрузки ---
    const infoCard = target.closest('[data-item-type="video-info"]') as HTMLElement | null
    if (infoCard) {
      const url         = infoCard.dataset.url
      const title       = infoCard.dataset.title
      const uploaderUrl = infoCard.dataset.uploaderUrl
      const items: typeof groups[0]['items'] = []
      if (url)   items.push({ label: 'Открыть на сайте',      icon: 'link',  action: () => commands.openUrl(url) })
      if (url)   items.push({ label: 'Копировать ссылку',     icon: 'copy',  action: () => commands.writeText(url) })
      if (title) items.push({ label: 'Копировать название',   icon: 'copy',  action: () => commands.writeText(title) })
      if (uploaderUrl) items.push({ label: 'Открыть канал',   icon: 'link',  action: () => commands.openUrl(uploaderUrl) })
      if (items.length) groups.push({ items })
    }

    // --- Контекст: история (gallery history card / history page item) ---
    const historyEl = target.closest('[data-item-type="history"]') as HTMLElement | null
    if (historyEl) {
      const filePath = historyEl.dataset.filePath
      const url      = historyEl.dataset.url
      const title    = historyEl.dataset.title
      const id       = historyEl.dataset.itemId
      const items: typeof groups[0]['items'] = []
      if (filePath) items.push({ label: 'Открыть файл',        icon: 'file',   action: () => commands.openFile(filePath).catch(() => {}) })
      if (filePath) items.push({ label: 'Открыть в папке',     icon: 'folder', action: () => commands.openFolder(filePath).catch(() => {}) })
      if (filePath) items.push({ label: 'Копировать путь',     icon: 'copy',   action: () => commands.writeText(filePath) })
      if (url)      items.push({ label: 'Копировать ссылку',   icon: 'copy',   action: () => commands.writeText(url) })
      if (url)      items.push({ label: 'Открыть на сайте',    icon: 'link',   action: () => commands.openUrl(url) })
      if (title)    items.push({ label: 'Копировать название', icon: 'copy',   action: () => commands.writeText(title) })
      if (url)      items.push({ label: 'Скачать снова',       icon: 'retry',  action: () => { dl.url = url; navigateTo('download') } })
      if (id) items.push({ label: 'Удалить', icon: 'delete', danger: true,
        action: async () => {
          const mode = await showDeleteDialog('single', 1, !!filePath)
          if (mode === 'history_only') commands.deleteHistoryItem(id)
          else if (mode === 'with_file') commands.deleteHistoryItemWithFile(id)
        } })
      if (items.length) groups.push({ items })
    }

    // --- Контекст: задача (gallery active task / queue page item / queue column) ---
    const taskEl = target.closest('[data-item-type="task"]') as HTMLElement | null
    if (taskEl) {
      const id       = taskEl.dataset.itemId
      const url      = taskEl.dataset.url
      const filePath = taskEl.dataset.filePath
      const title    = taskEl.dataset.title
      const state    = taskEl.dataset.taskState
      const isActive = state === 'waiting' || state === 'downloading' || state === 'converting'
      const items: typeof groups[0]['items'] = []
      if (url)      items.push({ label: 'Копировать ссылку',   icon: 'copy',   action: () => commands.writeText(url) })
      if (url)      items.push({ label: 'Открыть на сайте',    icon: 'link',   action: () => commands.openUrl(url) })
      if (title)    items.push({ label: 'Копировать название', icon: 'copy',   action: () => commands.writeText(title) })
      if (filePath && state === 'completed') items.push({ label: 'Открыть файл',    icon: 'file',   action: () => commands.openFile(filePath).catch(() => {}) })
      if (filePath && state === 'completed') items.push({ label: 'Открыть в папке', icon: 'folder', action: () => commands.openFolder(filePath).catch(() => {}) })
      if (id && isActive)
        items.push({ label: 'Отменить загрузку', icon: 'delete', danger: true, action: () => commands.cancelDownload(id) })
      if (url && state === 'failed')
        items.push({ label: 'Повторить', icon: 'retry', action: () => { dl.url = url; navigateTo('download') } })
      if (id && !isActive)
        items.push({ label: 'Удалить задачу', icon: 'delete', danger: true,
          action: async () => { if (await showConfirm('Удалить задачу?')) commands.removeTask(id) } })
      if (items.length) groups.push({ items })
    }

    // --- Контекст: путь к папке (SaveSettingsPage dir-path) ---
    const pathEl = target.closest('[data-copy-path]') as HTMLElement | null
    if (pathEl?.dataset.copyPath) {
      const path = pathEl.dataset.copyPath
      groups.push({ items: [
        { label: 'Копировать путь',  icon: 'copy',   action: () => commands.writeText(path) },
        { label: 'Открыть в папке',  icon: 'folder', action: () => commands.openFolder(path).catch(() => {}) },
      ]})
    }

    // --- Контекст: релиз на странице обновлений ---
    const releaseEl = target.closest('[data-release-tag]') as HTMLElement | null
    if (releaseEl) {
      const tag = releaseEl.dataset.releaseTag
      if (tag) groups.push({ items: [
        { label: 'Копировать версию',    icon: 'copy', action: () => commands.writeText(tag) },
        { label: 'Открыть на GitHub',    icon: 'link', action: () => commands.openUrl(`https://github.com/joreko/GRUZ/releases/tag/${tag}`) },
      ]})
    }

    // --- Контекст: строка changelog ---
    const lineEl = target.closest('[data-line-text]') as HTMLElement | null
    if (lineEl?.dataset.lineText) {
      groups.push({ items: [
        { label: 'Копировать', icon: 'copy', action: () => commands.writeText(lineEl.dataset.lineText!) },
      ]})
    }

    // --- Fallback: навигация ---
    if (groups.length === 0) {
      groups.push({ items: [
        { label: 'Загрузка',  icon: 'nav', action: () => navigateTo('download') },
        { label: 'Галерея',   icon: 'nav', action: () => navigateTo('gallery') },
        { label: 'Настройки', icon: 'nav', action: () => navigateTo('settings') },
      ]})
    }

    ctxMenu = { x: e.clientX, y: e.clientY, groups }
  }
  onMount(() => {
    prefersReducedMotion = matchMedia('(prefers-reduced-motion: reduce)').matches
    Promise.all([initQueue(), loadSettings()]).catch((e) => {
      reportCrash('INIT', e)
    })
    // Наш обработчик на capture — перехватывает раньше всех, preventDefault блокирует браузерное меню
    document.addEventListener('contextmenu', handleContextMenu as EventListener, true)
    // Боковые кнопки мыши (X1=назад, X2=вперёд) — как в браузере/проводнике
    document.addEventListener('mousedown', onMouseNav as EventListener, true)
    return () => {
      destroyQueue()
      document.removeEventListener('contextmenu', handleContextMenu as EventListener, true)
      document.removeEventListener('mousedown', onMouseNav as EventListener, true)
    }
  })
</script>

<div class="app-shell" role="application">
  <button class="app-logo-btn" aria-label="Открыть Telegram-канал" onclick={() => commands.openUrl('https://t.me/+rVTNJ_uXV0s4NTky')}>
    <img src="/logo.svg" alt="" class="app-logo" draggable="false" />
  </button>
  <div class="top-row">
    <TitleBar bind:route />
  </div>
  <div class="app-body">
    <Sidebar bind:route />
    <main class="content">
      {#key route}
        <div style="height:100%" in:fly={prefersReducedMotion ? {} : { x: 20, duration: 150, opacity: 0 }}>
          {#if route === 'download'}
            <DownloadPage bind:route />
          {:else if route === 'gallery'}
            <GalleryPage bind:route />
          {:else if route === 'settings'}
            <SettingsPage />
          {:else if route === 'save-settings'}
            <SaveSettingsPage />
          {:else if route === 'updates'}
            <UpdatesPage />
          {:else if route === 'editor'}
            <EditorPage />
          {:else if route === 'storage'}
            <StoragePage />
          {:else if route === 'scheduler'}
            <SchedulerPage />
          {:else if route === 'channels'}
            <ChannelsPage />
          {:else if route === 'orchestrator'}
            <OrchestratorPage />
          {:else if route === 'graph'}
            <GraphPage />
          {:else if route === 'debug'}
            <DebugPage />
          {/if}
        </div>
      {/key}
    </main>
    <aside class="sidebar-right">
      <QueueColumn />
    </aside>
  </div>

  <!-- FAB поддержки -->
  <button class="fab" class:fab-open={fabOpen} onclick={() => fabOpen ? closeFab() : fabOpen = true}
    use:tooltip={fabOpen ? '' : 'Поддержка'}>
    {#if fabOpen}
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 4L4 12M4 4l8 8"/></svg>
    {:else}
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2a5 5 0 0 1 4.33 7.5L14 13l-3.5-1.67A5 5 0 1 1 8 2z"/></svg>
    {/if}
  </button>

  {#if fabOpen}
    <div class="fab-menu">
      <button class="fab-item" onclick={() => { commands.openUrl('https://t.me/GRUZ_official'); closeFab() }}>
        <svg viewBox="0 0 16 16" fill="currentColor"><path d="M13.9 2.3L1.6 7c-.8.3-.8.8-.1 1l3.1 1 1.2 3.6c.2.5.3.7.7.7.3 0 .5-.1.7-.3l1.5-1.4 3.1 2.3c.6.3 1 .1 1.1-.5l2-9.4c.2-.8-.3-1.2-.9-.7z"/></svg>
        Написать разработчику
      </button>
      <button class="fab-item fab-item-log"
        class:loading={logState === 'loading'}
        class:done={logState === 'done'}
        class:error={logState === 'error'}
        onclick={sendLog}
        disabled={logState === 'loading'}
      >
        {#if logState === 'idle'}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M8 1v8M5 6l3 3 3-3M2 11v2a1 1 0 001 1h10a1 1 0 001-1v-2"/></svg>
          Отправить лог
        {:else if logState === 'loading'}
          <svg class="spin" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2"><circle cx="8" cy="8" r="5" stroke-dasharray="20 12"/></svg>
          Загружаю...
        {:else if logState === 'done'}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M3 8l3 3 7-7"/></svg>
          Ссылка скопирована
        {:else}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M8 5v4M8 11v1"/></svg>
          Ошибка — попробуй ещё
        {/if}
      </button>
      {#if logState === 'done' && logUrl}
        <p class="fab-url">{logUrl}</p>
      {/if}
    </div>
    <div class="fab-backdrop" onclick={closeFab} role="presentation"></div>
  {/if}
</div>

{#if ctxMenu}
  {#key ctxMenu}
    <ContextMenu x={ctxMenu.x} y={ctxMenu.y} groups={ctxMenu.groups} onclose={closeCtxMenu} />
  {/key}
{/if}

{#if tip.visible}
  <div class="tip-global"
    class:tip-right={tip.placement === 'right'}
    class:tip-bottom={tip.placement === 'bottom'}
    style="left:{tip.x}px;top:{tip.y}px"
  >{tip.text}</div>
{/if}

<Toasts />
<ErrorModal />
<ConfirmModal />
<DeleteModal />

<style>
  .app-shell {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: linear-gradient(135deg, var(--bg-elevated) 0%, var(--bg-base) 100%);
    overflow: hidden;
    border-radius: var(--radius-2xl);
    border: 1px solid rgba(0,0,0,1);
    box-shadow:
      inset 0 1px 0 rgba(120,120,120,0.5),
      inset 0 -1px 0 rgba(80,80,80,0.15),
      inset 1px 0 0 rgba(120,120,120,0.2),
      inset -1px 0 0 rgba(80,80,80,0.1);
  }
  .top-row {
    display: flex;
    flex-shrink: 0;
    height: var(--space-10);
    overflow: hidden;
  }
  .app-body {
    flex: 1;
    display: flex;
    overflow: hidden;
    background: transparent;
  }
  .content {
    position: relative;
    isolation: isolate;
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--bg-content);
    border-top-left-radius: var(--radius-2xl);
    border-top-right-radius: var(--radius-2xl);
  }
  /* Рамка страницы — рисуется ПОВЕРХ контента (в т.ч. оверлея),
     т.к. оверлей живёт ВНУТРИ страницы. Одна рамка на страницу. */
  .content::after {
    content: '';
    position: absolute;
    inset: 0;
    z-index: 250;
    pointer-events: none;
    border-radius: inherit;
    border: 1px solid rgba(0,0,0,1);
    box-shadow:
      inset 0 1px 0 rgba(120,120,120,0.5),
      inset 1px 0 0 rgba(120,120,120,0.15),
      inset -1px 0 0 rgba(120,120,120,0.15);
  }
  .sidebar-right {
    width: 64px;
    flex-shrink: 0;
    height: 100%;
  }
  .app-logo-btn {
    position: absolute;
    top: 10px;
    left: 18px;
    width: 56px;
    height: 56px;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    z-index: 10;
  }
  .app-logo {
    width: 56px;
    height: 56px;
    pointer-events: none;
  }

  /* FAB */
  .fab {
    position: absolute;
    bottom: var(--space-5);
    right: var(--space-4);
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
    color: var(--text-muted);
    cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    z-index: 20;
  }
  .fab svg { width: 15px; height: 15px; }
  .fab:hover { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }
  .fab.fab-open { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }

  .fab-backdrop {
    position: absolute; inset: 0; z-index: 15;
  }

  .fab-menu {
    position: absolute;
    bottom: 64px;
    right: 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-lg);
    padding: 6px;
    display: flex; flex-direction: column; gap: 2px;
    min-width: 210px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
    z-index: 21;
  }

  .fab-item {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px; font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, color 0.1s;
  }
  .fab-item svg { width: 14px; height: 14px; flex-shrink: 0; }
  .fab-item:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .fab-item.done { color: var(--thought-success); }
  .fab-item.error { color: var(--accent); }
  .fab-item.loading { color: var(--thought-info); cursor: default; }
  .fab-item:disabled { cursor: default; }

  .fab-url {
    margin: 2px 10px 6px;
    font-size: 10px; color: var(--text-muted);
    word-break: break-all; line-height: 1.4;
  }

  @keyframes spin { to { transform: rotate(360deg); } }
  .spin { animation: spin 0.9s linear infinite; }

  .tip-global {
    position: fixed;
    transform: translateX(-50%) translateY(calc(-100% - 8px));
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: 6px 10px;
    font-size: var(--text-xs); font-weight: 500;
    color: var(--text-primary); line-height: 1.5;
    white-space: nowrap;
    box-shadow: 0 4px 16px rgba(0,0,0,0.55);
    pointer-events: none;
    z-index: 99999;
    animation: tip-in 0.12s ease forwards;
  }
  .tip-global.tip-right {
    transform: translateX(10px) translateY(-50%);
    animation: tip-in-right 0.12s ease forwards;
  }
  .tip-global.tip-bottom {
    transform: translateX(-50%) translateY(8px);
    animation: tip-in-bottom 0.12s ease forwards;
  }
  @keyframes tip-in {
    from { opacity: 0; transform: translateX(-50%) translateY(calc(-100% - 4px)); }
    to   { opacity: 1; transform: translateX(-50%) translateY(calc(-100% - 8px)); }
  }
  @keyframes tip-in-right {
    from { opacity: 0; transform: translateX(6px) translateY(-50%); }
    to   { opacity: 1; transform: translateX(10px) translateY(-50%); }
  }
  @keyframes tip-in-bottom {
    from { opacity: 0; transform: translateX(-50%) translateY(4px); }
    to   { opacity: 1; transform: translateX(-50%) translateY(8px); }
  }

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
