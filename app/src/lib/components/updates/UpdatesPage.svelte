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
  } from '$lib/utils/changelog'

  const REPO = 'joreko/GRUZ'

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

  let unlistenProgress: (() => void) | null = null

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
    unlistenProgress = await onUpdateProgress((p) => {
      progress = p
    })
    try {
      releases = await fetchChangelog()
    } catch {
      error = 'Не удалось загрузить историю изменений'
    } finally {
      loading = false
    }
  })

  onDestroy(() => unlistenProgress?.())

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

  // Иконки для типов изменений
  const typeIcon: Record<string, string> = {
    добавлено: '+',
    исправлено: '✕',
    улучшено: '↑',
    быстрее: '⚡',
  }

  const typeColor: Record<string, string> = {
    добавлено: 'var(--thought-success)',
    исправлено: 'var(--accent)',
    улучшено: 'var(--thought-info)',
    быстрее: 'var(--thought-warning)',
  }

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

  // (+N) для каждого релиза в списке — пересчитывается при смене releases/currentCounter
  const releasesWithNew = $derived(
    filteredReleases.map(rel => ({ rel, newCount: countNew(rel), kind: relKind(rel.tag) }))
  )

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
    <!-- Детальная страница релиза -->
    <div class="detail">
      <button class="back" onclick={() => selected = null}>
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M10 3L5 8l5 5"/></svg>
        Все версии
      </button>

      <div class="detail-header">
        <div class="detail-tag-row">
          <span class="tag">v{displayVersion(selected.tag.replace(/^v/, ''))}</span>
          {#if selected.prerelease}<span class="badge beta">beta</span>{/if}
          {#if relKind(selected.tag) === 'current'}<span class="badge current">установлена</span>{/if}
          {#if relKind(selected.tag) === 'older'}<span class="badge old">старее</span>{/if}
          {#if selectedLines.filter(l => l.isNew).length > 0}<span class="badge new">+{selectedLines.filter(l => l.isNew).length} новых</span>{/if}
        </div>
        <h1 class="detail-title">{selected.name}</h1>
        <span class="detail-date">{formatDate(selected.publishedAt)}</span>
      </div>

      {#if selectedLines.length === 0}
        <p class="empty">Технические улучшения и исправления.</p>
      {:else}
        <ul class="lines">
          {#each selectedLines as line}
            <li class="line" class:line-new={line.isNew}>
              <span class="line-icon" style="color:{typeColor[line.type] ?? 'var(--text-muted)'}">{typeIcon[line.type] ?? '·'}</span>
              <span class="line-scope">{line.scope}</span>
              <span class="line-text">{line.text}</span>
            </li>
          {/each}
        </ul>
      {/if}
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
          <button class="install-error-close" onclick={() => installError = ''} aria-label="Скрыть">✕</button>
        </div>
      {/if}
      <div class="grid">
        {#each releasesWithNew as { rel, newCount, kind }}
          <div class="card" class:card-current={kind === 'current'} onclick={() => selected = rel} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && (selected = rel)}>
            <div class="card-top">
              <span class="card-tag">v{displayVersion(rel.tag.replace(/^v/, ''))}</span>
              {#if rel.prerelease}<span class="badge beta">beta</span>{/if}
              {#if kind === 'current'}<span class="badge current">✓</span>{/if}
              {#if kind === 'older'}<span class="badge old">старее</span>{/if}
              {#if newCount > 0}<span class="badge new">+{newCount}</span>{/if}
            </div>
            <p class="card-date">{formatDate(rel.publishedAt)}</p>
            {#if rel.totalUserLines > 0}
              <p class="card-stat">{rel.totalUserLines} {rel.totalUserLines === 1 ? 'изменение' : rel.totalUserLines < 5 ? 'изменения' : 'изменений'}</p>
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
    padding: 32px 36px;
    min-height: 100%;
  }
  h2 {
    margin: 0;
    font-size: 20px; font-weight: 700; color: var(--text-primary);
  }

  .top-row {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: 24px;
  }

  .tabs {
    display: flex;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 11px;
    padding: 3px;
  }
  .tab {
    height: 30px; padding: 0 14px;
    background: transparent; border: none; border-radius: 8px;
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
    gap: 12px;
  }
  .card {
    text-align: left;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 14px;
    padding: 16px;
    cursor: pointer;
    transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
  }
  .card:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0,0,0,0.35);
    border-color: var(--border-default);
  }
  .card-current {
    border-color: rgba(229,61,70,0.4);
    box-shadow: 0 0 0 1px rgba(229,61,70,0.15);
  }
  .card-top {
    display: flex; align-items: center; gap: 6px; margin-bottom: 6px;
  }
  .card-tag {
    font-size: 13px; font-weight: 700; color: var(--text-primary); font-variant-numeric: tabular-nums;
  }
  .card-date {
    margin: 0 0 6px;
    font-size: 11px; color: var(--text-muted);
  }
  .card-stat {
    margin: 0;
    font-size: 11px; color: var(--text-muted);
  }

  /* Badges */
  .badge {
    font-size: 9px; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase;
    padding: 2px 5px; border-radius: 4px;
  }
  .badge.current { background: rgba(229,61,70,0.15); color: var(--accent); }
  .badge.new { background: rgba(34,197,94,0.12); color: var(--thought-success); }
  .badge.beta { background: rgba(99,102,241,0.15); color: var(--status-downloading); }
  .badge.old { background: rgba(255,255,255,0.06); color: var(--text-muted); }

  .install-btn {
    margin-top: 10px;
    width: 100%;
    padding: 5px 0;
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
    background: transparent;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .install-btn:hover:not(:disabled) {
    background: var(--bg-overlay);
    color: var(--text-primary);
    border-color: var(--border-default);
  }
  .install-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .install-btn.rollback { color: var(--text-muted); }
  .install-btn.rollback:hover:not(:disabled) { color: var(--text-secondary); }

  /* Подтверждение установки */
  .confirm {
    margin-top: 10px;
    display: flex; flex-direction: column; gap: 8px;
  }
  .confirm-q { font-size: 11px; color: var(--text-secondary); }
  .confirm-actions { display: flex; gap: 6px; }
  .confirm-yes, .confirm-no {
    flex: 1; padding: 5px 0; border-radius: 6px; font-size: 11px; font-weight: 600;
    cursor: pointer; transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .confirm-yes {
    border: 1px solid rgba(229,61,70,0.4); background: rgba(229,61,70,0.12); color: var(--accent);
  }
  .confirm-yes:hover { background: rgba(229,61,70,0.2); }
  .confirm-no {
    border: 1px solid var(--border-subtle); background: transparent; color: var(--text-muted);
  }
  .confirm-no:hover { color: var(--text-secondary); border-color: var(--border-default); }

  /* Прогресс скачивания */
  .dl { margin-top: 10px; display: flex; flex-direction: column; gap: 6px; }
  .dl-track {
    height: 4px; border-radius: 2px; background: var(--bg-overlay); overflow: hidden;
  }
  .dl-fill {
    height: 100%; border-radius: 2px;
    background: var(--thought-info);
    transition: width 0.25s ease;
  }
  .dl-fill.indet { animation: indet 1s ease-in-out infinite; }
  @keyframes indet { 0%,100% { opacity: 0.4; } 50% { opacity: 1; } }
  .dl-label {
    font-size: 10px; color: var(--thought-info); font-variant-numeric: tabular-nums;
  }

  /* Детальная страница */
  .detail {
    display: flex; flex-direction: column; gap: 24px; max-width: 680px;
  }
  .back {
    display: inline-flex; align-items: center; gap: 4px;
    background: none; border: none; color: var(--text-muted); font-size: 12px;
    cursor: pointer; padding: 0; width: fit-content;
    transition: color 0.15s;
  }
  .back:hover { color: var(--text-primary); }
  .back svg { width: 14px; height: 14px; }

  .detail-header { display: flex; flex-direction: column; gap: 4px; }
  .detail-tag-row { display: flex; align-items: center; gap: 8px; }
  .tag {
    font-size: 12px; font-weight: 700; color: var(--accent); font-variant-numeric: tabular-nums;
  }
  .detail-title {
    margin: 0;
    font-size: 22px; font-weight: 700; color: var(--text-primary);
  }
  .detail-date { font-size: 12px; color: var(--text-muted); }

  /* Список изменений */
  .lines {
    margin: 0; padding: 0; list-style: none;
    display: flex; flex-direction: column; gap: 2px;
  }
  .line {
    display: flex; align-items: baseline; gap: 8px;
    padding: 6px 10px;
    border-radius: 8px;
    transition: background 0.1s;
  }
  .line:hover { background: var(--bg-elevated); }
  .line-new { background: rgba(34,197,94,0.05); }
  .line-new:hover { background: rgba(34,197,94,0.09); }
  .line-icon {
    font-size: 11px; font-weight: 700; width: 14px; text-align: center; flex-shrink: 0;
  }
  .line-scope {
    font-size: 11px; color: var(--text-muted); flex-shrink: 0;
    font-family: monospace;
  }
  .line-text {
    font-size: 13px; color: var(--text-secondary); line-height: 1.45;
  }

  .empty {
    font-size: 13px; color: var(--text-muted); padding: 24px 0;
  }

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
    background: rgba(229,61,70,0.1);
    border: 1px solid rgba(229,61,70,0.3);
    border-radius: 10px;
    color: var(--accent); font-size: 12px;
  }
  .install-error-close {
    flex-shrink: 0;
    background: none; border: none; color: var(--accent); cursor: pointer;
    font-size: 12px; padding: 0 2px; opacity: 0.7; transition: opacity 0.15s;
  }
  .install-error-close:hover { opacity: 1; }
</style>
