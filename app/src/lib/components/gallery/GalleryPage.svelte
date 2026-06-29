<script lang="ts">
  import { onMount } from "svelte"
  import { queue, refresh } from "$lib/stores/queue.svelte"
  import { commands } from "$lib/bridge/commands"
  import { onQueueUpdated } from "$lib/bridge/events"
  import { tooltip } from "$lib/utils/tooltip"
  import { dl } from "$lib/stores/download.svelte"
  import { formatDuration, formatParams } from "$lib/utils/format"
  import type { HistoryItem, Route } from "$lib/bridge/types"

  let { route = $bindable<Route>('gallery') } = $props()

  let history = $state<HistoryItem[]>([])
  let loadingHistory = $state(false)
  let error = $state<string | null>(null)
  let confirmClear = $state(false)
  let confirmDelete = $state<{type: 'history'|'queue', id: string} | null>(null)
  let confirmTimer: ReturnType<typeof setTimeout> | null = null

  async function loadHistory() {
    loadingHistory = true
    error = null
    try { history = await commands.getHistory() }
    catch (e) { error = e instanceof Error ? e.message : String(e) }
    finally { loadingHistory = false }
  }
  let unlistenQueue: (() => void) | null = null
  let destroyed = false

  onMount(() => {
    loadHistory()
    onQueueUpdated(() => { loadHistory(); refresh() }).then(fn => { if (!destroyed) unlistenQueue = fn })
    return () => { destroyed = true; unlistenQueue?.(); clearTimeout(confirmTimer ?? undefined) }
  })

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true
      confirmTimer = setTimeout(() => { confirmClear = false }, 3000)
      return
    }
    clearTimeout(confirmTimer!)
    confirmTimer = null
    confirmClear = false
    try {
      await commands.clearHistory()
      await commands.clearQueue()
      history = []
      error = null
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  function deleteItem(id: string) {
    confirmDelete = { type: 'history', id }
  }

  async function confirmDeleteItem(id: string) {
    confirmDelete = null
    try {
      await commands.deleteHistoryItem(id)
      history = history.filter(i => i.id !== id)
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  async function cancel(id: string) {
    try {
      await commands.cancelDownload(id)
      await refresh()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  function remove(id: string) {
    confirmDelete = { type: 'queue', id }
  }

  async function confirmRemove(id: string) {
    confirmDelete = null
    try {
      await commands.removeTask(id)
      await refresh()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  // Повторить загрузку: убрать из очереди и открыть DownloadPage с тем же URL
  async function retry(id: string, url: string) {
    try {
      await commands.removeTask(id)
      await refresh()
      dl.url = url
      route = 'download'
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  const stateLabel: Record<string, string> = {
    waiting: "Ожидание", downloading: "Загрузка",
    converting: "Конвертация", paused: "Пауза", completed: "Готово",
    failed: "Ошибка", cancelled: "Отменено",
  }

  // Задачи в очереди (не завершённые и не отменённые)
  const activeTasks = $derived(
    queue.tasks.filter(t => t.state !== 'completed' && t.state !== 'cancelled')
  )

  const isEmpty = $derived(activeTasks.length === 0 && history.length === 0 && !loadingHistory)
</script>

{#if confirmDelete}
  <div class="confirm-overlay">
    <div class="confirm-dialog">
      <p>Удалить без возможности восстановления?</p>
      <div class="confirm-actions">
        <button class="btn-danger" onclick={() => confirmDelete && (confirmDelete.type === 'history' ? confirmDeleteItem(confirmDelete.id) : confirmRemove(confirmDelete.id))}>Удалить</button>
        <button class="btn-cancel" onclick={() => confirmDelete = null}>Отмена</button>
      </div>
    </div>
  </div>
{/if}

<div class="page">
  <div class="top-row">
    <div class="tabs">
      <button class="tab active">Видео</button>
    </div>
    <div class="top-actions">
      <button class="btn-clear-all" aria-label="История" onclick={() => route = 'history'} use:tooltip={'История'}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><path d="M3 3v5h5"/><path d="M3.05 13A9 9 0 1 0 6 5.3L3 8"/><polyline points="12 7 12 12 15 15"/></svg>
      </button>
      <button class="btn-clear-all" onclick={loadHistory} aria-label="Обновить" use:tooltip={'Обновить'}
        class:spin={loadingHistory}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="12" height="12"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>
      </button>
      {#if (history.length > 0 || activeTasks.length > 0) && !error}
        <button class="btn-clear-all" class:danger={confirmClear} onclick={clearAll}>
          {confirmClear ? "Точно?" : "Очистить всё"}
        </button>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="error-banner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{error}</span>
      <button class="btn-retry" onclick={loadHistory}>Повторить</button>
    </div>
  {:else if loadingHistory && history.length === 0 && activeTasks.length === 0}
    <div class="grid skeleton-grid">
      {#each Array(6) as _}
        <div class="card card-skeleton shimmer"></div>
      {/each}
    </div>
  {:else if isEmpty}
    <div class="empty-state">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="7 10 12 15 17 10"/>
        <line x1="12" y1="15" x2="12" y2="3"/>
      </svg>
      <p class="empty-title">Ничего нет</p>
      <p class="empty-hint">Скачайте видео — оно появится здесь</p>
    </div>
  {:else}
    <div class="grid">
      <!-- Активные задачи из очереди -->
      {#each activeTasks as task (task.id)}
        <div class="card" class:card-failed={task.state === 'failed'}>
          {#if task.thumbnail}
            <img class="card-img" src={task.thumbnail} alt={task.title ?? 'Превью видео'} />
          {:else}
            <div class="card-img card-img-empty"></div>
          {/if}

          <div class="card-overlay">
            <div class="card-meta">
              <span class="card-state" data-state={task.state}>{stateLabel[task.state] ?? task.state}</span>
              {#if task.state === "downloading"}
                <span class="card-pct">{task.progress.toFixed(0)}%</span>
              {:else if task.state === "converting" && task.progress > 0}
                <span class="card-pct">{task.progress.toFixed(0)}%</span>
              {/if}
            </div>
            {#if task.title}
              <p class="card-title">{task.title}</p>
            {/if}
            <!-- Текст ошибки под названием -->
            {#if task.state === 'failed' && task.error}
              <p class="card-error">{task.error}</p>
            {/if}
          </div>

          {#if task.state === "downloading" || task.state === "converting"}
            <div class="card-progress">
              <div
                class="card-progress-fill"
                class:card-progress-converting={task.state === "converting" && task.progress <= 0}
                style="width:{task.state === 'converting' && task.progress > 0 ? task.progress : task.state === 'downloading' ? task.progress : 100}%"
              ></div>
            </div>
          {/if}

          <div class="card-actions">
            {#if task.state === "downloading" || task.state === "converting" || task.state === "waiting"}
              <button onclick={() => cancel(task.id)}>Отмена</button>
              <button class="btn-remove" aria-label="Удалить" onclick={() => remove(task.id)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            {:else if task.state === "failed"}
              <button class="btn-retry" onclick={() => retry(task.id, task.url)}>Повторить</button>
              <button class="btn-remove" aria-label="Удалить" onclick={() => remove(task.id)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            {:else}
              <button class="btn-remove" aria-label="Удалить" onclick={() => remove(task.id)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            {/if}
          </div>
        </div>
      {/each}

      <!-- История завершённых загрузок -->
      {#each history as item (item.id)}
        <div class="card">
          {#if item.thumbnail}
            <img class="card-img" src={item.thumbnail} alt={item.title} />
          {:else}
            <div class="card-img card-img-empty"></div>
          {/if}
          <div class="card-overlay">
            {#if item.duration}
              <span class="card-duration">{formatDuration(item.duration)}</span>
            {/if}
            <p class="card-title">{item.title}</p>
            <p class="card-params">{formatParams(item)}</p>
          </div>
          <div class="card-actions">
            <button onclick={async () => { try { await commands.openFile(item.file_path) } catch { error = 'Файл не найден. Возможно, он был перемещён или удалён.' } }}>Открыть</button>
            <button onclick={async () => { try { await commands.openFolder(item.file_path) } catch { error = 'Папка не найдена.' } }}>Папка</button>
            <button class="btn-remove" aria-label="Удалить" onclick={() => deleteItem(item.id)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    padding: var(--space-8) var(--space-9);
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100%;
  }

  /* ── Верхняя строка ── */
  .top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  /* Вкладки — впалый active как format-toggle */
  .tabs {
    display: flex;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: var(--space-1);
    width: fit-content;
  }
  .tab {
    display: flex; align-items: center; gap: 6px;
    height: var(--space-9); padding: 0 var(--space-4);
    background: transparent; border: none; border-radius: var(--radius-md);
    color: var(--text-muted); font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .tab:hover { color: var(--text-secondary); }
  .tab.active {
    background: rgba(0,0,0,0.35); color: var(--text-primary); font-weight: 600;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
  }

  .top-actions { display: flex; align-items: center; gap: 6px; }
  .btn-clear-all {
    display: inline-flex; align-items: center;
    height: 28px; padding: 0 11px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: 7px; color: var(--text-secondary);
    font-size: 11px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .btn-clear-all:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .btn-clear-all.danger { color: var(--status-error); border-color: var(--status-error); }

  /* ── Сетка карточек ── */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }
  .skeleton-grid { pointer-events: none; }
  .shimmer { position: relative; overflow: hidden; background: var(--bg-overlay); }
  .shimmer::after {
    content: ''; position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.04) 50%, transparent 100%);
    animation: shimmer-sweep 1.6s ease-in-out infinite;
  }
  @keyframes shimmer-sweep { 0% { transform: translateX(-100%); } 100% { transform: translateX(100%); } }

  /* Карточка */
  .card {
    position: relative;
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
    transition: transform 0.18s, box-shadow 0.18s, border-color 0.18s;
  }
  .card-skeleton {
    aspect-ratio: 16/9;
    cursor: default;
  }
  .card-skeleton:hover { transform: none; box-shadow: 0 4px 20px rgba(0,0,0,0.2); border-color: var(--border-subtle); }
  .card:hover {
    transform: translateY(-3px);
    box-shadow: 0 12px 32px rgba(0,0,0,0.5);
    border-color: var(--border-default);
  }
  .card:hover .card-overlay { background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.2) 60%, transparent 100%); }
  /* Карточка с ошибкой — красная рамка */
  .card-failed { border-color: color-mix(in srgb, var(--accent) 35%, transparent); }

  .card-img {
    display: block; width: 100%; aspect-ratio: 16/9; object-fit: cover;
  }
  .card-img-empty {
    aspect-ratio: 16/9; background: var(--bg-overlay);
  }

  /* Оверлей поверх превью */
  .card-overlay {
    position: absolute; inset: 0;
    display: flex; flex-direction: column; justify-content: flex-end;
    padding: 10px 12px;
    background: linear-gradient(to top, rgba(0,0,0,0.88) 0%, rgba(0,0,0,0.45) 45%, rgba(0,0,0,0.1) 70%, transparent 100%);
    transition: background var(--transition-default);
    pointer-events: none;
  }
  .card-title {
    margin: 0;
    font-size: 12px; font-weight: 600; line-height: 1.35;
    color: var(--text-primary);
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    text-shadow: 0 1px 6px rgba(0,0,0,0.9), 0 0 2px rgba(0,0,0,0.6);
  }
  .card-params {
    margin: 3px 0 0;
    font-size: 10px; font-weight: 500; color: rgba(255,255,255,0.75);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    text-shadow: 0 1px 4px rgba(0,0,0,0.9);
  }
  .card-meta {
    display: flex; align-items: center; gap: 6px; margin-bottom: 4px;
  }
  .card-state {
    font-size: 10px; font-weight: 700; letter-spacing: 0.04em;
    color: var(--text-primary);
    text-shadow: 0 1px 4px rgba(0,0,0,0.9);
  }
  .card-state[data-state="downloading"] { color: var(--status-downloading); text-shadow: 0 1px 6px rgba(0,0,0,1); }
  .card-state[data-state="converting"]  { color: var(--thought-warning); text-shadow: 0 1px 6px rgba(0,0,0,1); }
  .card-state[data-state="failed"]      { color: var(--status-error); text-shadow: 0 1px 6px rgba(0,0,0,1); }
  .card-state[data-state="completed"]   { color: var(--status-success); text-shadow: 0 1px 6px rgba(0,0,0,1); }
  .card-pct { font-size: 10px; font-weight: 700; color: var(--text-primary); text-shadow: 0 1px 4px rgba(0,0,0,0.9); }
  .card-duration {
    font-size: 10px; font-weight: 600; color: rgba(255,255,255,0.9);
    background: rgba(0,0,0,0.65); border-radius: var(--radius-sm); padding: 1px 5px;
    margin-bottom: 4px; width: fit-content;
  }
  /* Текст ошибки под названием */
  .card-error {
    margin: 4px 0 0;
    font-size: 10px; color: color-mix(in srgb, var(--status-error) 95%, transparent); line-height: 1.4;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    pointer-events: none;
    text-shadow: 0 1px 6px rgba(0,0,0,1);
  }

  /* Прогресс-бар */
  .card-progress {
    position: absolute; bottom: 0; left: 0; right: 0; height: 3px;
    background: rgba(255,255,255,0.06);
    z-index: 2;
  }
  .card-progress-fill {
    height: 100%;
    background: var(--accent);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 60%, transparent);
    transition: width 0.4s ease;
  }
  .card-progress-converting {
    width: 100%;
    background: linear-gradient(90deg, var(--accent) 0%, var(--accent-warm) 50%, var(--accent) 100%);
    background-size: 200% 100%;
    box-shadow: 0 0 8px rgba(255,100,61,0.5);
    animation: shimmer 1.4s ease-in-out infinite;
  }
  .btn-clear-all.spin svg { animation: spin-icon 0.7s linear infinite; }
  @keyframes spin-icon { to { transform: rotate(360deg); } }

  @keyframes shimmer {
    0%   { background-position: 100% 0; }
    100% { background-position: -100% 0; }
  }

  /* Кнопки карточки — полупрозрачны всегда, полная видимость при hover/focus */
  .card-actions {
    position: absolute; top: 8px; right: 8px;
    display: flex; gap: 4px;
    opacity: 0.85; transition: opacity var(--transition-fast);
  }
  .card:hover .card-actions, .card:focus-within .card-actions { opacity: 1; }
  .card-actions button {
    height: 26px; padding: 0 8px;
    background: var(--bg-overlay); backdrop-filter: blur(8px);
    border: 1px solid rgba(255,255,255,0.1); border-radius: var(--radius-sm);
    color: rgba(255,255,255,0.85); font-size: 11px; font-weight: 500;
    cursor: pointer; transition: background var(--transition-fast), color var(--transition-fast);
    white-space: nowrap;
  }
  .card-actions button:hover { background: rgba(40,40,40,0.95); color: rgba(255,255,255,1); }
  .card-actions .btn-remove:hover { background: color-mix(in srgb, var(--status-error) 85%, transparent); border-color: var(--status-error); }
  /* Кнопка «Повторить» — акцентный стиль */
  .card-actions .btn-retry { border-color: rgba(230,57,70,0.4); color: color-mix(in srgb, var(--status-error) 90%, transparent); }
  .card-actions .btn-retry:hover {
    background: rgba(230,57,70,0.2);
    border-color: rgba(230,57,70,0.7);
    color: var(--status-error);
  }

  /* ── Empty state ── */
  .empty-state {
    flex: 1;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 12px; padding: 60px 0;
    color: var(--text-muted);
  }
  .empty-state svg { width: 36px; height: 36px; opacity: 0.35; }
  .empty-title { margin: 0; font-size: 15px; font-weight: 500; color: var(--text-secondary); }
  .empty-hint { margin: 0; font-size: 13px; color: var(--text-muted); }

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

  /* focus-visible для всех интерактивных элементов */
  .card:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .card-actions button:focus-visible {
    outline: 2px solid var(--accent); outline-offset: 2px;
    background: rgba(40,40,40,0.95); color: rgba(255,255,255,1);
  }
  .btn-clear-all:focus-visible, .tab:focus-visible {
    outline: 2px solid var(--accent); outline-offset: 2px;
  }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
