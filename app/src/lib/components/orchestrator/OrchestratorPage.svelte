<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { onDownloadProgress, onQueueUpdated } from '$lib/bridge/events'
  import { commands } from '$lib/bridge/commands'
  import { thoughts } from '$lib/stores/thought.svelte'
  import type { DownloadTask, DownloadProgress, TaskState } from '$lib/bridge/types'

  // ── Активные загрузки (воркеры) ──────────────────────────────────────────
  interface ActiveDownload {
    progress: number
    speed: string | null
    eta: string | null
    title: string | null
    stream: string | null
  }

  // Человекочитаемый лейбл фазы по stream_type из события прогресса:
  // "video" → видео, "audio" → аудио, "converting" → слияние.
  function streamLabel(stream: string | null): string | null {
    switch (stream) {
      case 'video':     return 'Видео'
      case 'audio':     return 'Аудио'
      case 'converting':return 'Слияние'
      default:          return null
    }
  }
  // Реактивный Map активных потоков. Переприсваиваем целиком,
  // чтобы Svelte 5 зафиксировал изменение (мутация Map не реактивна).
  let active = $state<Map<string, ActiveDownload>>(new Map())

  // ── Очередь ──────────────────────────────────────────────────────────────
  let tasks = $state<DownloadTask[]>([])

  // Агрегированная скорость всех активных потоков (MiB/s).
  const totalSpeed = $derived.by(() => {
    let sum = 0
    for (const a of active.values()) sum += speedToMiB(a.speed)
    return sum
  })
  const activeCount = $derived(active.size)
  const queuedCount = $derived(tasks.filter(t => t.state === 'waiting').length)

  // speed приходит строкой вида "1.23MiB/s" / "500KiB/s" — нормализуем в MiB/s.
  function speedToMiB(s: string | null): number {
    if (!s) return 0
    const m = s.match(/([\d.]+)\s*([KMG]?)i?B\/s/i)
    if (!m) return 0
    const v = parseFloat(m[1])
    const u = m[2].toUpperCase()
    if (u === 'K') return v / 1024
    if (u === 'M') return v
    if (u === 'G') return v * 1024
    return v / (1024 * 1024)
  }

  function titleFor(id: string): string | null {
    return tasks.find(t => t.id === id)?.title ?? null
  }

  function onProgress(p: DownloadProgress) {
    // Завершённые/упавшие потоки выпадают из активных.
    if (p.state === 'finished' || p.state === 'error') {
      if (active.has(p.task_id)) {
        active.delete(p.task_id)
        active = new Map(active)
      }
      return
    }
    const prev = active.get(p.task_id)
    active.set(p.task_id, {
      progress: p.progress,
      speed: p.speed,
      eta: p.eta,
      title: prev?.title ?? titleFor(p.task_id),
      stream: p.stream_type,
    })
    active = new Map(active)
  }

  async function refreshQueue() {
    try {
      tasks = await commands.getQueue()
    } catch {
      // Оркестратор не должен падать из-за недоступности очереди.
    }
  }

  let unlisten: (() => void)[] = []
  onMount(async () => {
    const u1 = await onDownloadProgress(onProgress)
    const u2 = await onQueueUpdated(refreshQueue)
    await refreshQueue()
    unlisten = [u1, u2]
  })
  onDestroy(() => {
    unlisten.forEach(fn => fn())
    unlisten = []
  })

  // ── Лента мыслей ──────────────────────────────────────────────────────────
  // severity → CSS-переменная дизайн-системы (хардкод hex запрещён).
  const severityColor: Record<string, string> = {
    muted:   'var(--thought-muted)',
    success: 'var(--thought-success)',
    error:   'var(--thought-error)',
    warn:    'var(--thought-warning)',
    info:    'var(--thought-info)',
  }

  function fmtTime(ts: number): string {
    const d = new Date(ts)
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  }

  let feedEl: HTMLDivElement | null = null
  let atBottom = $state(true)
  function onFeedScroll() {
    if (!feedEl) return
    atBottom = feedEl.scrollHeight - feedEl.scrollTop - feedEl.clientHeight < 48
  }
  // Авто-прокрутка к новым мыслям, пока пользователь не отскроллил вверх.
  $effect(() => {
    thoughts.length // зависимость от ленты
    if (feedEl && atBottom) feedEl.scrollTop = feedEl.scrollHeight
  })

  // ── Очередь: бейджи состояния ─────────────────────────────────────────────
  const stateLabel: Record<TaskState, string> = {
    waiting: 'ожидание',
    downloading: 'загрузка',
    converting: 'конвертация',
    completed: 'готово',
    failed: 'ошибка',
    cancelled: 'отменено',
  }
  const stateColor: Record<TaskState, string> = {
    waiting: 'var(--text-muted)',
    downloading: 'var(--status-downloading)',
    converting: 'var(--status-info)',
    completed: 'var(--status-success)',
    failed: 'var(--status-error)',
    cancelled: 'var(--text-muted)',
  }
