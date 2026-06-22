<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app'
  import { onMount } from 'svelte'
  import { formatDate } from '$lib/utils/format'
  import {
    fetchChangelog,
    getBaselineCounter,
    displayVersion,
    type ReleaseChangelog,
  } from '$lib/utils/changelog'

  let releases = $state<ReleaseChangelog[]>([])
  let selected = $state<ReleaseChangelog | null>(null)
  let loading = $state(true)
  let error = $state('')
  let currentVersion = $state('')
  let baselineCounter = $state<number | null>(null)

  onMount(async () => {
    currentVersion = await getVersion()
    baselineCounter = getBaselineCounter()
    try {
      releases = await fetchChangelog()
    } catch {
      error = 'Не удалось загрузить историю изменений'
    } finally {
      loading = false
    }
  })

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

  function isCurrent(tag: string) {
    return tag.replace(/^v/, '') === currentVersion
  }

  function countNew(rel: ReleaseChangelog): number {
    if (baselineCounter === null) return 0
    return rel.commits.reduce((s, c) => c.counter > baselineCounter! ? s + c.lines.length : s, 0)
  }

  // Строки выбранного релиза — пересчитываем только при смене selected/baseline
  const selectedLines = $derived(
    selected
      ? selected.commits.flatMap(commit =>
          commit.lines.map(line => ({
            ...line,
            counter: commit.counter,
            isNew: baselineCounter !== null && commit.counter > baselineCounter,
          }))
        )
      : []
  )

  // (+N) для каждого релиза в списке — пересчитывается при смене releases/baseline
  const releasesWithNew = $derived(
    releases.map(rel => ({ rel, newCount: countNew(rel) }))
  )
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
          {#if isCurrent(selected.tag)}<span class="badge current">установлена</span>{/if}
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
    <h2>Обновления</h2>

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
      <div class="grid">
        {#each releasesWithNew as { rel, newCount }}
          <button class="card" class:card-current={isCurrent(rel.tag)} onclick={() => selected = rel}>
            <div class="card-top">
              <span class="card-tag">v{displayVersion(rel.tag.replace(/^v/, ''))}</span>
              {#if isCurrent(rel.tag)}<span class="badge current">✓</span>{/if}
              {#if newCount > 0}<span class="badge new">+{newCount}</span>{/if}
            </div>
            <p class="card-name">{rel.name}</p>
            <p class="card-date">{formatDate(rel.publishedAt)}</p>
            {#if rel.totalUserLines > 0}
              <p class="card-stat">{rel.totalUserLines} {rel.totalUserLines === 1 ? 'изменение' : rel.totalUserLines < 5 ? 'изменения' : 'изменений'}</p>
            {:else}
              <p class="card-stat">технические улучшения</p>
            {/if}
          </button>
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
    margin: 0 0 24px;
    font-size: 20px; font-weight: 700; color: var(--text-primary);
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
  .card-name {
    margin: 0 0 4px;
    font-size: 12px; font-weight: 500; color: var(--text-secondary);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
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
</style>
