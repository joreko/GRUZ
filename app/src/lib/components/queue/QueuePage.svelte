<script lang="ts">
  import { queue, refresh } from '$lib/stores/queue.svelte'
  import { commands } from '$lib/bridge/commands'
  import { formatDuration, formatBytes } from '$lib/utils/format'
  import { tooltip } from '$lib/utils/tooltip'
  import type { DownloadTask } from '$lib/bridge/types'

  const stateLabel: Record<string, string> = {
    waiting: 'Ожидание', downloading: 'Загрузка', converting: 'Конвертация',
    paused: 'Пауза', completed: 'Готово',
    failed: 'Ошибка', cancelled: 'Отменено',
  }
  const stateColor: Record<string, string> = {
    downloading: 'var(--status-downloading)',
    converting: 'var(--status-downloading)',
    completed: 'var(--status-success)',
    failed: 'var(--status-error)',
    cancelled: 'var(--text-muted)',
    paused: 'var(--status-warning)',
  }

  let confirmDelete = $state<{id: string} | null>(null)

  async function cancel(id: string) {
    try {
      await commands.cancelDownload(id)
      await refresh()
    } catch (e) {
      queue.error = e instanceof Error ? e.message : String(e)
    }
  }

  function remove(id: string) {
    confirmDelete = { id }
  }

  async function doRemove(id: string) {
    confirmDelete = null
    try {
      await commands.removeTask(id)
      await refresh()
    } catch (e) {
      queue.error = e instanceof Error ? e.message : String(e)
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
      <p>Удалить задачу без возможности восстановления?</p>
      <div class="confirm-actions">
        <button class="btn-danger" onclick={() => confirmDelete && doRemove(confirmDelete.id)}>Удалить</button>
        <button class="btn-cancel" onclick={() => confirmDelete = null}>Отмена</button>
      </div>
    </div>
  </div>
{/if}

<div class="page">
  <h2>Очередь</h2>

  {#if queue.error}
    <div class="error-banner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{queue.error}</span>
      <button class="btn-retry" onclick={refresh}>Повторить</button>
    </div>
  {/if}

  {#if !queue.error && queue.tasks.length === 0 && !queue.loading}
    <div class="empty-state">
      <p class="empty-title">Очередь пуста</p>
      <p class="empty-hint">Вставьте ссылку YouTube на странице загрузки</p>
      <button class="btn-nav" onclick={navigateToDownload}>Перейти к загрузке</button>
    </div>
  {:else if queue.tasks.length > 0}
    <ul class="task-list">
      {#each queue.tasks as task (task.id)}
        <li class="task-item">
          {#if task.thumbnail}
            <img class="thumb" src={task.thumbnail} alt={task.title ?? 'Превью видео'} />
          {:else}
            <div class="thumb thumb-placeholder"></div>
          {/if}
          <div class="task-body">
            <p class="task-title">{task.title ?? task.url}</p>
            <div class="task-meta">
              <span style="color: {stateColor[task.state] ?? 'var(--text-muted)'}">{stateLabel[task.state] ?? task.state}</span>
              {#if task.state === 'downloading'}
                <span>{task.progress.toFixed(1)}%</span>
                {#if task.speed}<span>{task.speed}</span>{/if}
                {#if task.eta}<span>ETA {task.eta}</span>{/if}
              {:else if task.state === 'converting' && task.progress > 0}
                <span>{task.progress.toFixed(1)}%</span>
                {#if task.speed}<span>{task.speed}</span>{/if}
              {/if}
              {#if task.error}<span class="error">{task.error}</span>{/if}
            </div>
            {#if task.state === 'downloading' || task.state === 'converting'}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {task.state === 'converting' && task.progress > 0 ? task.progress : task.state === 'converting' ? 100 : task.progress}%" class:indeterminate={task.state === 'converting' && task.progress <= 0}></div>
              </div>
            {/if}
          </div>
          <div class="task-actions">
            {#if task.state === 'downloading' || task.state === 'waiting' || task.state === 'converting'}
              <button onclick={() => cancel(task.id)}>Отмена</button>
            {:else if task.state === 'completed'}
              {#if task.file_path}
                <button aria-label="Открыть файл" use:tooltip={'Открыть файл'} onclick={() => commands.openFile(task.file_path!)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                </button>
                <button aria-label="Открыть папку" use:tooltip={'Открыть папку'} onclick={() => commands.openFolder(task.file_path!)}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                </button>
              {/if}
              <button aria-label="Удалить" use:tooltip={'Удалить'} onclick={() => remove(task.id)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            {:else}
              <button onclick={() => remove(task.id)}>✕</button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .page { padding: var(--space-6); display: flex; flex-direction: column; gap: var(--space-4); }
  h2 { margin: 0; font-size: var(--text-xl); font-weight: 600; }
  .empty-state { display: flex; flex-direction: column; gap: var(--space-2); padding: var(--space-8) 0; }
  .empty-title { margin: 0; font-size: var(--text-md); font-weight: 500; color: var(--text-secondary); }
  .empty-hint { margin: 0; font-size: var(--text-sm); color: var(--text-muted); }
  .task-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--space-2); }
  .task-item {
    display: flex; align-items: center; gap: var(--space-3);
    background: var(--bg-surface); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md); padding: var(--space-3);
  }
  .thumb { width: 80px; height: 45px; object-fit: cover; border-radius: var(--radius-sm); flex-shrink: 0; }
  .thumb-placeholder { background: var(--bg-elevated); }
  .task-body { flex: 1; display: flex; flex-direction: column; gap: var(--space-1); overflow: hidden; min-width: 0; }
  .task-title { margin: 0; font-size: var(--text-sm); font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .task-meta { display: flex; gap: var(--space-3); font-size: var(--text-xs); color: var(--text-muted); }
  .progress-bar { height: 3px; background: var(--bg-elevated); border-radius: var(--radius-full); overflow: hidden; }
  .progress-fill { height: 100%; background: var(--status-downloading); border-radius: var(--radius-full); transition: width 0.3s; }
  .progress-fill.indeterminate { width: 100% !important; opacity: 0.5; animation: pulse 1.5s ease-in-out infinite; }
  @keyframes pulse { 0%,100% { opacity: 0.3 } 50% { opacity: 0.8 } }
  .task-actions { flex-shrink: 0; }
  .task-actions button {
    padding: var(--space-1) var(--space-3); background: var(--bg-elevated);
    border: 1px solid var(--border-default); border-radius: var(--radius-sm);
    color: var(--text-secondary); font-size: var(--text-xs); cursor: pointer;
    transition: background var(--transition-fast);
  }
  .task-actions button:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .error { color: var(--status-error); }

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

  /* Navigate button */
  .btn-nav {
    margin-top: var(--space-2); padding: var(--space-2) var(--space-4);
    background: var(--bg-elevated); border: 1px solid var(--border-default);
    border-radius: var(--radius-md); color: var(--text-secondary);
    font-size: var(--text-sm); font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .btn-nav:hover { background: var(--bg-overlay); color: var(--text-primary); }

  /* Confirmation dialog */
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
