<script lang="ts">
  import { onMount } from "svelte"
  import { queue, refresh } from "$lib/stores/queue.svelte"
  import { commands } from "$lib/bridge/commands"
  import { onQueueUpdated } from "$lib/bridge/events"
  import { dl } from "$lib/stores/download.svelte"
  import { formatDuration } from "$lib/utils/format"
  import type { HistoryItem } from "$lib/bridge/types"

  // Роут для навигации к странице загрузки (кнопка «Повторить»)
  type Route = 'download' | 'gallery' | 'settings' | 'save-settings'
  let { route = $bindable<Route>('gallery') } = $props()

  let history = $state<HistoryItem[]>([])
  let loadingHistory = $state(false)
  let confirmClear = $state(false)
  let confirmTimer: ReturnType<typeof setTimeout> | null = null

  async function loadHistory() {
    loadingHistory = true
    try { history = await commands.getHistory() }
    finally { loadingHistory = false }
  }

  let unlistenQueue: (() => void) | null = null

  onMount(() => {
    loadHistory()
    onQueueUpdated(() => loadHistory()).then(fn => { unlistenQueue = fn })
    return () => { unlistenQueue?.(); clearTimeout(confirmTimer ?? undefined) }
  })

  async function clearAll() {
    if (!confirmClear) {
      confirmClear = true
      confirmTimer = setTimeout(() => { confirmClear = false }, 3000)
      return
    }
    clearTimeout(confirmTimer!)
    confirmClear = false
    await commands.clearHistory()
    history = []
  }

  async function deleteItem(id: string) {
    await commands.deleteHistoryItem(id)
    history = history.filter(i => i.id !== id)
  }

  async function cancel(id: string) {
    await commands.cancelDownload(id)
    await refresh()
  }

  async function remove(id: string) {
    await commands.removeTask(id)
    await refresh()
  }

  // Повторить загрузку: убрать из очереди и открыть DownloadPage с тем же URL
  async function retry(id: string, url: string) {
    await commands.removeTask(id)
    await refresh()
    dl.url = url
    route = 'download'
  }

  const stateLabel: Record<string, string> = {
    queued: "В очереди", fetching: "Анализ", waiting: "Ожидание",
    downloading: "Загрузка", converting: "Конвертация", paused: "Пауза", completed: "Готово",
    failed: "Ошибка", cancelled: "Отменено",
  }

  // Задачи в очереди (не завершённые и не отменённые)
  const activeTasks = $derived(
    queue.tasks.filter(t => t.state !== 'completed' && t.state !== 'cancelled')
  )

  const isEmpty = $derived(activeTasks.length === 0 && history.length === 0 && !loadingHistory)
</script>

