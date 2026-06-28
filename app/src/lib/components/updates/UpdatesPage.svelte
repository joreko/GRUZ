<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app'
  import { onMount, onDestroy } from 'svelte'
  import { formatDate, formatBytes } from '$lib/utils/format'
  import { commands } from '$lib/bridge/commands'
  import { onUpdateProgress, type UpdateProgress } from '$lib/bridge/events'
  import {
    fetchChangelog,
    displayVersion,
    versionCounter,
    compareVersions,
    type ReleaseChangelog,
    REPO,
  } from '$lib/utils/changelog'

  let releases = $state<ReleaseChangelog[]>([])
  let selected = $state<ReleaseChangelog | null>(null)
  let loading = $state(true)
  let error = $state('')
  let currentVersion = $state('')
  let installing = $state<string | null>(null)  // tag который устанавливается
  let confirmTag = $state<string | null>(null)   // tag ожидающий подтверждения
  let progress = $state<UpdateProgress | null>(null)
  let installError = $state('')                   // ошибка скачивания/установки
  let showBeta = $state(false)
  let showTechLines = $state(false)

  let unlistenProgress: (() => void) | null = null
  let destroyed = false

  // По умолчанию бета если нет стабильных
  $effect(() => {
    if (!loading && releases.length > 0 && releases.every(r => r.prerelease)) showBeta = true
  })

  const filteredReleases = $derived(
    releases.filter(r => showBeta ? r.prerelease : !r.prerelease)
  )

  // Счётчик текущей установленной версии (= patch = номер коммита)
  const currentCounter = $derived(versionCounter(currentVersion))

  onMount(async () => {
    currentVersion = await getVersion()
    const fn = await onUpdateProgress((p) => {
      progress = p
    })
    if (destroyed) { fn(); return }
    unlistenProgress = fn
    try {
      releases = await fetchChangelog()
    } catch {
      error = 'Не удалось загрузить историю изменений'
    } finally {
      loading = false
    }
  })

  onDestroy(() => { destroyed = true; unlistenProgress?.() })

  function pluralLines(n: number): string {
    const mod10 = n % 10, mod100 = n % 100
    if (mod10 === 1 && mod100 !== 11) return 'изменение'
    if (mod10 >= 2 && mod10 <= 4 && (mod100 < 10 || mod100 >= 20)) return 'изменения'
    return 'изменений'
  }

  // Отношение релиза к установленной версии
  type RelKind = 'current' | 'newer' | 'older'
  function relKind(tag: string): RelKind {
    const v = tag.replace(/^v/, '')
    const cmp = compareVersions(v, currentVersion)
    if (cmp === 0) return 'current'
    return cmp > 0 ? 'newer' : 'older'
  }

  function actionLabel(tag: string): string {
    return relKind(tag) === 'older' ? 'откатиться' : 'обновить'
  }

  function startInstall(tag: string) {
    if (installing) return
    confirmTag = tag
  }

  async function confirmInstall(tag: string) {
    confirmTag = null
    if (installing) return
    const version = tag.replace(/^v/, '')
    const url = `https://github.com/${REPO}/releases/download/${tag}/GRUZ_${version}_Setup.exe`
    installing = tag
    installError = ''
    progress = { downloaded: 0, total: null, pct: null, done: false }
    try {
      await commands.installVersion(url)
    } catch (e) {
      installing = null
      progress = null
      installError = `Не удалось установить v${version}: ${e}`
    }
  }

  function cancelConfirm() {
    confirmTag = null
  }

  // Цвета и метки групп — иконки теперь inline SVG в шаблоне
  const typeColor: Record<string, string> = {
    добавлено:  'var(--thought-success)',
    исправлено: 'var(--accent)',
    улучшено:   'var(--thought-info)',
    быстрее:    'var(--thought-warning)',
  }

  const typeLabel: Record<string, string> = {
    добавлено:  'Добавлено',
    исправлено: 'Исправлено',
    улучшено:   'Улучшено',
    быстрее:    'Быстрее',
  }

  // Цвета и человеческие названия scope-бейджиков
  const scopeColor: Record<string, string> = {
    installer:    'var(--status-downloading)',
    queue:        'var(--thought-info)',
    downloader:   'var(--thought-warning)',
    gallery:      'var(--thought-pink)',
    settings:     'var(--text-secondary)',
    updates:      'var(--thought-muted)',
    history:      'var(--thought-muted)',
    db:           'var(--status-warning)',
    ui:           'var(--thought-success)',
    download:     'var(--accent)',
    orchestrator: 'var(--thought-info)',
    'save-settings': 'var(--text-secondary)',
    sidebar:      'var(--thought-muted)',
    titlebar:     'var(--thought-muted)',
    ytdlp:        'var(--thought-warning)',
    bridge:       'var(--thought-muted)',
    commands:     'var(--accent-warm)',
    stores:       'var(--thought-muted)',
    design:       'var(--thought-pink)',
    tauri:        'var(--thought-muted)',
    ci:           'var(--thought-muted)',
    deps:         'var(--thought-muted)',
    'channel-prefs': 'var(--thought-info)',
    session:      'var(--thought-muted)',
  }
  const scopeLabel: Record<string, string> = {
    installer:    'Установщик',
    queue:        'Очередь',
    downloader:   'Загрузчик',
    gallery:      'Галерея',
    settings:     'Настройки',
    history:      'История',
    db:           'База данных',
    ui:           'Интерфейс',
    download:     'Загрузка',
    orchestrator: 'Оркестратор',
    'save-settings': 'Сохранение',
    sidebar:      'Навигация',
    titlebar:     'Шапка',
    ytdlp:        'yt-dlp',
    design:       'Дизайн',
    'channel-prefs': 'Каналы',
    session:      'Сессия',
    commands:     'Команды',
    updates:      'Обновления',
    bridge:       'Мост',
    stores:       'Сторы',
    tauri:        'Tauri',
    ci:           'CI',
    deps:         'Зависимости',
  }
  // scope без смысла для пользователя — скрываем
  const HIDDEN_SCOPES = new Set(['session'])
  function getScopeColor(scope: string): string {
    return scopeColor[scope] ?? 'var(--text-muted)'
  }
  function getScopeLabel(scope: string): string {
    return scopeLabel[scope] ?? scope
  }

  const TYPE_ORDER = ['добавлено', 'улучшено', 'быстрее', 'исправлено']

  const selectedGroups = $derived.by(() => {
    const map = new Map<string, typeof selectedLines>()
    for (const line of selectedLines) {
      if (!map.has(line.type)) map.set(line.type, [])
      map.get(line.type)!.push(line)
    }
    return TYPE_ORDER.filter(t => map.has(t)).map(t => ({ type: t, lines: map.get(t)! }))
  })

  // (+N) для релиза = пользовательские строки в коммитах новее текущей версии
  function countNew(rel: ReleaseChangelog): number {
    return rel.commits.reduce((s, c) => c.counter > currentCounter ? s + c.lines.length : s, 0)
  }

  // Строки выбранного релиза — пересчитываем только при смене selected/currentCounter
  const selectedLines = $derived(
    selected
      ? selected.commits.flatMap(commit =>
          commit.lines.map(line => ({
            ...line,
            counter: commit.counter,
            isNew: commit.counter > currentCounter,
          }))
        )
      : []
  )

  const selectedTechLines = $derived(
    selected
      ? selected.commits.flatMap(c => c.techLines)
      : []
  )

  // (+N) для каждого релиза в списке — пересчитывается при смене releases/currentCounter
  const releasesWithNew = $derived(
    filteredReleases.map(rel => ({ rel, newCount: countNew(rel), kind: relKind(rel.tag) }))
  )
  const newLinesCount = $derived(selectedLines.filter(l => l.isNew).length)

  // Подпись прогресса скачивания
  const progressLabel = $derived.by(() => {
    if (!progress) return ''
    if (progress.done) return 'запускаю установщик…'
    if (progress.pct !== null) {
      const size = progress.total ? ` · ${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}` : ''
      return `${progress.pct}%${size}`
    }
    return `скачано ${formatBytes(progress.downloaded)}`
  })