</script>

<div class="page">
  <div class="orch">
    <!-- Шапка -->
    <header class="orch-header">
      <div class="orch-title">
        <span class="live-dot" aria-hidden="true"></span>
        <h1>Оркестратор</h1>
        <span class="live-text">в эфире</span>
      </div>
      <div class="orch-stats">
        <div class="stat">
          <span class="stat-val">{activeCount}</span>
          <span class="stat-label">потоков</span>
        </div>
        <div class="stat">
          <span class="stat-val">{queuedCount}</span>
          <span class="stat-label">в очереди</span>
        </div>
        <div class="stat speed">
          <span class="stat-val">∑ {totalSpeed.toFixed(1)}<span class="stat-unit"> MiB/s</span></span>
          <span class="stat-label">общая скорость</span>
        </div>
      </div>
    </header>

    <!-- Тело: лента мыслей + боковая панель -->
    <div class="orch-body">
      <!-- Лента мыслей -->
      <section class="panel feed-panel">
        <div class="panel-head">
          <span class="panel-title">Лента мыслей</span>
          <span class="panel-count">{thoughts.length}</span>
        </div>
        <div class="feed" bind:this={feedEl} onscroll={onFeedScroll}>
          {#if thoughts.length === 0}
            <div class="empty">
              <span class="empty-icon">·</span>
              <p>Оркестратор молчит. Мысли появятся здесь, как только что-то начнёт происходить.</p>
            </div>
          {:else}
            {#each thoughts as t (t.ts + ':' + t.kind + ':' + t.text + ':' + (t.title ?? ''))}
              <article class="thought-card" style="--sev:{severityColor[t.severity] ?? 'var(--thought-muted)'}">
                <span class="thought-dot" aria-hidden="true"></span>
                <div class="thought-body">
                  <div class="thought-meta">
                    <time class="thought-time">{fmtTime(t.ts)}</time>
                    {#if t.title}
                      <span class="thought-task">{t.title}</span>
                    {/if}
                  </div>
                  <p class="thought-text">{t.text}</p>
                  {#if t.description}
                    <p class="thought-desc">{t.description}</p>
                  {/if}
                </div>
              </article>
            {/each}
          {/if}
        </div>
      </section>

      <!-- Боковая колонка -->
      <div class="side">
        <!-- Активные загрузки -->
        <section class="panel workers-panel">
          <div class="panel-head">
            <span class="panel-title">Активные загрузки</span>
            <span class="panel-count">{activeCount}</span>
          </div>
          <div class="workers">
            {#if activeCount === 0}
              <div class="empty small">
                <p>Нет активных загрузок.</p>
              </div>
            {:else}
              {#each [...active.entries()] as [id, a] (id)}
                <div class="worker">
                  <div class="worker-head">
                    <span class="worker-title" title={a.title ?? id}>{a.title ?? id}</span>
                    <span class="worker-pct">{Math.round(a.progress)}%</span>
                  </div>
                  <div class="progress">
                    <div class="progress-fill" style="width:{a.progress}%"></div>
                  </div>
                  <div class="worker-meta">
                    {#if streamLabel(a.stream)}
                      <span class="worker-stream">{streamLabel(a.stream)}</span>
                    {/if}
                    <span>{a.speed ?? '—'}</span>
                    {#if a.eta}<span class="worker-eta">· {a.eta}</span>{/if}
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </section>

        <!-- Очередь -->
        <section class="panel queue-panel">
          <div class="panel-head">
            <span class="panel-title">Очередь</span>
            <span class="panel-count">{tasks.length}</span>
          </div>
          <div class="queue">
            {#if tasks.length === 0}
              <div class="empty small">
                <p>Очередь пуста.</p>
              </div>
            {:else}
              {#each tasks as t (t.id)}
                <div class="q-row">
                  <span class="q-dot" style="--sc:{stateColor[t.state]}"></span>
                  <span class="q-title" title={t.title ?? t.url}>{t.title ?? t.url}</span>
                  <span class="q-state" style="--sc:{stateColor[t.state]}">{stateLabel[t.state]}</span>
                  {#if t.state === 'downloading' || t.state === 'converting'}
                    <span class="q-progress">{Math.round(t.progress)}%</span>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        </section>
      </div>
    </div>
  </div>
</div>

<style>
  .page {
    height: 100%;
    overflow: hidden;
    box-sizing: border-box;
  }

  .orch {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: var(--space-6) var(--space-8);
    box-sizing: border-box;
    gap: var(--space-5);
  }

  /* ── Шапка ─────────────────────────────────────────────────── */
  .orch-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    flex-shrink: 0;
  }
  .orch-title {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .orch-title h1 {
    margin: 0;
    font-size: var(--text-xl);
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--text-primary);
  }
  .live-text {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .live-dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    background: var(--status-success);
    box-shadow: 0 0 0 0 color-mix(in srgb, var(--status-success) 70%, transparent);
  }
  @media (prefers-reduced-motion: no-preference) {
    .live-dot { animation: pulse 2s ease-out infinite; }
  }
  @keyframes pulse {
    0%   { box-shadow: 0 0 0 0 color-mix(in srgb, var(--status-success) 70%, transparent); }
    70%  { box-shadow: 0 0 0 7px color-mix(in srgb, var(--status-success) 0%, transparent); }
    100% { box-shadow: 0 0 0 0 color-mix(in srgb, var(--status-success) 0%, transparent); }
  }

  .orch-stats {
    display: flex;
    align-items: stretch;
    gap: var(--space-3);
  }
  .stat {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    padding: var(--space-2) var(--space-4);
    background: var(--bg-surface);
    border-radius: var(--radius-md);
    box-shadow:
      inset 0 1px 0 rgba(120,120,120,0.5),
      inset 0 -1px 0 rgba(80,80,80,0.15),
      inset 1px 0 0 rgba(120,120,120,0.2),
      inset -1px 0 0 rgba(80,80,80,0.1);
    border: 1px solid var(--border-subtle);
    min-width: 72px;
  }
  .stat-val {
    font-size: var(--text-lg);
    font-weight: 700;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }
  .stat-unit { font-size: var(--text-sm); font-weight: 600; color: var(--text-secondary); }
  .stat.speed .stat-val { color: var(--accent-warm); }
  .stat-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-top: 2px;
  }

  /* ── Тело ──────────────────────────────────────────────────── */
  .orch-body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: var(--space-5);
  }

  .panel {
    background: var(--bg-surface);
    border-radius: var(--radius-lg);
    box-shadow:
      inset 0 1px 0 rgba(120,120,120,0.5),
      inset 0 -1px 0 rgba(80,80,80,0.15),
      inset 1px 0 0 rgba(120,120,120,0.2),
      inset -1px 0 0 rgba(80,80,80,0.1);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .panel-title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }
  .panel-count {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    background: var(--bg-elevated);
    border-radius: var(--radius-full);
    padding: 1px 8px;
    min-width: 22px;
    text-align: center;
  }

  /* Лента мыслей */
  .feed-panel { flex: 1.5; }
  .feed {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    scrollbar-width: thin;
    scrollbar-color: var(--border-strong) transparent;
  }
  .feed::-webkit-scrollbar { width: 8px; }
  .feed::-webkit-scrollbar-thumb { background: var(--border-strong); border-radius: var(--radius-full); }
  .feed::-webkit-scrollbar-track { background: transparent; }

  .thought-card {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-left: 2px solid var(--sev);
  }
  @media (prefers-reduced-motion: no-preference) {
    .thought-card { animation: thought-in 200ms ease-out; }
  }
  @keyframes thought-in {
    from { opacity: 0; transform: translateY(6px); }
    to   { opacity: 1; transform: translateY(0); }
  }
  .thought-dot {
    width: 7px;
    height: 7px;
    border-radius: var(--radius-full);
    background: var(--sev);
    margin-top: 6px;
    flex-shrink: 0;
    box-shadow: 0 0 6px color-mix(in srgb, var(--sev) 60%, transparent);
  }
  .thought-body { min-width: 0; flex: 1; }
  .thought-meta {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    margin-bottom: 2px;
  }
  .thought-time {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .thought-task {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-tertiary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .thought-text {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-primary);
    word-break: break-word;
  }
  .thought-desc {
    margin: 2px 0 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-muted);
    word-break: break-word;
  }

  /* Боковая колонка */
  .side {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  .workers-panel { flex: 1.1; }
  .queue-panel { flex: 1; }

  .workers, .queue {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    scrollbar-width: thin;
    scrollbar-color: var(--border-strong) transparent;
  }
  .workers::-webkit-scrollbar, .queue::-webkit-scrollbar { width: 8px; }
  .workers::-webkit-scrollbar-thumb, .queue::-webkit-scrollbar-thumb { background: var(--border-strong); border-radius: var(--radius-full); }
  .workers::-webkit-scrollbar-track, .queue::-webkit-scrollbar-track { background: transparent; }

  .worker {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .worker-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .worker-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .worker-pct {
    font-size: 12px;
    font-weight: 700;
    color: var(--accent-warm);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .progress {
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--bg-base);
    overflow: hidden;
    box-shadow: inset 0 1px 1px rgba(0,0,0,0.4);
  }
  .progress-fill {
    height: 100%;
    border-radius: var(--radius-full);
    background: linear-gradient(90deg, var(--accent), var(--accent-warm));
    transition: width var(--transition-default) ease;
  }
  .worker-meta {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .worker-eta { color: var(--text-tertiary); }
  .worker-stream {
    font-family: var(--font-sans);
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-secondary);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    padding: 0 6px;
    line-height: 16px;
  }

  .q-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    transition: background var(--transition-fast) ease;
  }
  .q-row:hover { background: var(--bg-elevated); }
  .q-dot {
    width: 7px;
    height: 7px;
    border-radius: var(--radius-full);
    background: var(--sc);
    flex-shrink: 0;
  }
  .q-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .q-state {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--sc);
    flex-shrink: 0;
  }
  .q-progress {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    min-width: 34px;
    text-align: right;
  }

  /* Пустые состояния */
  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    text-align: center;
    padding: var(--space-6);
    color: var(--text-muted);
  }
  .empty.small { padding: var(--space-4); }
  .empty-icon {
    font-size: 24px;
    line-height: 1;
    color: var(--thought-dash);
  }
  .empty p {
    margin: 0;
    font-size: 12px;
    line-height: 1.6;
    max-width: 280px;
  }
</style>
