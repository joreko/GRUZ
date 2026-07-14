<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app'
  import { onMount, onDestroy } from 'svelte'
  import { slide } from 'svelte/transition'
  import { formatDate, formatBytes } from '$lib/utils/format'
  import { commands } from '$lib/bridge/commands'
  import { pushToast } from '$lib/stores/toast.svelte'
  import { onUpdateProgress, type UpdateProgress } from '$lib/bridge/events'
  import {
    fetchChangelog,
    fetchRateLimitRemaining,
    fetchLatestTag,
    resolveSetupAsset,
    displayVersion,
    versionCounter,
    compareVersions,
    type ReleaseChangelog,
    REPO,
  } from '$lib/utils/changelog'
  import RepoCard from './RepoCard.svelte'

  let releases = $state<ReleaseChangelog[]>([])
  let selected = $state<ReleaseChangelog | null>(null)
  let loading = $state(true)
  let error = $state('')
  let currentVersion = $state('')
  let installing = $state<string | null>(null)  // tag который устанавливается
  let confirmTag = $state<string | null>(null)   // tag ожидающий подтверждения
  let progress = $state<UpdateProgress | null>(null)
  let installError = $state('')                   // ошибка скачивания/установки
  let filter = $state<'stable' | 'beta' | null>(null)
  let showTechLines = $state(false)
  let collapsedGroups = $state(new Set<string>())
  let checking = $state(false)                 // идёт проверка обновлений (ручная или авто)
  let rateLeft = $state<number | null>(null)   // оставшиеся запросы GitHub API за час
  let lastChecked = $state<number | null>(null) // когда последний раз реально опрашивали
  let nextCheckAt = $state<number | null>(null) // время следующей авто-проверки
  let now = $state(Date.now())                  // тикер для обратного отсчёта

  const AUTO_INTERVAL_MS = 120_000              // лёгкая авто-проверка раз в две минуты
  let autoTimer: ReturnType<typeof setTimeout> | null = null
  let tickTimer: ReturnType<typeof setInterval> | null = null

  let unlistenProgressPromise: Promise<() => void> | null = null

  // По умолчанию бета если нет стабильных
  $effect(() => {
    if (!loading && releases.length > 0 && releases.every(r => r.prerelease)) filter = 'beta'
  })

  const filteredReleases = $derived(
    filter === 'stable' ? releases.filter(r => !r.prerelease)
    : filter === 'beta'  ? releases.filter(r => r.prerelease)
    : releases
  )

  // Счётчик текущей установленной версии (= patch = номер коммита)
  const currentCounter = $derived(versionCounter(currentVersion))

  // Обратный отсчёт до следующей авто-проверки (тикает раз в секунду через `now`)
  const countdownLabel = $derived.by(() => {
    if (checking) return 'идёт запрос к GitHub…'
    if (nextCheckAt === null) return 'авто каждые 2 минуты'
    const secs = Math.max(0, Math.round((nextCheckAt - now) / 1000))
    if (secs <= 0) return 'скоро…'
    const m = Math.floor(secs / 60)
    const s = secs % 60
    if (m > 0) return `${m} мин ${s} с`
    return `${secs} с`
  })

  onMount(async () => {
    currentVersion = await getVersion()
    // Сохраняем promise — unlisten вызываем независимо от момента unmount
    unlistenProgressPromise = onUpdateProgress((p) => {
      progress = p
    })
    // Лимит GitHub API — показываем рядом с кнопкой проверки
    fetchRateLimitRemaining().then((n) => { rateLeft = n }).catch(() => {})
    try {
      releases = await fetchChangelog()
    } catch {
      error = 'Не удалось загрузить историю изменений (проверьте доступ к GitHub)'
    } finally {
      loading = false
    }
    startAutoCheck()
  })

  // Лёгкая авто-проверка: раз в минуту один дешёвый запрос к последнему релизу.
  // Полный пересбор списка — только когда реально появилась новая версия,
  // иначе жжём лимит GitHub (60/час) впустую.
  function scheduleNextCheck(delay: number) {
    nextCheckAt = Date.now() + delay
    if (autoTimer) clearTimeout(autoTimer)
    autoTimer = setTimeout(runAutoCheck, delay)
  }

  function startAutoCheck() {
    tickTimer = setInterval(() => { now = Date.now() }, 1000)
    scheduleNextCheck(AUTO_INTERVAL_MS)
  }

  async function runAutoCheck() {
    await pollUpdates()
    scheduleNextCheck(AUTO_INTERVAL_MS)
  }

  // Фоновый опрос: бережёт лимит (не тратим последние 2 запроса), при новой
  // версии делает полный fetchChangelog(true) один раз и уведомляет.
  async function pollUpdates() {
    if (checking) return
    if (rateLeft !== null && rateLeft <= 2) return
    checking = true
    const t0 = Date.now()
    try {
      const tag = await fetchLatestTag()
      const currentNewest = releases[0]?.tag
      if (
        tag &&
        tag !== currentNewest &&
        compareVersions(tag.replace(/^v/, ''), currentVersion) > 0
      ) {
        releases = await fetchChangelog(true)
        const newest = releases[0]
        if (newest && relKind(newest.tag) === 'newer') {
          pushToast({
            type: 'info',
            title: 'Доступно обновление',
            message: `v${displayVersion(newest.tag.replace(/^v/, ''))}`,
          })
        }
      }
      lastChecked = Date.now()
    } catch {
      // фоновую проверку не дёргаем ошибками — тихо ждём следующего цикла
    } finally {
      // Гарантируем видимую длительность анимации даже при мгновенном ответе
      const elapsed = Date.now() - t0
      if (elapsed < 700) await new Promise(r => setTimeout(r, 700 - elapsed))
      checking = false
      fetchRateLimitRemaining().then((n) => { rateLeft = n }).catch(() => {})
    }
  }

  onDestroy(async () => {
    if (unlistenProgressPromise) (await unlistenProgressPromise)()
    if (autoTimer) clearTimeout(autoTimer)
    if (tickTimer) clearInterval(tickTimer)
  })

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
    installing = tag
    installError = ''
    progress = { downloaded: 0, total: null, pct: null, done: false }
    try {
      // Реальный URL ассета, а не хардкод GRUZ_<ver>_Setup.exe (иначе 404 на бетах/переименованиях)
      const url = await resolveSetupAsset(tag)
      if (!url) {
        installing = null
        progress = null
        installError = `Для v${version} нет файла установщика на GitHub`
        return
      }
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
    исправлено: 'var(--accent-warm)',
    улучшено:   'var(--thought-info)',
    быстрее:    'var(--thought-warning)',
    удалено:    'var(--accent)',
  }

  const typeLabel: Record<string, string> = {
    добавлено:  'Добавлено',
    исправлено: 'Исправлено',
    улучшено:   'Улучшено',
    быстрее:    'Быстрее',
    удалено:    'Удалено',
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

  const TYPE_ORDER = ['добавлено', 'улучшено', 'быстрее', 'исправлено', 'удалено']

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
    return rel.commits.reduce((s, c) => c.counter > currentCounter ? s + c.lines.length + c.techLines.length : s, 0)
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
  const newLinesCount = $derived(
    selectedLines.filter(l => l.isNew).length +
    (selected ? selected.commits.reduce((s, c) => c.counter > currentCounter ? s + c.techLines.length : s, 0) : 0)
  )

  // Статистика по scope для детального вида
  const selectedScopeStats = $derived.by(() => {
    if (!selected) return []
    const map = new Map<string, number>()
    for (const commit of selected.commits) {
      for (const line of [...commit.lines, ...commit.techLines]) {
        if (!line.scope || HIDDEN_SCOPES.has(line.scope)) continue
        map.set(line.scope, (map.get(line.scope) ?? 0) + 1)
      }
    }
    return [...map.entries()]
      .sort((a, b) => b[1] - a[1])
  })

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
      <div class="detail-sidebar">
        <button class="back" onclick={() => selected = null}>
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M10 3L5 8l5 5"/></svg>
          Все версии
        </button>
        <aside class="detail-meta">
          <div class="meta-ver">v{ver}</div>
          <div class="meta-date">{formatDate(selected.publishedAt)}</div>
          <div class="meta-badges">
            {#if selected.prerelease}<span class="badge beta">бета</span>{/if}
            {#if kind === 'current'}<span class="badge current">установлена</span>{/if}
            {#if newLinesCount > 0}<span class="badge new">+{newLinesCount} новых</span>{/if}
          </div>
          <div class="meta-bottom">
            {#if selectedScopeStats.length > 0}
              <div class="meta-scopes">
                {#each selectedScopeStats as [scope, count]}
                  <div class="meta-scope-row">
                    <span class="meta-scope-label" style="color:{getScopeColor(scope)}">{getScopeLabel(scope)}</span>
                    <span class="meta-scope-line"></span>
                    <span class="meta-scope-count">{count}</span>
                  </div>
                {/each}
              </div>
            {/if}
            {#if kind !== 'current'}
              <div class="meta-install">
                <div class="meta-divider"></div>
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
          </div>
        </aside>
      </div>

        <!-- Правая колонка: список изменений -->
      <div class="detail-changes">
          {#if selected.description?.trim() && selected.description.trim() !== 'Технические улучшения и исправления.'}
            <p class="meta-desc">{selected.description.trim()}</p>
          {/if}
          {#if selectedLines.length === 0}
            <p class="empty">Технические улучшения и исправления.</p>
          {:else}
            {@const hiddenCount = selectedTechLines.length}
            <div class="groups">
              {#each selectedGroups as group, gi}
                {@const offset = selectedGroups.slice(0, gi).reduce((s, g) => s + g.lines.length, 0)}
                {@const collapsed = collapsedGroups.has(group.type)}
                <div class="group" style="--gc:{typeColor[group.type]}">
                  <button class="group-header" onclick={() => {
                    const s = new Set(collapsedGroups)
                    if (s.has(group.type)) s.delete(group.type); else s.add(group.type)
                    collapsedGroups = s
                  }}>
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
                    <svg class="group-chevron" class:collapsed viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="4,5 7,8 10,5"/></svg>
                  </button>
                  {#if !collapsed}
                    <ul class="lines" transition:slide={{ duration: 180 }}>
                      {#each group.lines as line, li}
                        <li class="line" class:line-new={line.isNew} data-line-text={line.text}>
                          <span class="line-num">{offset + li + 1}</span>
                          {#if line.scope && !HIDDEN_SCOPES.has(line.scope)}
                            <span class="line-scope" style="--sc:{getScopeColor(line.scope)}">{getScopeLabel(line.scope)}</span>
                          {/if}
                          <span class="line-text">{line.text}</span>
                        </li>
                      {/each}
                    </ul>
                  {/if}
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
                        <li class="line" data-line-text={line.text}>
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

  {:else}
    <div class="versions-scroll">
    <!-- Список релизов -->
    <div class="top-row">
      <div class="top-left">
        <h2>Обновления</h2>
        <RepoCard />
        <div class="check-card" class:is-checking={checking}>
          <span class="check-icon" class:spin={checking} aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 12a9 9 0 1 1-2.64-6.36"/>
              <polyline points="21 3 21 9 15 9"/>
            </svg>
          </span>
          <span class="check-info">
            <span class="check-label">{checking ? 'Проверяем…' : 'Проверка обновлений'}</span>
            <span class="check-meta">{countdownLabel}</span>
          </span>
          <span class="check-sweep" aria-hidden="true"></span>
        </div>
      </div>
      <div class="tabs">
        <button class="tab" class:active={filter === 'stable'} onclick={() => filter = filter === 'stable' ? null : 'stable'}>Стабильные</button>
        <button class="tab" class:active={filter === 'beta'}   onclick={() => filter = filter === 'beta'   ? null : 'beta'  }>Бета</button>
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
      <div class="releases">
        {#each releasesWithNew as { rel, newCount, kind }, i}
          {@const isHero = i === 0}
          <div
            class="card"
            class:card-hero={isHero}
            class:card-current={kind === 'current'}
            data-release-tag={rel.tag}
            role="button"
            tabindex="0"
            onclick={() => selected = rel}
            onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (selected = rel)}
          >
            <div class="card-head">
              <span class="card-tag">v{displayVersion(rel.tag.replace(/^v/, ''))}</span>
              {#if newCount > 0}<span class="card-new">+{newCount}</span>{/if}
              <div class="card-badges">
                {#if rel.prerelease}<span class="badge beta">бета</span>{/if}
                {#if kind === 'current'}<span class="badge current">текущая</span>{/if}
              </div>
            </div>
            <div class="card-meta">
              <span class="card-date">{formatDate(rel.publishedAt)}</span>
            </div>
            {#if !isHero && rel.totalUserLines > 0}
              <p class="card-stat">{rel.totalUserLines} {pluralLines(rel.totalUserLines)}</p>
            {/if}
            {#if isHero}
              {@const allLines = rel.commits.flatMap(c => c.lines)}
              {@const allTechLines = rel.commits.flatMap(c => c.techLines)}
              {@const typeCounts = ['добавлено','исправлено','улучшено','быстрее','удалено'].map(t => ({ type: t, count: allLines.filter(l => l.type === t).length })).filter(x => x.count > 0)}
              {@const techCount = allTechLines.length}
              {@const desc = rel.description?.trim()}
              {#if desc && desc !== 'Технические улучшения и исправления.'}<p class="hero-desc">{desc}</p>{/if}
              <div class="hero-body">
                <div class="hero-left">
                  {#if kind === 'current'}
                    <span class="hero-current">установлена</span>
                  {:else}
                    <button
                      class="hero-btn"
                      disabled={installing !== null}
                      onclick={(e) => { e.stopPropagation(); startInstall(rel.tag) }}
                    >{kind === 'older' ? 'откатиться' : 'обновить'} до v{displayVersion(rel.tag.replace(/^v/, ''))}</button>
                  {/if}
                </div>
                {#if typeCounts.length > 0}
                  <div class="hero-stats">
                    {#each typeCounts as { type, count }}
                      <div class="hero-stat" style="--lc:{typeColor[type]}">
                        <span class="hero-stat-n">{count}</span>
                        <span class="hero-stat-label">{typeLabel[type]}</span>
                      </div>
                    {/each}
                    {#if techCount > 0}
                      <div class="hero-stat" style="--lc:var(--text-muted)">
                        <span class="hero-stat-n">{techCount}</span>
                        <span class="hero-stat-label">прочее</span>
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    </div><!-- versions-scroll -->
  {/if}
</div>

<style>
  .page {
    padding: 0;
    height: 100%;
    overflow: hidden;
    box-sizing: border-box;
  }
  .page::-webkit-scrollbar { width: 0; }
  .page { scrollbar-width: none; }
  /* Список версий — скроллится сам */
  .versions-scroll {
    height: 100%;
    overflow-y: auto;
    padding: var(--space-8) var(--space-9);
    box-sizing: border-box;
    scrollbar-width: none;
  }
  .versions-scroll::-webkit-scrollbar { width: 0; }

  h2 {
    margin: 0;
    font-size: var(--text-xl); font-weight: 700; color: var(--text-primary);
  }

  .top-row {
    display: flex; align-items: center; justify-content: space-between;
    margin-bottom: var(--space-6);
  }
  .top-left { display: flex; align-items: center; gap: var(--space-4); }

  .check-card {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: 40px;
    padding: 0 var(--space-4) 0 6px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    overflow: hidden;
    transition: border-color var(--transition-default), box-shadow var(--transition-default);
  }
  .check-card.is-checking {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 35%, transparent), 0 0 18px color-mix(in srgb, var(--accent) 22%, transparent);
    animation: check-pulse 1.4s ease-in-out infinite;
  }
  @keyframes check-pulse {
    0%, 100% { box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 30%, transparent), 0 0 12px color-mix(in srgb, var(--accent) 14%, transparent); }
    50%      { box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent) 50%, transparent), 0 0 22px color-mix(in srgb, var(--accent) 30%, transparent); }
  }
  .check-icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    flex-shrink: 0;
    border-radius: var(--radius-full);
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    transition: color var(--transition-default), background var(--transition-default), border-color var(--transition-default);
  }
  .check-icon svg { width: 15px; height: 15px; }
  .check-card.is-checking .check-icon {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .check-icon.spin { animation: check-spin 0.9s linear infinite; }
  @keyframes check-spin { to { transform: rotate(360deg); } }
  /* Бегущий блик по карточке во время проверки */
  .check-sweep {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background: linear-gradient(
      100deg,
      transparent 30%,
      color-mix(in srgb, var(--accent) 16%, transparent) 50%,
      transparent 70%
    );
    background-size: 220% 100%;
    transform: translateX(-60%);
  }
  .check-card.is-checking .check-sweep {
    opacity: 1;
    animation: check-sweep 1.1s ease-in-out infinite;
  }
  @keyframes check-sweep {
    0%   { background-position: 180% 0; }
    100% { background-position: -80% 0; }
  }
  .check-info {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    text-align: left;
    z-index: 1;
  }
  .check-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    transition: color var(--transition-default);
  }
  .check-card.is-checking .check-label { color: var(--accent); }
  .check-meta {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
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

  /* Список релизов: герой на всю ширину, остальные — сетка */
  .releases { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: var(--space-3); }

  .card {
    text-align: left; width: 100%; font: inherit;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    padding: var(--space-3) var(--space-4);
    cursor: pointer;
    display: flex; flex-direction: column; gap: var(--space-1);
    transition: border-color 120ms ease, background 120ms ease, box-shadow 120ms ease;
  }
  .card:hover { background: var(--bg-overlay); border-color: var(--border-default); box-shadow: 0 2px 8px rgba(0,0,0,0.3); }
  .card-current { border-color: color-mix(in srgb, var(--accent) 25%, transparent); }

  /* Герой — первая новая версия, растянуть на всю строку сетки */
  .card-hero {
    grid-column: 1 / -1;
    flex-direction: column; align-items: stretch; gap: var(--space-3);
    padding: var(--space-5);
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 8%, var(--bg-elevated)) 0%, var(--bg-elevated) 60%);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    margin-bottom: var(--space-1);
  }
  .card-hero:hover {
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 12%, var(--bg-overlay)) 0%, var(--bg-overlay) 60%);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }

  /* Шапка */
  .card-head { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); flex: none; }
  .card-hero .card-head { margin-bottom: 0; }

  .card-tag { font-size: 13px; font-weight: 600; color: var(--text-primary); font-variant-numeric: tabular-nums; }
  .card-hero .card-tag { font-size: 26px; font-weight: 700; letter-spacing: -0.01em; }

  .card-badges { display: flex; gap: 4px; margin-left: auto; }

  /* Мета строка */
  .card-meta { display: flex; align-items: center; gap: var(--space-2); }
  .card-hero .card-meta { flex: none; }
  .card-date { font-size: 11px; color: var(--text-muted); }
  .card-new { font-size: 11px; font-weight: 600; color: var(--thought-success); }
  .card-hero .card-new { font-size: 15px; }

  /* Описание (только у героя) */
  .card-stat { margin: 0; font-size: 12px; color: var(--text-secondary); }

  /* Тело героя: лево (текст + кнопка) и право (статистика) */
  .hero-desc {
    margin: var(--space-1) 0 var(--space-2);
    font-family: var(--font-sans);
    font-size: var(--text-xs);
    font-weight: 400;
    color: var(--text-secondary);
    line-height: 1.6;
    letter-spacing: -0.01em;
    max-width: 75%;
  }
  .hero-body { display: flex; align-items: center; justify-content: space-between; gap: var(--space-6); }
  .hero-left { display: flex; flex-direction: column; gap: var(--space-3); }
  .hero-stats {
    display: flex; gap: var(--space-4); align-items: flex-end; flex-shrink: 0;
    padding-left: var(--space-4);
    border-left: 1px solid var(--border-subtle);
  }
  .hero-stat { display: flex; flex-direction: column; align-items: flex-end; gap: var(--space-1); }
  .hero-stat-n { font-size: 22px; font-weight: 700; color: var(--lc); line-height: 1; font-variant-numeric: tabular-nums; min-width: 2ch; text-align: right; }
  .hero-stat-label { font-size: 10px; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: var(--text-secondary); }

  /* Кнопка героя */
  .hero-btn {
    align-self: flex-start;
    padding: 8px 20px;
    background: var(--accent); color: #fff;
    border: 1px solid var(--accent);
    border-radius: var(--radius-md);
    font-size: 13px; font-weight: 600; cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
    margin-top: var(--space-1);
  }
  .hero-btn:hover { background: var(--accent-hover); border-color: var(--accent-hover); }
  .hero-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .hero-current { font-size: 12px; color: var(--text-muted); font-weight: 500; margin-top: var(--space-1); }

  /* Badges */
  .badge { font-size: 10px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; padding: 2px 6px; border-radius: var(--radius-sm); white-space: nowrap; border: 1px solid transparent; }
  .badge.current { background: transparent; color: var(--accent); border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
  .badge.new     { background: color-mix(in srgb, var(--thought-success) 10%, transparent); color: var(--thought-success); border-color: color-mix(in srgb, var(--thought-success) 30%, transparent); }
  .badge.beta {
    background: transparent;
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent-hover);
    letter-spacing: 0.08em;
  }
  .card-hero .badge.beta {
    font-size: 11px;
    padding: 4px 9px;
  }

  /* Детальная страница */
  .detail {
    display: grid;
    grid-template-columns: 200px 1fr;
    height: 100%;
    overflow: hidden;
  }

  .detail-sidebar {
    display: flex; flex-direction: column;
    padding: var(--space-8) var(--space-6) var(--space-8) var(--space-9);
    border-right: 1px solid var(--border-subtle);
    gap: var(--space-5);
    overflow: hidden;
  }

  .back {
    display: inline-flex; align-items: center; gap: var(--space-1);
    background: none; border: none; color: var(--text-muted); font-size: 12px;
    cursor: pointer; padding: 4px 8px 4px 4px; margin-left: -4px;
    border-radius: var(--radius-md); flex-shrink: 0;
    transition: color var(--transition-fast), background var(--transition-fast);
  }
  .back:hover { color: var(--text-primary); background: var(--bg-elevated); }
  .back svg { width: 14px; height: 14px; }

  /* Левая колонка мета */
  .detail-meta {
    display: flex; flex-direction: column; gap: var(--space-3);
    flex: 1;
    min-height: 0;
  }
  .meta-bottom {
    margin-top: auto;
    max-height: 100%;
    min-height: 0;
    display: flex; flex-direction: column; gap: var(--space-3);
  }

  .detail-changes {
    overflow-y: auto;
    padding: var(--space-8) var(--space-9) var(--space-8) var(--space-8);
    scrollbar-width: none;
  }
  .detail-changes::-webkit-scrollbar { width: 0; }
  .meta-ver {
    font-size: var(--text-xl); font-weight: 700; color: var(--text-primary);
    letter-spacing: -0.01em; font-variant-numeric: tabular-nums;
  }
  .meta-date { font-size: 11px; color: var(--text-muted); }
  .meta-badges { display: flex; flex-direction: column; gap: var(--space-1); align-items: flex-start; }
  .meta-install { margin-top: auto; }
  .meta-divider { margin-bottom: var(--space-3); }
  .meta-scopes {
    display: flex; flex-direction: column; gap: var(--space-1);
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
    scrollbar-width: thin;
    scrollbar-color: var(--border-strong) transparent;
  }
  .meta-scopes::-webkit-scrollbar { width: 6px; }
  .meta-scopes::-webkit-scrollbar-track { background: transparent; }
  .meta-scopes::-webkit-scrollbar-thumb { background: var(--border-strong); border-radius: 3px; }
  .meta-scopes::-webkit-scrollbar-thumb:hover { background: var(--border-default); }
  .meta-scope-row {
    display: flex; align-items: center; gap: var(--space-1);
    padding: 3px var(--space-1);
    border-radius: var(--radius-sm);
  }
  .meta-scope-label {
    font-size: 11px;
    position: relative; z-index: 1; white-space: nowrap;
  }
  .meta-scope-line {
    flex: 1;
    border-bottom: 1px dotted var(--border-subtle);
    margin-bottom: 0;
    align-self: flex-end;
    min-width: var(--space-2);
  }
  .meta-scope-count {
    font-size: var(--text-sm); font-weight: 700;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums; letter-spacing: -0.01em;
    position: relative; z-index: 1;
  }
  .meta-desc {
    margin: 0 0 var(--space-4);
    padding-bottom: var(--space-4);
    border-bottom: 1px solid var(--border-subtle);
    font-family: var(--font-sans);
    font-size: 12px;
    font-style: normal;
    font-weight: 400;
    color: var(--text-primary);
    line-height: 1.55;
    opacity: 0.75;
    text-align: center;
  }

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
    width: 100%; background: none; border-top: none; border-left: none; border-right: none;
    cursor: pointer; text-align: left;
    transition: opacity var(--transition-fast);
  }
  .group-header:hover { opacity: 0.8; }
  .group-chevron {
    width: 14px; height: 14px; flex-shrink: 0; margin-left: auto;
    color: var(--text-muted);
    transition: transform var(--transition-fast);
  }
  .group-chevron.collapsed { transform: rotate(-90deg); }
  .group-icon {
    width: 16px; height: 16px; display: grid; place-items: center; flex-shrink: 0;
    color: var(--gc);
  }
  .group-icon svg { width: 12px; height: 12px; }
  .group-label {
    font-size: 11px; font-weight: 600; letter-spacing: 0.04em; text-transform: uppercase; color: var(--gc);
  }
  .group-count {
    font-size: 11px; font-weight: 600;
    color: var(--text-muted);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    padding: 1px 6px;
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
