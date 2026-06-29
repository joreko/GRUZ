<script lang="ts">
  import { queue } from '$lib/stores/queue.svelte'
  import { tooltip } from '$lib/utils/tooltip'
  import type { Route } from '$lib/bridge/types'
  let { route = $bindable<Route>('download') } = $props()

  const nav: { id: Route; label: string }[] = [
    { id: 'download',      label: 'Загрузка'    },
    { id: 'gallery',       label: 'Галерея'     },
    { id: 'queue-page',    label: 'Очередь'     },
    { id: 'scheduler',     label: 'Планировщик' },
    { id: 'channels',      label: 'Каналы'      },
    { id: 'editor',        label: 'Редактор'    },
    { id: 'storage',       label: 'Хранилище'   },
    { id: 'graph',         label: 'Граф'        },
    { id: 'settings',      label: 'Настройки'   },
  ]

  const activeDownloads = $derived(
    queue.tasks.filter(t => t.state === 'downloading' || t.state === 'waiting' || t.state === 'converting')
  )
</script>

<aside class="sidebar">
  <!-- Навигация -->
  <nav class="nav">
    {#each nav as item}
      <button
        class="nav-btn"
        class:active={route === item.id}
        use:tooltip={{ text: item.label, placement: 'right' }}
        onclick={() => route = item.id}
        aria-label={item.label}
        aria-current={route === item.id ? 'page' : undefined}
      >
        {#if item.id === 'download'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
        {:else if item.id === 'gallery'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
            <rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/>
          </svg>
          {#if activeDownloads.length > 0}
            <span class="badge">{activeDownloads.length}</span>
          {/if}
        {:else if item.id === 'editor'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"/>
          </svg>
        {:else if item.id === 'queue-page'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/>
            <line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>
          </svg>
        {:else if item.id === 'scheduler'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="4" width="18" height="18" rx="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/>
          </svg>
        {:else if item.id === 'channels'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="8" r="4"/><path d="M4 20c0-4 3.6-7 8-7s8 3 8 7"/>
          </svg>
        {:else if item.id === 'storage'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5v14c0 1.66 4.03 3 9 3s9-1.34 9-3V5"/><path d="M3 12c0 1.66 4.03 3 9 3s9-1.34 9-3"/>
          </svg>
        {:else if item.id === 'graph'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="5" cy="12" r="2"/><circle cx="19" cy="5" r="2"/><circle cx="19" cy="19" r="2"/><circle cx="12" cy="8" r="2"/>
            <line x1="7" y1="12" x2="17" y2="6"/><line x1="7" y1="12" x2="17" y2="18"/><line x1="14" y1="8" x2="17" y2="6"/>
          </svg>
        {:else if item.id === 'settings'}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
          </svg>
        {/if}
      </button>
    {/each}
  </nav>

  <!-- конец навигации -->
</aside>

<style>
  .sidebar {
    width: 64px;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: transparent;
    flex-shrink: 0;
    overflow: hidden;
  }

  /* Навигация */
  .nav {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 52px 0 10px;
    gap: 4px;
  }

  .nav-btn {
    position: relative;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--transition-default), color var(--transition-default), transform 0.1s;
    flex-shrink: 0;
  }
  .nav-btn svg {
    width: 18px;
    height: 18px;
    transition: transform var(--transition-default), color var(--transition-default);
  }
  .nav-btn:hover {
    background: rgba(0,0,0,0.3);
    color: var(--text-primary);
  }
  .nav-btn:hover svg { transform: scale(1.1); }
  .nav-btn:active { transform: scale(0.93); }
  .nav-btn.active {
    background: rgba(0,0,0,0.3);
    box-shadow: inset 0 1px 3px rgba(0,0,0,0.5), inset 0 -1px 1px rgba(255,255,255,0.03);
  }
  .nav-btn.active svg { color: var(--accent); }

  /* Badge */
  .badge {
    position: absolute;
    top: 4px;
    right: 4px;
    min-width: 14px;
    height: 14px;
    background: var(--accent);
    color: var(--text-primary);
    font-size: 9px;
    font-weight: 700;
    border-radius: 9999px;
    display: grid;
    place-items: center;
    padding: 0 3px;
    line-height: 1;
    pointer-events: none;
  }

  /* Скроллбар */</style>
