<script lang="ts">
  import '$lib/design/theme.css'
  import { onMount } from 'svelte'
  import { initQueue, destroyQueue } from '$lib/stores/queue.svelte'
  import { loadSettings } from '$lib/stores/settings.svelte'
  import Sidebar from '$lib/components/shared/Sidebar.svelte'
  import TitleBar from '$lib/components/shared/TitleBar.svelte'
  import DownloadPage from '$lib/components/download/DownloadPage.svelte'
  import QueuePage from '$lib/components/queue/QueuePage.svelte'
  import HistoryPage from '$lib/components/history/HistoryPage.svelte'
  import GalleryPage from '$lib/components/gallery/GalleryPage.svelte'
  import SettingsPage from '$lib/components/settings/SettingsPage.svelte'
  import SaveSettingsPage from '$lib/components/save-settings/SaveSettingsPage.svelte'
  import UpdatesPage from '$lib/components/updates/UpdatesPage.svelte'
  import QueueColumn from '$lib/components/shared/QueueColumn.svelte'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { writeText } from '@tauri-apps/plugin-clipboard-manager'
  import { commands } from '$lib/bridge/commands'

  import type { Route } from '$lib/bridge/types'

  let route = $state<Route>('download')
  const routeTitle: Record<Route, string> = {
    download:       'Загрузка',
    gallery:        'Галерея',
    settings:       'Настройки',
    'save-settings': 'Сохранение',
    updates: 'Обновления',
  }

  let fabOpen = $state(false)
  let logState = $state<'idle' | 'loading' | 'done' | 'error'>('idle')
  let logUrl = $state('')

  async function sendLog() {
    if (logState === 'loading') return
    logState = 'loading'
    try {
      logUrl = await commands.uploadLog()
      await writeText(logUrl)
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

  onMount(() => {
    Promise.all([initQueue(), loadSettings()])
    return () => destroyQueue()
  })
</script>

<div class="app-shell">
  <button class="app-logo-btn" onclick={() => openUrl('https://t.me/+rVTNJ_uXV0s4NTky')}>
    <img src="/logo.svg" alt="" class="app-logo" draggable="false" />
  </button>
  <div class="top-row">
    <TitleBar bind:route />
  </div>
  <div class="app-body">
    <Sidebar bind:route />
    <main class="content">
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
      {/if}
    </main>
    <aside class="sidebar-right">
      <QueueColumn />
    </aside>
  </div>

  <!-- FAB поддержки -->
  <button class="fab" class:fab-open={fabOpen} onclick={() => fabOpen ? closeFab() : fabOpen = true} title="Поддержка">
    {#if fabOpen}
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 4L4 12M4 4l8 8"/></svg>
    {:else}
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2a5 5 0 0 1 4.33 7.5L14 13l-3.5-1.67A5 5 0 1 1 8 2z"/></svg>
    {/if}
  </button>

  {#if fabOpen}
    <div class="fab-menu">
      <button class="fab-item" onclick={() => { openUrl('https://t.me/GRUZ_official'); closeFab() }}>
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

<style>
  .app-shell {
    position: relative;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: linear-gradient(135deg, #1a1a1a 0%, #0f0f0f 100%);
    overflow: hidden;
    border-radius: 48px;
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
    height: 40px;
    overflow: hidden;
  }
  .app-body {
    flex: 1;
    display: flex;
    overflow: hidden;
    background: transparent;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    background: var(--bg-content);
    border-top-left-radius: 48px;
    border-top-right-radius: 48px;
    border: 1px solid rgba(0,0,0,1);
    border-bottom: none;
    border-right: none;
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
    bottom: 20px;
    right: 14px;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
    color: var(--text-muted);
    cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
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
    border-radius: 12px;
    padding: 6px;
    display: flex; flex-direction: column; gap: 2px;
    min-width: 210px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5);
    z-index: 21;
  }

  .fab-item {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 10px;
    border-radius: 8px;
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
</style>
