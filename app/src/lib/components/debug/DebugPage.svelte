<script lang="ts">
  import { onMount } from 'svelte'
  import type { UnlistenFn } from '@tauri-apps/api/event'
  import { commands } from '$lib/bridge/commands'
  import { onLogLine, onDownloadProgress, onDownloadCompleted, onDownloadFailed } from '$lib/bridge/events'
  import { queue } from '$lib/stores/queue.svelte'
  import { tooltip } from '$lib/utils/tooltip'
  import type { DownloadProgress } from '$lib/bridge/types'
  import type {
    DownloadCompletedPayload,
    DownloadFailedPayload,
  } from '$lib/bridge/events'

  // Модульный кеш task_id → title — живёт между монтированиями DebugPage
  const TASK_TITLES = new Map<string, string>()
  function taskTitle(task_id: string): string {
    const cached = TASK_TITLES.get(task_id)
    if (cached) return cached
    const q = queue.tasks.find((x) => x.id === task_id)?.title
    if (q) {
      TASK_TITLES.set(task_id, q)
      return q
    }
    return '—'
  }

  type Filter = 'all' | 'progress' | 'debug' | 'info' | 'warn' | 'error'
  const LOG_RE = /^\[(\d{2}:\d{2}:\d{2}\.\d{3})\]\s+(\w+)\s+([\w:]+):\s+(.+)$/
  const KV_RE = /\s*(\w+)=(?:Some\("([^"]*)"\)|Some\(([^)]+)\)|None|([^\s]+))/g
  // Ленивый парсер: строка начинается с [ts], но не совпала со строгим LOG_RE
  // (нетипичный target/уровень) — вытаскиваем хотя бы время и тело.
  const TS_RE = /^\[(\d{2}:\d{2}:\d{2}\.\d{3})\]/

  const MODULE_COLORS: Record<string, string> = {
    orchestrator: '#b37feb',
    process: '#5dade2',
    pipeline: '#73c6b6',
    ytdlp: '#f0b27a',
    queue: '#85c1e9',
    db: '#aeb6bf',
    error: '#e74c3c',
    logging: '#95a5a6',
    support: '#48c9b0',
    history: '#f5b041',
    session: '#bb8fce',
    settings: '#5dade2',
    update: '#e67e22',
    channel_prefs: '#f1948a',
    shortcuts: '#82e0aa',
    spec: '#aed6f1',
    root: '#aeb6bf',
  }

  interface Field {
    key: string
    val: string
  }

  interface Entry {
    id: number
    kind: 'log' | 'progress'
    text?: string
    level?: string
    // parsed
    ts?: string
    modShort?: string
    body?: string
    fields?: Field[]
    taskId?: string
    // progress
    task_id?: string
    phase?: string
    percent?: number
    speed?: string | null
    title?: string
    done?: boolean
  }

  let entries = $state<Entry[]>([])
  let filter = $state<Filter>('all')
  let autoScroll = $state(true)
  let container = $state<HTMLDivElement | null>(null)
  let unlisten: UnlistenFn[] = []
  let levelBusy = $state(false)
  let uid = 0
  const MAX_UI = 5000
  let selectedEntry = $state<Entry | null>(null)

  function closeModal() {
    selectedEntry = null
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') closeModal()
  }

  function hscroll(node: HTMLElement) {
    function onWheel(e: WheelEvent) {
      const f = (e.target as HTMLElement).closest('.ln-fields') as HTMLElement | null
      if (f && f.scrollWidth > f.clientWidth && e.deltaY) {
        f.scrollLeft += e.deltaY
        e.preventDefault()
      }
    }
    node.addEventListener('wheel', onWheel, { passive: false })
    return { destroy() { node.removeEventListener('wheel', onWheel) } }
  }

  function levelOf(line: string): string {
    const m = line.match(/\] (\w+) /)
    return m ? m[1].toLowerCase() : 'info'
  }

  function passFilter(kind: string, level?: string): boolean {
    if (kind === 'progress') return filter === 'all' || filter === 'progress'
    if (filter === 'all' || filter === 'progress') return filter === 'all'
    return level === filter
  }

  function phaseOf(p: DownloadProgress): string {
    if (p.stream_type === 'video') return 'Видео'
    if (p.stream_type === 'audio') return 'Аудио'
    if (p.state === 'converting') return 'Слияние'
    return 'Загрузка'
  }

  function nowTs(): string {
    const d = new Date()
    return d.toLocaleTimeString('ru-RU', { hour12: false }) + '.' + String(d.getMilliseconds()).padStart(3, '0')
  }

  function clampPct(v: number): number {
    return Math.max(0, Math.min(100, v))
  }

  function shortMod(mod: string): string {
    const i = mod.lastIndexOf('::')
    return i >= 0 ? mod.slice(i + 2) : 'root'
  }

  function cleanValue(raw: string): string {
    if (!raw || raw === 'None') return ''
    let v = raw
    if (v.startsWith('"') && v.endsWith('"')) v = v.slice(1, -1)
    return v
  }

  function parseLog(text: string): Entry {
    const m = text.match(LOG_RE)
    if (!m) {
      const ts = text.match(TS_RE)
      // Строка с таймстампом, но нестандартного вида — показываем как лог
      // (не «сырую»), чтобы весь поток выглядел единообразно.
      return {
        id: uid++,
        kind: 'log',
        text,
        level: levelOf(text),
        ts: ts ? ts[1] : undefined,
        modShort: undefined,
        body: text.replace(TS_RE, '').replace(/^\s*\S+\s+\S+:\s*/, '').replace(/^\s+\S+/, '').trim() || text,
      }
    }
    const [, ts, lvl, mod, rest] = m
    const pipe = rest.indexOf(' | ')
    const body = pipe >= 0 ? rest.slice(0, pipe) : rest
    const rawFields = pipe >= 0 ? rest.slice(pipe + 3) : ''
    // В rawFields может быть второй ` | span=...` — отцепляем
    const spanPipe = rawFields.indexOf(' | ')
    const fieldsStr = spanPipe >= 0 ? rawFields.slice(0, spanPipe) : rawFields
    const fields: Field[] = []
    let taskId: string | undefined
    if (fieldsStr) {
      let match: RegExpExecArray | null
      const re = new RegExp(KV_RE)
      while ((match = re.exec(fieldsStr)) !== null) {
        const [, key, q, p, s] = match
        if (key === 'task_id') {
          const fullId = q || p || s || ''
          taskId = fullId.slice(0, 8)
          continue
        }
        const val = cleanValue(q || p || s || '')
        if (!val) continue
        if (val.length > 44) {
          fields.push({ key, val: val.slice(0, 40) + '…' })
        } else {
          fields.push({ key, val })
        }
      }
    }
    return {
      id: uid++,
      kind: 'log',
      text,
      level: lvl.toLowerCase(),
      ts,
      modShort: shortMod(mod),
      body,
      fields: fields.length > 0 ? fields : undefined,
      taskId,
    }
  }

  function pushLog(text: string) {
    entries.push(parseLog(text))
    if (entries.length > MAX_UI) entries.shift()
  }

  function upsertProgress(p: DownloadProgress) {
    // state: 'finished' = один стрим (video/audio/converting) докачался.
    // Не трогаем фазу — оставляем текущую, просто 100% и done.
    if (p.state === 'finished') {
      let idx = -1
      for (let i = entries.length - 1; i >= 0; i--) {
        const e = entries[i]
        if (e.kind === 'progress' && e.task_id === p.task_id && !e.done) {
          idx = i
          break
        }
      }
      if (idx >= 0) {
        entries[idx].ts = nowTs()
        entries[idx].percent = 100
        entries[idx].done = true
        entries[idx].speed = null
      } else {
        // стрим завершился, но бара ещё не было — создаём с фазой из stream_type
        const phase = p.stream_type === 'video' ? 'Видео' : p.stream_type === 'audio' ? 'Аудио' : 'Загрузка'
        const title = taskTitle(p.task_id)
        entries.push({
          id: uid++, kind: 'progress', ts: nowTs(),
          task_id: p.task_id, phase, percent: 100, speed: null, title, done: true,
        })
      }
      return
    }

    // state: 'downloading' | 'converting' — живой прогресс стрима
    let idx = -1
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === p.task_id && !e.done) {
        idx = i
        break
      }
    }
    const phase = phaseOf(p)
    const title = taskTitle(p.task_id)
    if (idx >= 0) {
      entries[idx].phase = phase
      entries[idx].percent = p.progress
      entries[idx].speed = p.speed
      entries[idx].title = title
    } else {
      entries.push({
        id: uid++,
        kind: 'progress', ts: nowTs(),
        task_id: p.task_id,
        phase,
        percent: p.progress,
        speed: p.speed,
        title,
        done: false,
      })
    }
  }

  function addDoneBar(task_id: string) {
    // дофинализировать активный бар (слияние могло застрять на 99%)
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === task_id && !e.done) {
        e.ts = nowTs()
        e.percent = 100
        e.done = true
        e.speed = null
      }
    }
    // добавить «Готово», если ещё нет
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === task_id && e.phase === 'Готово') return
    }
    entries.push({
      id: uid++, kind: 'progress', ts: nowTs(), phase: 'Готово', percent: 100, speed: null,
      task_id, title: taskTitle(task_id), done: true,
    })
  }

  function finalize(id: string) {
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === id && !e.done) {
        e.done = true
        break
      }
    }
  }

  async function setLevel(lvl: string) {
    if (levelBusy) return
    levelBusy = true
    try {
      await commands.setLogLevel(`gruz=${lvl}`)
    } finally {
      levelBusy = false
    }
  }

  function copyAll() {
    const text = entries
      .map((e) =>
        e.kind === 'log'
          ? e.text
          : `${e.ts ?? ''} ${e.phase} ${(e.percent ?? 0).toFixed(0)}% ${e.title}${e.speed ? '  ' + e.speed : ''}`,
      )
      .join('\n')
    if (text) commands.writeText(text)
  }

  const DONE_RE = /download (completed successfully|finalized)[\s\S]*?task_id=([\da-f-]+)/
  const STREAM_RE = /\|\s*task_id=([\da-f-]+)/
  const START_RE = /starting yt-dlp process.*?stream=Some\("(\w+)"\)/
  const FINISH_RE = /stream finished.*?stream_type=Some\("(\w+)"\)/
  const MERGE_START_RE = /ffmpeg merge: start.*?task_id=([\da-f-]+)/
  const MERGE_FINISH_RE = /ffmpeg merge finished.*?task_id=([\da-f-]+)/
  const FP_Q_RE = /file_path=Some\("([^"]+)"\)/
  const P_RAW_RE = /path=(.+?)(?:\s*\|\s*)?$/

  function streamPhase(stream: string): string {
    return stream === 'video' ? 'Видео' : stream === 'audio' ? 'Аудио' : 'Загрузка'
  }

  function initStreamBar(task_id: string, phase: string) {
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === task_id && e.phase === phase) return
    }
    entries.push({
      id: uid++, kind: 'progress', ts: nowTs(), task_id, phase, percent: 0, speed: null,
      title: taskTitle(task_id), done: false,
    })
  }

  function finalizeStreamBar(task_id: string, phase: string) {
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i]
      if (e.kind === 'progress' && e.task_id === task_id && e.phase === phase && !e.done) {
        e.ts = nowTs(); e.percent = 100; e.done = true; e.speed = null; return
      }
    }
  }

  function replayHistory(lines: string[]) {
    const seen = new Set<string>()
    for (const line of lines) {
      pushLog(line)

      // кешировать название таска из file_path / path и обновить существующие бары
      const fp = line.match(FP_Q_RE) ?? line.match(P_RAW_RE)
      if (fp) {
        const tid = line.match(STREAM_RE)
        if (tid && !TASK_TITLES.has(tid[1])) {
          const raw = fp[1].replace(/\\/g, '/')
          const base = raw.split('/').pop() ?? raw
          const title = base.replace(/\.[^/.]+$/, '')
          TASK_TITLES.set(tid[1], title)
          for (let i = 0; i < entries.length; i++) {
            const e = entries[i]
            if (e.kind === 'progress' && e.task_id === tid[1] && (e.title === '—' || e.title === undefined)) {
              e.title = title
            }
          }
        }
      }

      // старт стрима (video/audio)
      const sm = line.match(START_RE)
      if (sm) {
        const tid = line.match(STREAM_RE)
        if (tid) initStreamBar(tid[1], streamPhase(sm[1]))
      }

      // финиш стрима (video/audio)
      const fm = line.match(FINISH_RE)
      if (fm) {
        const tid = line.match(STREAM_RE)
        if (tid) finalizeStreamBar(tid[1], streamPhase(fm[1]))
      }

      // старт слияния
      const mm = line.match(MERGE_START_RE)
      if (mm) initStreamBar(mm[1], 'Слияние')

      // финиш слияния
      const mf = line.match(MERGE_FINISH_RE)
      if (mf) finalizeStreamBar(mf[1], 'Слияние')

      // финиш задачи
      const dm = line.match(DONE_RE)
      if (dm && !seen.has(dm[2])) {
        seen.add(dm[2])
        addDoneBar(dm[2])
      }
    }
  }

  onMount(() => {
    commands.getLogHistory().then((h) => replayHistory(h.slice(-MAX_UI))).catch(() => {})
    onLogLine(pushLog).then((u) => unlisten.push(u))
    onDownloadProgress(upsertProgress).then((u) => unlisten.push(u))
    onDownloadCompleted((p: DownloadCompletedPayload) => addDoneBar(p.task_id)).then((u) =>
      unlisten.push(u),
    )
    onDownloadFailed((p: DownloadFailedPayload) => finalize(p.task_id)).then((u) => unlisten.push(u))
    return () => unlisten.forEach((u) => u())
  })

  $effect(() => {
    entries
    if (autoScroll && container) container.scrollTop = container.scrollHeight
  })

  function onScroll() {
    if (!container) return
    autoScroll = container.scrollHeight - container.scrollTop - container.clientHeight < 40
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="debug-page">
  <header class="head">
    <div class="seg" role="group">
      {#each (['all', 'progress', 'debug', 'info', 'warn', 'error'] as Filter[]) as f}
        <button class:active={filter === f} onclick={() => (filter = f)}>
          {f === 'all' ? 'Все' : f === 'progress' ? 'LOAD' : f.toUpperCase()}
        </button>
      {/each}
    </div>
    <button class="copy-btn" onclick={copyAll} disabled={!entries.length} aria-label="Копировать лог" use:tooltip={'Копировать лог'}>
      <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <rect x="4.5" y="2.5" width="9" height="11" rx="1.5" />
        <path d="M2.5 5v8.5a1.5 1.5 0 0 0 1.5 1.5h6.5" />
      </svg>
    </button>
  </header>

  <div class="log" bind:this={container} onscroll={onScroll} use:hscroll>
    {#each entries as e (e.id)}
      {#if passFilter(e.kind, e.level)}
        {#if e.kind === 'log'}
          {#if e.ts}
            <div class="ln-struct" class:err={e.level === 'error'} class:warn={e.level === 'warn'}>
              <span class="ln-ts">{e.ts}</span>
              <span class="ln-lvl" class:lvl-debug={e.level === 'debug'} class:lvl-info={e.level === 'info'} class:lvl-warn={e.level === 'warn'} class:lvl-err={e.level === 'error'}>{e.level}</span>
              {#if e.modShort}
                <span class="ln-mod" style="color: {MODULE_COLORS[e.modShort] ?? 'var(--text-muted)'}">{e.modShort}</span>
              {/if}
              <span class="ln-body" role="button" tabindex="0" onclick={() => (selectedEntry = e)} onkeydown={(ev) => ev.key === 'Enter' && (selectedEntry = e)}>{e.body}</span>
              {#if e.taskId}
                <span class="ln-task">{e.taskId}…</span>
              {/if}
              {#if e.fields}
                <span class="ln-fields">
                  {#each e.fields as f}
                    <span class="ln-tag">{f.key}<span class="ln-tag-v">{f.val}</span></span>
                  {/each}
                </span>
              {/if}
            </div>
          {:else}
            <div class="ln-raw" role="button" tabindex="0" onclick={() => (selectedEntry = e)} onkeydown={(ev) => ev.key === 'Enter' && (selectedEntry = e)}>{e.text}</div>
          {/if}
        {:else}
          <div class="dl-row" class:done={e.done}>
            {#if e.ts}<span class="ln-ts">{e.ts}</span>{/if}
            <span class="dl-type l">LOAD</span>
            <span class="dl-phase" class:v={e.phase === 'Видео'} class:a={e.phase === 'Аудио'} class:m={e.phase === 'Слияние'} class:d={e.done}>{e.phase}</span>
            <span class="dl-title" title={e.title}>{e.title}</span>
            <div class="dl-bar"><div class="dl-fill" style="width:{clampPct(e.percent ?? 0)}%"></div></div>
            <span class="dl-pct">{(e.percent ?? 0).toFixed(0)}%</span>
            {#if e.speed}<span class="dl-speed">{e.speed}</span>{/if}
          </div>
        {/if}
      {/if}
    {/each}
    {#if !entries.length}
      <div class="empty">Лог пуст — подождите событий или переключите уровень бэкенда на debug/trace.</div>
    {/if}
  </div>
</div>

{#if selectedEntry}
  <div class="modal-overlay" onclick={closeModal} onkeydown={(e) => e.key === 'Escape' && closeModal()} role="none">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && closeModal()} role="dialog" tabindex="-1" aria-modal="true">
      <div class="modal-head">
        <span class="ln-lvl" class:lvl-debug={selectedEntry.level === 'debug'} class:lvl-info={selectedEntry.level === 'info'} class:lvl-warn={selectedEntry.level === 'warn'} class:lvl-err={selectedEntry.level === 'error'}>{selectedEntry.level}</span>
        <span class="m-mod">{selectedEntry.modShort}</span>
        <span class="m-ts">{selectedEntry.ts}</span>
        <div class="m-spacer"></div>
        <button class="m-close" onclick={closeModal} aria-label="Закрыть">&times;</button>
      </div>
      <div class="modal-body">
        <div class="m-row">
          <span class="m-label">Сообщение</span>
          <span class="m-val">{selectedEntry.body ?? selectedEntry.text}</span>
        </div>
        {#if selectedEntry.taskId}
          <div class="m-row">
            <span class="m-label">Задача</span>
            <span class="m-val m-mono">{selectedEntry.taskId}</span>
          </div>
        {/if}
        {#if selectedEntry.fields}
          <div class="m-row">
            <span class="m-label">Поля</span>
            <div class="m-fields">
              {#each selectedEntry.fields as f}
                <span class="m-field"><span class="m-fk">{f.key}</span><span class="m-fv">{f.val}</span></span>
              {/each}
            </div>
          </div>
        {/if}
        <div class="m-row">
          <span class="m-label">Сырая строка</span>
          <span class="m-val m-mono m-raw">{selectedEntry.text}</span>
        </div>
      </div>
      <div class="modal-foot">
        <button class="m-copy" onclick={() => { commands.writeText(selectedEntry?.text ?? ''); }}>Копировать строку</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .debug-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--space-8) var(--space-9);
    gap: 12px;
    background: var(--bg-base);
    color: var(--text-secondary);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .head .seg {
    display: flex;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 2px;
    flex-shrink: 0;
  }
  .head .seg button {
    height: 26px; padding: 0 10px;
    background: transparent; border: none; border-radius: 6px;
    color: var(--text-muted); font-size: 11px; font-weight: 500; cursor: pointer;
    font-family: inherit;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .head .seg button:hover { color: var(--text-secondary); }
  .head .seg button.active { background: color-mix(in srgb, var(--scrim) 35%, transparent); color: var(--text-primary); font-weight: 600; }

  .copy-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: 7px; color: var(--text-secondary); cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    flex-shrink: 0;
  }
  .copy-btn svg { width: 14px; height: 14px; }
  .copy-btn:hover:not(:disabled) { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .copy-btn:active:not(:disabled) { background: var(--bg-overlay); transform: scale(0.94); }
  .copy-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .copy-btn:disabled { opacity: 0.35; cursor: default; }

  .log {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
    padding: var(--space-3) 12px var(--space-3) 0;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* — Структурированная строка лога — */
  .ln-struct {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0;
    line-height: 1.7;
    border-bottom: 1px solid transparent;
    transition: color var(--transition-fast);
  }
  .ln-struct:hover .ln-body { color: var(--accent); }

  .ln-ts {
    flex-shrink: 0;
    color: var(--text-muted);
    opacity: 0.5;
    font-size: 10px;
    min-width: 82px;
  }
  .ln-lvl {
    flex-shrink: 0;
    min-width: 44px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-align: center;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-overlay);
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .lvl-debug { color: #95a5a6; }
  .lvl-info { color: #5dade2; background: rgba(93,173,226,0.1); }
  .lvl-warn { color: #f0b27a; background: rgba(240,178,122,0.1); }
  .lvl-err { color: var(--accent); background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .dl-type {
    flex-shrink: 0;
    min-width: 44px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-align: center;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-overlay);
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .dl-type.l { color: #48c9b0; background: rgba(72,201,176,0.1); }

  .ln-mod {
    flex-shrink: 0;
    font-size: 10px;
    font-weight: 600;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-overlay);
    min-width: 60px;
    text-align: center;
  }
  .ln-body {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .ln-task {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--text-muted);
    opacity: 0.6;
    background: var(--bg-overlay);
    padding: 1px 5px;
    border-radius: 3px;
  }
  .ln-fields {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    max-width: 380px;
    overflow-x: hidden;
    white-space: nowrap;
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  .ln-fields::-webkit-scrollbar { display: none; }
  .ln-struct:hover .ln-fields { overflow-x: auto; }
  .ln-tag {
    display: inline-flex;
    align-items: center;
    gap: 0;
    font-size: 10px;
    border-radius: 3px;
    background: var(--bg-overlay);
    white-space: nowrap;
    color: var(--text-muted);
    opacity: 0.55;
    padding: 0 4px;
  }
  .ln-tag span {
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ln-tag span::before {
    content: '=';
    color: var(--text-muted);
    opacity: 0.3;
    margin-right: 1px;
  }

  .ln-struct.warn .ln-body { color: #f0b27a; }
  .ln-struct.err .ln-body { color: var(--accent); }

  /* fallback для нераспарсенных строк */
  .ln-raw {
    color: var(--text-secondary);
    font-size: 11px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-word;
    padding-left: 8px;
    border-left: 2px solid var(--border-subtle);
    cursor: pointer;
    transition: color var(--transition-fast);
  }
  .ln-raw:hover { color: var(--accent); }

  /* — Прогресс-бар — */
  .dl-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 1px 0;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
  }
  .dl-phase {
    flex-shrink: 0;
    min-width: 44px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    text-align: center;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--bg-overlay);
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .dl-phase.v { color: #5dade2; background: rgba(93,173,226,0.1); }
  .dl-phase.a { color: #48c9b0; background: rgba(72,201,176,0.1); }
  .dl-phase.m { color: #f0b27a; background: rgba(240,178,122,0.1); }
  .dl-phase.d { background: var(--bg-overlay); }
  .dl-title {
    flex-shrink: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-secondary);
  }
  .dl-bar {
    flex: 1 1 auto;
    height: 10px;
    border-radius: 9999px;
    background: var(--bg-base);
    overflow: hidden;
    box-shadow: inset 0 0 0 1px var(--border-default);
  }
  .dl-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--thought-info));
    border-radius: 9999px;
    transition: width 0.2s linear;
  }
  .dl-row.done .dl-fill { background: var(--thought-success); }
  .dl-pct {
    flex-shrink: 0;
    min-width: 38px;
    text-align: right;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }
  .dl-speed {
    flex-shrink: 0;
    min-width: 78px;
    text-align: right;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .empty {
    color: var(--text-muted);
    font-style: italic;
    padding: var(--space-4);
  }

  .log::-webkit-scrollbar {
    width: 7px;
    height: 7px;
  }
  .log::-webkit-scrollbar-track {
    background: transparent;
  }
  .log::-webkit-scrollbar-thumb {
    background: var(--border-default);
    border-radius: 9999px;
    border: 1px solid transparent;
    background-clip: content-box;
    min-height: 40px;
  }
  .log::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
    border: 1px solid transparent;
    background-clip: content-box;
  }
  .log::-webkit-scrollbar-corner {
    background: transparent;
  }

  /* — Модалка деталей лога — */
  .modal-overlay {
    position: fixed; inset: 0;
    z-index: 1000;
    background: color-mix(in srgb, var(--scrim) 45%, transparent);
    display: flex; align-items: center; justify-content: center;
    backdrop-filter: blur(2px);
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 32px rgba(0,0,0,0.35);
    max-width: 680px;
    width: 90%;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .modal-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: var(--space-4) var(--space-6);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .m-mod {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .m-ts {
    font-size: 11px;
    color: var(--text-muted);
    opacity: 0.6;
  }
  .m-spacer { flex: 1; }
  .m-close {
    display: flex; align-items: center; justify-content: center;
    width: 24px; height: 24px;
    background: transparent; border: none;
    color: var(--text-muted); font-size: 18px; cursor: pointer;
    border-radius: 6px;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .m-close:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .modal-body {
    padding: var(--space-4) var(--space-6);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .m-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .m-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    opacity: 0.6;
  }
  .m-val {
    font-size: 13px;
    color: var(--text-primary);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .m-mono {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }
  .m-raw {
    font-size: 11px;
    color: var(--text-secondary);
    padding: 8px;
    background: var(--bg-base);
    border-radius: var(--radius-sm);
    max-height: 200px;
    overflow-y: auto;
  }
  .m-fields {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .m-field {
    display: inline-flex;
    align-items: center;
    font-size: 11px;
    border-radius: 4px;
    background: var(--bg-overlay);
    white-space: nowrap;
  }
  .m-fk {
    padding: 2px 0 2px 6px;
    color: var(--text-muted);
    font-weight: 600;
  }
  .m-fk::after { content: '='; color: var(--text-muted); opacity: 0.3; margin-left: 2px; }
  .m-fv {
    padding: 2px 6px 2px 2px;
    color: var(--text-primary);
    word-break: break-all;
  }
  .modal-foot {
    display: flex;
    justify-content: flex-end;
    padding: var(--space-3) var(--space-6);
    border-top: 1px solid var(--border-subtle);
    flex-shrink: 0;
  }
  .m-copy {
    height: 28px; padding: 0 12px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: 7px; color: var(--text-secondary); cursor: pointer;
    font-size: 11px; font-family: inherit; font-weight: 500;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .m-copy:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .m-copy:active { transform: scale(0.97); }
</style>