<div class="page">
  <div class="top-row">
    <div class="tabs">
      <button class="tab active">Видео</button>
      <button class="tab tab-soon" disabled>
        Плейлисты
        <span class="tab-soon-badge">скоро</span>
      </button>
    </div>
    {#if history.length > 0}
      <button class="btn-clear-all" class:danger={confirmClear} onclick={clearAll}>
        {confirmClear ? "Точно?" : "Очистить всё"}
      </button>
    {/if}
  </div>

  {#if isEmpty}
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
            <img class="card-img" src={task.thumbnail} alt="" />
          {:else}
            <div class="card-img card-img-empty"></div>
          {/if}

          <div class="card-overlay">
            <div class="card-meta">
              <span class="card-state" data-state={task.state}>{stateLabel[task.state] ?? task.state}</span>
              {#if task.state === "downloading"}
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
                class:card-progress-converting={task.state === "converting"}
                style={task.state === "downloading" ? "width:" + task.progress + "%" : ""}
              ></div>
            </div>
          {/if}

          <div class="card-actions">
            {#if task.state === "downloading" || task.state === "converting" || task.state === "waiting" || task.state === "queued" || task.state === "fetching"}
              <button onclick={() => cancel(task.id)}>Отмена</button>
            {:else if task.state === "failed"}
              <button class="btn-retry" onclick={() => retry(task.id, task.url)}>Повторить</button>
              <button class="btn-remove" onclick={() => remove(task.id)}>✕</button>
            {:else}
              <button class="btn-remove" onclick={() => remove(task.id)}>✕</button>
            {/if}
          </div>
        </div>
      {/each}

      <!-- История завершённых загрузок -->
      {#each history as item (item.id)}
        <div class="card">
          {#if item.thumbnail}
            <img class="card-img" src={item.thumbnail} alt="" />
          {:else}
            <div class="card-img card-img-empty"></div>
          {/if}
          <div class="card-overlay">
            {#if item.duration}
              <span class="card-duration">{formatDuration(item.duration)}</span>
            {/if}
            <p class="card-title">{item.title}</p>
          </div>
          <div class="card-actions">
            <button onclick={() => commands.openFile(item.file_path)}>Открыть</button>
            <button onclick={() => commands.openFolder(item.file_path)}>Папка</button>
            <button class="btn-remove" onclick={() => deleteItem(item.id)}>✕</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .page {
    padding: 32px 36px;
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
    border-radius: 11px;
    padding: 3px;
    width: fit-content;
  }
  .tab {
    display: flex; align-items: center; gap: 6px;
    height: 36px; padding: 0 16px;
    background: transparent; border: none; border-radius: 8px;
    color: var(--text-muted); font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .tab:hover { color: var(--text-secondary); }
  .tab.active {
    background: rgba(0,0,0,0.35); color: var(--text-primary); font-weight: 600;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
  }
  .tab-soon { opacity: 0.4; cursor: default; }
  .tab-soon-badge {
    font-size: 8px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase;
    color: var(--text-muted); background: var(--bg-overlay); border-radius: 4px; padding: 1px 5px;
  }

  /* Кнопка «Очистить всё» */
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

  /* Карточка */
  .card {
    position: relative;
    border-radius: 14px;
    overflow: hidden;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
    transition: transform 0.18s, box-shadow 0.18s, border-color 0.18s;
  }
  .card:hover {
    transform: translateY(-3px);
    box-shadow: 0 12px 32px rgba(0,0,0,0.5);
    border-color: var(--border-default);
  }
  .card:hover .card-actions { opacity: 1; }
  .card:hover .card-overlay { background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.2) 60%, transparent 100%); }
  /* Карточка с ошибкой — красная рамка */
  .card-failed { border-color: rgba(239,68,68,0.35); }

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
    background: linear-gradient(to top, rgba(0,0,0,0.7) 0%, rgba(0,0,0,0.1) 50%, transparent 100%);
    transition: background 0.2s;
    pointer-events: none;
  }
  .card-title {
    margin: 0;
    font-size: 12px; font-weight: 600; line-height: 1.35;
    color: rgba(255,255,255,0.95);
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    text-shadow: 0 1px 4px rgba(0,0,0,0.8);
  }
  .card-meta {
    display: flex; align-items: center; gap: 6px; margin-bottom: 4px;
  }
  .card-state {
    font-size: 10px; font-weight: 600; letter-spacing: 0.04em;
    color: rgba(255,255,255,0.7);
  }
  .card-state[data-state="downloading"] { color: var(--status-downloading); }
  .card-state[data-state="converting"]  { color: var(--thought-warning); }
  .card-state[data-state="failed"]      { color: var(--status-error); }
  .card-state[data-state="completed"]   { color: var(--status-success); }
  .card-pct { font-size: 10px; font-weight: 700; color: rgba(255,255,255,0.9); }
  .card-duration {
    font-size: 10px; font-weight: 600; color: rgba(255,255,255,0.8);
    background: rgba(0,0,0,0.55); border-radius: 4px; padding: 1px 5px;
    margin-bottom: 4px; width: fit-content;
  }
  /* Текст ошибки под названием */
  .card-error {
    margin: 4px 0 0;
    font-size: 10px; color: rgba(239,100,100,0.85); line-height: 1.4;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
    pointer-events: none;
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
    box-shadow: 0 0 6px rgba(255,61,61,0.6);
    transition: width 0.4s ease;
  }
  .card-progress-converting {
    width: 100%;
    background: linear-gradient(90deg, var(--accent) 0%, var(--accent-warm) 50%, var(--accent) 100%);
    background-size: 200% 100%;
    box-shadow: 0 0 8px rgba(255,100,61,0.5);
    animation: shimmer 1.4s ease-in-out infinite;
  }
  @keyframes shimmer {
    0%   { background-position: 100% 0; }
    100% { background-position: -100% 0; }
  }

  /* Кнопки карточки — появляются при hover */
  .card-actions {
    position: absolute; top: 8px; right: 8px;
    display: flex; gap: 4px;
    opacity: 0; transition: opacity 0.15s;
  }
  .card-actions button {
    height: 26px; padding: 0 8px;
    background: rgba(15,15,15,0.85); backdrop-filter: blur(8px);
    border: 1px solid rgba(255,255,255,0.1); border-radius: 6px;
    color: rgba(255,255,255,0.85); font-size: 11px; font-weight: 500;
    cursor: pointer; transition: background 0.15s, color 0.15s;
    white-space: nowrap;
  }
  .card-actions button:hover { background: rgba(40,40,40,0.95); color: rgba(255,255,255,1); }
  .card-actions .btn-remove:hover { background: rgba(180,30,30,0.85); border-color: var(--status-error); }
  /* Кнопка «Повторить» — акцентный стиль */
  .card-actions .btn-retry { border-color: rgba(230,57,70,0.4); color: rgba(239,100,100,0.9); }
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
</style>
