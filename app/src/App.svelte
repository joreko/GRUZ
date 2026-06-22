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

  import type { Route } from '$lib/bridge/types'

  let route = $state<Route>('download')
  const routeTitle: Record<Route, string> = {
    download:       'Р—Р°РіСЂСѓР·РєР°',
    gallery:        'Р“Р°Р»РµСЂРµСЏ',
    settings:       'РќР°СЃС‚СЂРѕР№РєРё',
    'save-settings': 'РЎРѕС…СЂР°РЅРµРЅРёРµ',
    updates: 'РћР±РЅРѕРІР»РµРЅРёСЏ',
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
</style>
