<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { commands } from '$lib/bridge/commands'
  import { onQueueUpdated } from '$lib/bridge/events'
  import { formatBytes, formatDuration, formatDate } from '$lib/utils/format'
  import { tooltip } from '$lib/utils/tooltip'
  import type { HistoryItem } from '$lib/bridge/types'

  let items = $state<HistoryItem[]>([])
  let loading = $state(true)
  let error = $state<string | null>(null)
  let confirmClear = $state(false)
  let confirmDelete = $state<string | null>(null)
  let confirmTimer: ReturnType<typeof setTimeout> | null = null

  let unlistenQueue: (() => void) | null = null
  onMount(async () => {
    await loadHistory()
    unlistenQueue = await onQueueUpdated(loadHistory)
  })
  onDestroy(() => { unlistenQueue?.(); if (confirmTimer) clearTimeout(confirmTimer) })

  async function loadHistory() {
    // При фоновом обновлении (уже есть данные) не мерцаем loading
    if (items.length === 0) loading = true
    error = null
    try {
      items = await commands.getHistory()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      loading = false
    }
  }

  function doDelete(id: string) {
    confirmDelete = id
  }

  async function confirmDeleteItem(id: string) {
    confirmDelete = null
    try {
      await commands.deleteHistoryItem(id)
      items = items.filter(i => i.id !== id)
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true
      confirmTimer = setTimeout(() => { confirmClear = false }, 3000)
      return
    }
    clearTimeout(confirmTimer!)
    confirmClear = false
    try {
      await commands.clearHistory()
      items = []
      error = null
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  function navigateToDownload() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__navigate?.('download')
  }
</script>

{#if confirmDelete}
  <div class="confirm-overlay">
    <div class="confirm-dialog">
      <p>Удалить запись без возможности восстановления?</p>
      <div class="confirm-actions">
        <button class="btn-danger" onclick={() => confirmDelete && confirmDeleteItem(confirmDelete)}>Удалить</button>
        <button class="btn-cancel" onclick={() => confirmDelete = null}>Отмена</button>
      </div>
    </div>
  </div>
{/if}

<div class="page">
  <div class="header">
    <h2>История</h2>
    {#if items.length > 0 && !error}
      <button class="btn-ghost" class:danger={confirmClear} onclick={clearAll}>
        {confirmClear ? 'Точно очистить?' : 'Очистить всё'}
      </button>
    {/if}
  </div>

  {#if error}
    <div class="error-banner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{error}</span>
      <button class="btn-retry" onclick={loadHistory}>Повторить</button>
    </div>
  {:else if loading}
    <p class="muted">Загрузка...</p>
  {:else if items.length === 0}
    <div class="empty-state">
      <p class="empty-title">Ничего не скачано</p>
      <p class="empty-hint">Завершённые загрузки появятся здесь</p>
      <button class="btn-nav" onclick={navigateToDownload}>Перейти к загрузке</button>
    </div>
  {:else}
    <ul class="list">
      {#each items as item (item.id)}
        <li class="item">
          {#if item.thumbnail}
            <img class="thumb" src={item.thumbnail} alt={item.title} />
          {:else}
            <div class="thumb thumb-placeholder"></div>
          {/if}
          <div class="body">
            <p class="title">{item.title}</p>
            <div class="meta">
              {#if item.channel}<span>{item.channel}</span>{/if}
              {#if item.duration}<span>{formatDuration(item.duration)}</span>{/if}
              <span>{item.quality} · {item.container.toUpperCase()}</span>
              {#if item.file_size}<span>{formatBytes(item.file_size)}</span>{/if}
              <span>{formatDate(item.created_at)}</span>
            </div>
          </div>
          <div class="actions">
            <button onclick={() => commands.openFile(item.file_path)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
              Открыть
            </button>
            <button aria-label="Открыть папку" use:tooltip={'Открыть папку'} onclick={() => commands.openFolder(item.file_path)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            </button>
            <button aria-label="Удалить" use:tooltip={'Удалить'} onclick={() => doDelete(item.id)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page { padding: var(--space-6); display: flex; flex-direction: column; gap: var(--space-4); }
  .header { display: flex; align-items: center; justify-content: space-between; }
  h2 { margin: 0; font-size: var(--text-xl); font-weight: 600; }
  .btn-ghost { background: none; border: 1px solid var(--border-default); border-radius: var(--radius-sm); color: var(--text-secondary); font-size: var(--text-xs); padding: var(--space-1) var(--space-3); cursor: pointer; transition: color var(--transition-fast), border-color var(--transition-fast); }
  .btn-ghost:hover { color: var(--status-error); border-color: var(--status-error); }
  .btn-ghost.danger { color: var(--status-error); border-color: var(--status-error); }
  .muted { color: var(--text-muted); font-size: var(--text-sm); }
  .empty-state { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-8) 0; }
  .empty-title { margin: 0; font-size: var(--text-md); font-weight: 500; color: var(--text-secondary); }
  .empty-hint { margin: 0; font-size: var(--text-sm); color: var(--text-muted); }
  .list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .item {
    display: flex; align-items: center; gap: var(--space-3);
    background: var(--bg-surface); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md); padding: var(--space-3);
  }
  .thumb { width: 80px; height: 45px; object-fit: cover; border-radius: var(--radius-sm); flex-shrink: 0; }
  .thumb-placeholder { background: var(--bg-elevated); }
  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--space-1); }
  .title { margin: 0; font-size: var(--text-sm); font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { display: flex; gap: var(--space-3); font-size: var(--text-xs); color: var(--text-muted); flex-wrap: wrap; }
  .actions { display: flex; gap: var(--space-1); flex-shrink: 0; }
  .actions button {
    padding: var(--space-1) var(--space-2); background: var(--bg-elevated);
    border: 1px solid var(--border-subtle); border-radius: var(--radius-sm);
    color: var(--text-secondary); font-size: var(--text-xs); cursor: pointer;
    transition: background var(--transition-fast);
  }
  .actions button:hover { background: var(--bg-overlay); color: var(--text-primary); }

  /* Error banner */
  .error-banner {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: color-mix(in srgb, var(--status-error) 10%, transparent); border: 1px solid color-mix(in srgb, var(--status-error) 30%, transparent);
    border-radius: var(--radius-md); color: var(--status-error);
    font-size: var(--text-sm);
  }
  .error-banner svg { width: 16px; height: 16px; flex-shrink: 0; }
  .btn-retry {
    margin-left: auto; padding: var(--space-1) var(--space-3);
    background: color-mix(in srgb, var(--status-error) 15%, transparent); border: 1px solid color-mix(in srgb, var(--status-error) 30%, transparent);
    border-radius: var(--radius-sm); color: var(--status-error);
    font-size: var(--text-xs); cursor: pointer;
    transition: background var(--transition-fast);
  }
  .btn-retry:hover { background: color-mix(in srgb, var(--status-error) 25%, transparent); }
  .btn-nav {
    margin-top: var(--space-2); padding: var(--space-2) var(--space-4);
    background: var(--bg-elevated); border: 1px solid var(--border-default);
    border-radius: var(--radius-md); color: var(--text-secondary);
    font-size: var(--text-sm); font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .btn-nav:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .confirm-overlay {
    position: fixed; inset: 0; z-index: 100;
    display: flex; align-items: center; justify-content: center;
    background: rgba(0,0,0,0.6); backdrop-filter: blur(4px);
  }
  .confirm-dialog {
    background: var(--bg-elevated); border: 1px solid var(--border-default);
    border-radius: var(--radius-lg); padding: var(--space-5) var(--space-6);
    max-width: 360px; width: 90%; box-shadow: 0 8px 32px rgba(0,0,0,0.5);
  }
  .confirm-dialog p { margin: 0 0 var(--space-4); font-size: var(--text-sm); color: var(--text-primary); line-height: 1.5; }
  .confirm-actions { display: flex; gap: var(--space-2); justify-content: flex-end; }
  .confirm-actions button {
    padding: var(--space-2) var(--space-4); border-radius: var(--radius-sm);
    font-size: var(--text-sm); font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast);
  }
  .btn-danger {
    background: var(--status-error); border: none; color: var(--text-primary);
  }
  .btn-danger:hover { filter: brightness(1.1); }
  .btn-cancel {
    background: var(--bg-overlay); border: 1px solid var(--border-default); color: var(--text-secondary);
  }
  .btn-cancel:hover { background: var(--border-subtle); color: var(--text-primary); }
</style>
