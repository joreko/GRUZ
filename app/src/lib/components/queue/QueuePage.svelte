<script lang="ts">
  import { queue, refresh } from '$lib/stores/queue.svelte'
  import { commands } from '$lib/bridge/commands'
  import { formatDuration, formatBytes } from '$lib/utils/format'
  import type { DownloadTask } from '$lib/bridge/types'

  const stateLabel: Record<string, string> = {
    queued: 'В очереди', fetching: 'Получение данных',
    waiting: 'Ожидание', downloading: 'Загрузка',
    paused: 'Пауза', completed: 'Готово',
    failed: 'Ошибка', cancelled: 'Отменено',
  }
  const stateColor: Record<string, string> = {
    downloading: 'var(--status-downloading)',
    completed: 'var(--status-success)',
    failed: 'var(--status-error)',
    cancelled: 'var(--text-muted)',
    paused: 'var(--status-warning)',
  }

  async function cancel(id: string) {
    await commands.cancelDownload(id)
    await refresh()
  }

  async function remove(id: string) {
    await commands.removeTask(id)
    await refresh()
  }
</script>

<div class="page">
  <h2>Очередь</h2>
  {#if queue.tasks.length === 0}
    <div class="empty-state">
      <p class="empty-title">Очередь пуста</p>
      <p class="empty-hint">Вставьте ссылку YouTube на странице загрузки</p>
    </div>
  {:else}
    <ul class="task-list">
      {#each queue.tasks as task (task.id)}
        <li class="task-item">
          {#if task.thumbnail}
            <img class="thumb" src={task.thumbnail} alt="" />
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
              {/if}
              {#if task.error}<span class="error">{task.error}</span>{/if}
            </div>
            {#if task.state === 'downloading'}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {task.progress}%"></div>
              </div>
            {/if}
          </div>
          <div class="task-actions">
            {#if task.state === 'downloading' || task.state === 'waiting'}
              <button onclick={() => cancel(task.id)}>Отмена</button>
            {:else if task.state === 'completed'}
              {#if task.file_path}
                <button onclick={() => commands.openFile(task.file_path!)}>↗</button>
                <button onclick={() => commands.openFolder(task.file_path!)}>📁</button>
              {/if}
              <button onclick={() => remove(task.id)}>✕</button>
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
  .task-actions { flex-shrink: 0; }
  .task-actions button {
    padding: var(--space-1) var(--space-3); background: var(--bg-elevated);
    border: 1px solid var(--border-default); border-radius: var(--radius-sm);
    color: var(--text-secondary); font-size: var(--text-xs); cursor: pointer;
    transition: background var(--transition-fast);
  }
  .task-actions button:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .error { color: var(--status-error); }
</style>
