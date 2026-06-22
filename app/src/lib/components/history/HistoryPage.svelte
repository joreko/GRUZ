<script lang="ts">
  import { onMount } from 'svelte'
  import { commands } from '$lib/bridge/commands'
  import { formatBytes, formatDuration, formatDate } from '$lib/utils/format'
  import type { HistoryItem } from '$lib/bridge/types'

  let items = $state<HistoryItem[]>([])
  let loading = $state(true)
  let confirmClear = $state(false)
  let confirmTimer: ReturnType<typeof setTimeout> | null = null

  onMount(async () => {
    try {
      items = await commands.getHistory()
    } finally {
      loading = false
    }
  })

  async function deleteItem(id: string) {
    await commands.deleteHistoryItem(id)
    items = items.filter(i => i.id !== id)
  }

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true
      confirmTimer = setTimeout(() => { confirmClear = false }, 3000)
      return
    }
    clearTimeout(confirmTimer!)
    confirmClear = false
    await commands.clearHistory()
    items = []
  }
</script>

<div class="page">
  <div class="header">
    <h2>История</h2>
    {#if items.length > 0}
      <button class="btn-ghost" class:danger={confirmClear} onclick={clearAll}>
        {confirmClear ? 'Точно очистить?' : 'Очистить всё'}
      </button>
    {/if}
  </div>

  {#if loading}
    <p class="muted">Загрузка...</p>
  {:else if items.length === 0}
    <div class="empty-state">
      <p class="empty-title">Ничего не скачано</p>
      <p class="empty-hint">Завершённые загрузки появятся здесь</p>
    </div>
  {:else}
    <ul class="list">
      {#each items as item (item.id)}
        <li class="item">
          {#if item.thumbnail}
            <img class="thumb" src={item.thumbnail} alt="" />
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
            <button onclick={() => commands.openFile(item.file_path)}>↗ Открыть</button>
            <button onclick={() => commands.openFolder(item.file_path)}>📁</button>
            <button onclick={() => deleteItem(item.id)}>✕</button>
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
</style>