</script>

<div class="page">
  {#if selected}
    {@const kind = relKind(selected.tag)}
    {@const ver = displayVersion(selected.tag.replace(/^v/, ''))}
    <div class="detail">

      <button class="back" onclick={() => selected = null}>
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M10 3L5 8l5 5"/></svg>
        Все версии
      </button>

      <div class="detail-body">

        <!-- Левая колонка: мета -->
        <aside class="detail-meta">
          <div class="meta-ver">v{ver}</div>
          <div class="meta-date">{formatDate(selected.publishedAt)}</div>
          <div class="meta-badges">
            {#if selected.prerelease}<span class="badge beta">beta</span>{/if}
            {#if kind === 'current'}<span class="badge current">установлена</span>{/if}
            {#if newLinesCount > 0}<span class="badge new">+{newLinesCount} новых</span>{/if}
          </div>
          {#if kind !== 'current'}
            <div class="meta-install">
              {#if installing === selected.tag}
                <div class="dl">
                  <div class="dl-track"><div class="dl-fill" style="width:{progress?.pct ?? 5}%" class:indet={progress?.pct == null}></div></div>
                  <span class="dl-label">{progressLabel}</span>
                </div>
              {:else if confirmTag === selected.tag}
                <div class="confirm">
                  <span class="confirm-q">{kind === 'older' ? 'Откатиться?' : 'Установить?'}</span>
                  <div class="confirm-actions">
                    <button class="confirm-yes" onclick={() => confirmInstall(selected!.tag)}>да</button>
                    <button class="confirm-no" onclick={cancelConfirm}>нет</button>
                  </div>
                </div>
              {:else}
                <button
                  class="install-btn"
                  class:rollback={kind === 'older'}
                  disabled={installing !== null}
                  onclick={() => startInstall(selected!.tag)}
                >{kind === 'older' ? 'Откатиться' : 'Обновить'}</button>
              {/if}
            </div>
          {/if}
        </aside>

        <!-- Правая колонка: список изменений -->
        <div class="detail-changes">
          {#if selectedLines.length === 0}
            <p class="empty">Технические улучшения и исправления.</p>
          {:else}
            {@const hiddenCount = selected.latestCounter - selected.totalUserLines}
            <div class="groups">
              {#each selectedGroups as group, gi}
                {@const offset = selectedGroups.slice(0, gi).reduce((s, g) => s + g.lines.length, 0)}
                <div class="group" style="--gc:{typeColor[group.type]}">
                  <div class="group-header">
                    <span class="group-icon">
                      {#if group.type === 'добавлено'}
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><line x1="7" y1="1" x2="7" y2="13"/><line x1="1" y1="7" x2="13" y2="7"/></svg>
                      {:else if group.type === 'исправлено'}
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><line x1="2" y1="2" x2="12" y2="12"/><line x1="12" y1="2" x2="2" y2="12"/></svg>
                      {:else if group.type === 'улучшено'}
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="2,9 7,3 12,9"/></svg>
                      {:else if group.type === 'быстрее'}
                        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M7 1l5 6H8.5L7 13 2 7h3.5z"/></svg>
                      {/if}
                    </span>
                    <span class="group-label">{typeLabel[group.type]}</span>
                    <span class="group-count">{group.lines.length}</span>
                  </div>
                  <ul class="lines">
                    {#each group.lines as line, li}
                      <li class="line" class:line-new={line.isNew}>
                        <span class="line-num">{offset + li + 1}</span>
                        {#if line.scope && !HIDDEN_SCOPES.has(line.scope)}
                          <span class="line-scope" style="--sc:{getScopeColor(line.scope)}">{getScopeLabel(line.scope)}</span>
                        {/if}
                        <span class="line-text">{line.text}</span>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/each}
              {#if hiddenCount > 0}
                <div class="tech-block">
                  <button class="tech-toggle" onclick={() => showTechLines = !showTechLines}>
                    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
                      style="transform: rotate({showTechLines ? 90 : 0}deg); transition: var(--transition-fast)">
                      <polyline points="4,2 10,7 4,12"/>
                    </svg>
                    и ещё {hiddenCount} технических {pluralLines(hiddenCount)} под капотом
                  </button>
                  {#if showTechLines}
                    <ul class="lines tech-lines">
                      {#each selectedTechLines as line, i}
                        <li class="line">
                          <span class="line-num">{selectedLines.length + i + 1}</span>
                          {#if line.scope}
                            <span class="line-scope" style="--sc:{getScopeColor(line.scope)}">{getScopeLabel(line.scope)}</span>
                          {/if}
                          <span class="line-text">{line.text}</span>
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </div>

      </div>
    </div>

  {:else}
    <!-- Список релизов -->
    <div class="top-row">
      <h2>Обновления</h2>
      <div class="tabs">
        <button class="tab" class:active={!showBeta} onclick={() => showBeta = false}>Стабильные</button>
        <button class="tab" class:active={showBeta} onclick={() => showBeta = true}>Бета</button>
      </div>
    </div>

    {#if loading}
      <div class="loading">
        <div class="spinner"></div>
        <span>Загружаю историю изменений…</span>
      </div>
    {:else if error}
      <div class="error">{error}</div>
    {:else if releases.length === 0}
      <div class="error">Релизов пока нет</div>
    {:else}
      {#if installError}
        <div class="install-error" role="alert">
          <span>{installError}</span>
          <button class="install-error-close" onclick={() => installError = ''} aria-label="Скрыть">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
      {/if}
      <div class="grid" role="listbox" aria-label="Версии">
        {#each releasesWithNew as { rel, newCount, kind }}
          <div
            class="card"
            class:card-current={kind === 'current'}
            role="button"
            tabindex="0"
            onclick={() => selected = rel}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (selected = rel)}
          >
            <div class="card-top">
              <span class="card-tag">v{displayVersion(rel.tag.replace(/^v/, ''))}</span>
              {#if rel.prerelease}<span class="badge beta">beta</span>{/if}
              {#if kind === 'current'}<span class="badge current">✓</span>{/if}
              {#if kind === 'older'}<span class="badge old">старее</span>{/if}
              {#if newCount > 0}<span class="badge new">+{newCount}</span>{/if}
            </div>
            <p class="card-date">{formatDate(rel.publishedAt)}</p>
            {#if rel.totalUserLines > 0}
              <p class="card-stat">{rel.totalUserLines} {pluralLines(rel.totalUserLines)}</p>
            {:else}
              <p class="card-stat">технические улучшения</p>
            {/if}

            {#if kind !== 'current'}
              {#if installing === rel.tag}
                <!-- Прогресс скачивания -->
                <div class="dl" onclick={(e) => e.stopPropagation()} role="presentation">
                  <div class="dl-track"><div class="dl-fill" style="width:{progress?.pct ?? 5}%" class:indet={progress?.pct == null}></div></div>
                  <span class="dl-label">{progressLabel}</span>
                </div>
              {:else if confirmTag === rel.tag}
                <!-- Подтверждение -->
                <div class="confirm" onclick={(e) => e.stopPropagation()} role="presentation">
                  <span class="confirm-q">{kind === 'older' ? 'Откатиться на эту версию?' : 'Установить эту версию?'}</span>
                  <div class="confirm-actions">
                    <button class="confirm-yes" onclick={() => confirmInstall(rel.tag)}>да, {actionLabel(rel.tag)}</button>
                    <button class="confirm-no" onclick={cancelConfirm}>отмена</button>
                  </div>
                </div>
              {:else}
                <button
                  class="install-btn"
                  class:rollback={kind === 'older'}
                  disabled={installing !== null}
                  onclick={(e) => { e.stopPropagation(); startInstall(rel.tag) }}
                >
                  {actionLabel(rel.tag)}
                </button>
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<style>
  .page {
    padding: var(--space-8) var(--space-9);
    height: 100%;
    overflow-y: scroll;
    box-sizing: border-box;
  }
  .page::-webkit-scrollbar { width: 0; }
  .page { scrollbar-width: none; }
  h2 {
    margin: 0;
    font-size: var(--text-xl); font-weight: 700; color: var(--text-primary);
  }

  .top-row {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: var(--space-6);
  }

  .tabs {
    display: flex;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    padding: 3px;
  }
  .tab {
    height: 30px; padding: 0 14px;
    background: transparent; border: none; border-radius: var(--radius-md);
    color: var(--text-muted); font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .tab:hover { color: var(--text-secondary); }
  .tab.active {
    background: rgba(0,0,0,0.35); color: var(--text-primary); font-weight: 600;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
  }

  /* Сетка */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-3);
  }
  .card {
    text-align: left; width: 100%; font: inherit;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    padding: var(--space-4);
    transition: transform var(--transition-fast), box-shadow var(--transition-fast), border-color var(--transition-fast);
  }
  .card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0,0,0,0.35);
    border-color: var(--border-default);
  }
  .card-current {
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 15%, transparent);
  }
  .card-top { display: flex; align-items: center; gap: var(--space-2); margin-bottom: var(--space-2); }
  .card-tag { font-size: 13px; font-weight: 700; color: var(--text-primary); font-variant-numeric: tabular-nums; }
  .card-date { margin: 0 0 var(--space-2); font-size: 11px; color: var(--text-muted); }
  .card-stat { margin: 0; font-size: 11px; color: var(--text-muted); }

  /* Badges */
  .badge {
    font-size: 9px; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase;
    padding: 2px 5px; border-radius: var(--radius-sm);
  }
  .badge.current { background: color-mix(in srgb, var(--accent) 15%, transparent); color: var(--accent); }
  .badge.new { background: color-mix(in srgb, var(--status-success) 12%, transparent); color: var(--thought-success); }
  .badge.beta { background: rgba(99,102,241,0.15); color: var(--status-downloading); }
  .badge.old { background: rgba(255,255,255,0.06); color: var(--text-muted); }

  /* Детальная страница */
  .detail { display: flex; flex-direction: column; gap: var(--space-6); }

  .back {
    display: inline-flex; align-items: center; gap: var(--space-1);
    background: none; border: none; color: var(--text-muted); font-size: 12px;
    cursor: pointer; padding: 4px 8px 4px 4px; margin-left: -4px;
    border-radius: var(--radius-md);
    transition: color var(--transition-fast), background var(--transition-fast);
  }
  .back:hover { color: var(--text-primary); background: var(--bg-elevated); }
  .back svg { width: 14px; height: 14px; }

  /* Двухколоночный layout */
  .detail-body {
    display: grid;
    grid-template-columns: 160px 1fr;
    gap: var(--space-8);
    align-items: start;
  }

  /* Левая колонка мета */
  .detail-meta {
    display: flex; flex-direction: column; gap: var(--space-3);
    position: sticky; top: 0;
  }
  .meta-ver {
    font-size: var(--text-xl); font-weight: 700; color: var(--text-primary);
    letter-spacing: -0.01em; font-variant-numeric: tabular-nums;
  }
  .meta-date { font-size: 11px; color: var(--text-muted); }
  .meta-badges { display: flex; flex-direction: column; gap: var(--space-1); align-items: flex-start; }
  .meta-install { margin-top: var(--space-1); }

  .install-btn {
    width: 100%; padding: 7px 0;
    border-radius: var(--radius-md);
    border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent); font-size: 12px; font-weight: 600;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }
  .install-btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-color: var(--accent);
  }
  .install-btn:disabled { opacity: 0.35; cursor: not-allowed; }
  .install-btn.rollback {
    border-color: var(--border-default);
    background: transparent; color: var(--text-muted);
  }
  .install-btn.rollback:hover:not(:disabled) {
    background: var(--bg-elevated); color: var(--text-secondary);
    border-color: var(--border-strong);
  }

  /* Подтверждение */
  .confirm { display: flex; flex-direction: column; gap: var(--space-2); }
  .confirm-q { font-size: 11px; color: var(--text-secondary); }
  .confirm-actions { display: flex; gap: var(--space-1); }
  .confirm-yes, .confirm-no {
    flex: 1; padding: 5px 0; border-radius: var(--radius-sm); font-size: 11px; font-weight: 600; cursor: pointer;
    transition: background var(--transition-fast);
  }
  .confirm-yes {
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent); color: var(--accent);
  }
  .confirm-yes:hover { background: color-mix(in srgb, var(--accent) 22%, transparent); }
  .confirm-no { border: 1px solid var(--border-subtle); background: transparent; color: var(--text-muted); }
  .confirm-no:hover { background: var(--bg-elevated); color: var(--text-secondary); }

  /* Прогресс */
  .dl { display: flex; flex-direction: column; gap: var(--space-1); }
  .dl-track { height: 3px; border-radius: 2px; background: var(--bg-overlay); overflow: hidden; }
  .dl-fill { height: 100%; background: var(--thought-info); transition: width 0.25s ease; }
  .dl-fill.indet { animation: indet 1s ease-in-out infinite; }
  @keyframes indet { 0%,100% { opacity: 0.4; } 50% { opacity: 1; } }
  .dl-label { font-size: 10px; color: var(--thought-info); font-variant-numeric: tabular-nums; }

  /* Правая колонка — список изменений */
  .detail-changes { min-width: 0; }

  .groups { display: flex; flex-direction: column; gap: var(--space-6); }
  .group { display: flex; flex-direction: column; gap: 2px; }

  .group-header {
    display: flex; align-items: center; gap: var(--space-2);
    padding: 0 var(--space-2) var(--space-2);
    border-bottom: 1px solid var(--border-subtle);
    margin-bottom: var(--space-1);
  }
  .group-icon {
    width: 16px; height: 16px; display: grid; place-items: center; flex-shrink: 0;
    color: var(--gc);
  }
  .group-icon svg { width: 12px; height: 12px; }
  .group-label {
    font-size: 11px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: var(--gc);
  }
  .group-count {
    margin-left: auto; font-size: 10px; color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .lines { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 2px; }
  .line {
    display: flex; align-items: baseline; gap: var(--space-2);
    padding: 5px var(--space-2);
    border-radius: var(--radius-md);
    transition: background 150ms ease-out;
  }
  .line:hover { background: var(--bg-elevated); }
  .line-new .line-num { color: var(--thought-success); opacity: 1; }
  .line-num {
    font-size: 10px; color: var(--text-muted); opacity: 0.35;
    font-variant-numeric: tabular-nums; font-family: var(--font-mono);
    min-width: 16px; text-align: right; flex-shrink: 0;
    border-right: 1px solid var(--border-subtle); padding-right: var(--space-2);
  }
  .line-text { font-size: 13px; color: var(--text-primary); line-height: 1.4; flex: 1; min-width: 0; }
  .line-scope {
    font-size: 9px; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase;
    color: var(--sc); font-family: var(--font-mono); flex-shrink: 0;
    background: color-mix(in srgb, var(--sc) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--sc) 30%, transparent);
    border-radius: var(--radius-sm); padding: 1px 5px; line-height: 16px;
  }

  .empty {
    font-size: 13px; color: var(--text-muted); padding: 24px 0;
  }
  .tech-block {
    margin-top: var(--space-3); padding-top: var(--space-3);
    border-top: 1px solid var(--border-subtle);
  }
  .tech-toggle {
    display: flex; align-items: center; gap: var(--space-2);
    background: none; border: none; cursor: pointer;
    font-size: 12px; color: var(--text-muted); padding: 0;
  }
  .tech-toggle:hover { color: var(--text-secondary); }
  .tech-toggle svg { width: 12px; height: 12px; flex-shrink: 0; }
  .tech-lines { margin-top: var(--space-2); }
  .tech-lines .line { opacity: 0.7; }

  /* Loading */
  .loading {
    display: flex; align-items: center; gap: 10px;
    padding: 48px 0; color: var(--text-muted); font-size: 13px;
  }
  .spinner {
    width: 16px; height: 16px;
    border: 2px solid var(--border-default); border-top-color: var(--accent);
    border-radius: 50%; animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .error {
    padding: 48px 0; color: var(--text-muted); font-size: 13px;
  }

  .install-error {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    margin-bottom: 16px; padding: 10px 14px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: var(--radius-card);
    color: var(--accent); font-size: 12px;
  }
  .install-error-close {
    flex-shrink: 0;
    background: none; border: none; color: var(--accent); cursor: pointer;
    font-size: 12px; padding: 0 2px; opacity: 0.7; transition: opacity var(--transition-fast);
  }
  .install-error-close:hover { opacity: 1; }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
