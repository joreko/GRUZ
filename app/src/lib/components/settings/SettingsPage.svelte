<script lang="ts">
  import { store, loadSettings, updateSetting } from '$lib/stores/settings.svelte'
  import type { Settings, ShortcutInfo } from '$lib/bridge/types'
  import { commands } from '$lib/bridge/commands'
  import { onMount } from 'svelte'

  let saveStatus = $state<'saved'|'saving'|'error'|''>('')
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  // ── Ярлыки ───────────────────────────────────────────────────────────────
  type ShortcutLocation = 'start_menu' | 'desktop'

  let shortcuts = $state<ShortcutInfo[]>([])
  let shortcutsLoading = $state(false)
  let shortcutsError = $state('')
  let pendingLoc = $state<ShortcutLocation | null>(null)

  const locationMeta: Record<ShortcutLocation, { label: string; desc: string; icon: 'desktop' | 'start' }> = {
    desktop:    { label: 'Рабочий стол', desc: 'Двойной клик запускает Груз',        icon: 'desktop' },
    start_menu: { label: 'Меню Пуск',   desc: 'Запуск из списка приложений',       icon: 'start' },
  }

  const canonicalLocations: ShortcutLocation[] = ['desktop', 'start_menu']

  // Канонический «Груз.lnk» существует и здоров в этой локации
  const enabledLocs = $derived(
    canonicalLocations.filter(loc => shortcuts.some(s => s.location === loc && !s.is_broken)),
  )
  // Старые / переименованные ярлыки, указывающие на gruz.exe
  const problemShortcuts = $derived(shortcuts.filter(s => s.is_broken))

  const overallStatus = $derived.by(() => {
    if (problemShortcuts.length > 0) return 'warn'
    if (canonicalLocations.every(loc => enabledLocs.includes(loc))) return 'ok'
    return 'idle'
  })

  async function loadShortcuts() {
    shortcutsLoading = true
    shortcutsError = ''
    try {
      shortcuts = await commands.listShortcuts()
    } catch (e) {
      shortcutsError = String(e)
    } finally {
      shortcutsLoading = false
    }
  }

  async function toggleLocation(loc: ShortcutLocation, next: boolean) {
    if (pendingLoc) return
    pendingLoc = loc
    try {
      await commands.setShortcut(loc, next)
      await loadShortcuts()
    } catch (e) {
      shortcutsError = String(e)
    } finally {
      pendingLoc = null
    }
  }

  async function onFix(s: ShortcutInfo) {
    try {
      await commands.fixShortcut(s.path)
      await loadShortcuts()
    } catch (e) {
      shortcutsError = String(e)
    }
  }

  async function onRemove(s: ShortcutInfo) {
    try {
      await commands.removeShortcut(s.path)
      await loadShortcuts()
    } catch (e) {
      shortcutsError = String(e)
    }
  }

  function openFolder(path: string) {
    const parent = path.replace(/[\\/][^\\/]*$/, '')
    commands.openFolder(parent)
  }

  onMount(() => {
    if (!store.settings) loadSettings()
    loadShortcuts()
  })

  async function pickDir() {
    const dir = await commands.pickDirectory()
    if (typeof dir === 'string') {
      await doUpdateSetting('download_dir', dir)
    }
  }

  async function doUpdateSetting(key: string, value: string) {
    saveStatus = 'saving'
    try {
      // Реальное изменение пользователем — шлём silent=false, чтобы
      // оркестратор выдал мысль «обновил настройки».
      await updateSetting(key as keyof Settings, value, false)
      saveStatus = 'saved'
      clearTimeout(saveTimer)
      saveTimer = setTimeout(() => { saveStatus = '' }, 2000)
    } catch {
      saveStatus = 'error'
      clearTimeout(saveTimer)
      saveTimer = setTimeout(() => { saveStatus = '' }, 3000)
    }
  }

  function handleChange(e: Event, key: string) {
    const target = e.currentTarget as HTMLInputElement | HTMLSelectElement
    doUpdateSetting(key, target.value)
  }

  function handleCheck(e: Event, key: string) {
    const target = e.currentTarget as HTMLInputElement
    doUpdateSetting(key, String(target.checked))
  }

  // Для download_dir используем bind:value — обновляется сразу после выбора папки
  let localDownloadDir = $state('')
  $effect(() => {
    if (store.settings) localDownloadDir = store.settings.download_dir
  })
</script>

<div class="page">
  {#snippet locIcon(kind: 'desktop' | 'start')}
    {#if kind === 'desktop'}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>
    {:else}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="8" height="8" rx="1.5"/><rect x="13" y="3" width="8" height="8" rx="1.5"/><rect x="3" y="13" width="8" height="8" rx="1.5"/><rect x="13" y="13" width="8" height="8" rx="1.5"/></svg>
    {/if}
  {/snippet}

  <div class="header-row">
    <h2>Настройки</h2>
    {#if saveStatus === 'saving'}
      <span class="save-status saving">Сохранение...</span>
    {:else if saveStatus === 'saved'}
      <span class="save-status saved">Сохранено</span>
    {:else if saveStatus === 'error'}
      <span class="save-status error">Ошибка сохранения</span>
    {/if}
  </div>

  {#if store.error}
    <div class="error-banner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{store.error}</span>
      <button class="btn-retry" onclick={loadSettings}>Повторить</button>
    </div>
  {:else if store.settings}
    <div class="settings-cols">
    <div class="col-left">
    <section class="section">
      <h3>Загрузка</h3>

      <div class="row">
        <label for="download-dir">Папка загрузки</label>
        <div class="dir-row">
          <input id="download-dir" type="text" bind:value={localDownloadDir} placeholder="Не выбрана" onchange={() => doUpdateSetting('download_dir', localDownloadDir)} />
          <button class="btn-browse" onclick={pickDir}>Выбрать</button>
        </div>
      </div>

      <div class="row">
        <label for="max-concurrent">Параллельных загрузок</label>
        <select
          id="max-concurrent"
          value={String(store.settings.max_concurrent)}
          onchange={e => doUpdateSetting('max_concurrent', (e.currentTarget as HTMLSelectElement).value)}
        >
          {#each [1,2,3,4,5] as n}
            <option value={String(n)}>{n}</option>
          {/each}
        </select>
      </div>

    </section>

    <section class="section">
      <h3>По умолчанию</h3>

      <div class="row">
        <label for="default-format">Формат</label>
        <select
          id="default-format"
          value={store.settings.default_format}
          onchange={e => handleChange(e, 'default_format')}
        >
          <option value="video">Видео</option>
          <option value="audio">Аудио</option>
        </select>
      </div>

      <div class="row">
        <label for="default-quality">Качество</label>
        <select
          id="default-quality"
          value={store.settings.default_quality}
          onchange={e => handleChange(e, 'default_quality')}
        >
          <option value="best">Лучшее</option>
          <option value="1080p">1080p</option>
          <option value="720p">720p</option>
          <option value="480p">480p</option>
        </select>
      </div>

      <div class="row">
        <label for="default-container">Контейнер</label>
        <select
          id="default-container"
          value={store.settings.default_container}
          onchange={e => handleChange(e, 'default_container')}
        >
          <optgroup label="Видео">
            <option value="mp4">MP4</option>
            <option value="webm">WebM</option>
            <option value="mkv">MKV</option>
            <option value="mov">MOV</option>
          </optgroup>
          <optgroup label="Аудио">
            <option value="mp3">MP3</option>
            <option value="m4a">M4A</option>
            <option value="opus">Opus</option>
          </optgroup>
        </select>
      </div>
    </section>

    <section class="section">
      <h3>Дополнительно</h3>

      <div class="row">
        <label for="proxy">Прокси</label>
        <input
          id="proxy"
          type="text"
          value={store.settings.proxy}
          placeholder="http://proxy:port"
          onchange={e => handleChange(e, 'proxy')}
        />
      </div>

      <div class="row">
        <label for="extra-args">Доп. аргументы yt-dlp</label>
        <input
          id="extra-args"
          type="text"
          value={store.settings.ytdlp_extra_args}
          placeholder="--no-check-certificate ..."
          onchange={e => handleChange(e, 'ytdlp_extra_args')}
        />
      </div>

      <div class="row">
        <label for="minimize-tray">Свернуть в трей</label>
        <label class="checkbox-label">
          <input
            id="minimize-tray"
            type="checkbox"
            checked={store.settings.minimize_to_tray}
            onchange={e => handleCheck(e, 'minimize_to_tray')}
          />
          <span class="checkbox-box"></span>
        </label>
      </div>
    </section>
    </div>

    <div class="col-right">
    <section class="section shortcuts">
      <div class="section-head">
        <div class="section-head-text">
          <h3>Ярлыки</h3>
          <p class="section-hint">Запускай Груз откуда удобно — из меню Пуск или с рабочего стола.</p>
        </div>
        <div class="section-head-side">
          {#if overallStatus === 'ok'}
            <span class="status-pill ok">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
              Всё на месте
            </span>
          {:else if overallStatus === 'warn'}
            <span class="status-pill warn">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
              Нужно внимание
            </span>
          {/if}
          <button class="btn-refresh" class:spin={shortcutsLoading} onclick={() => loadShortcuts()} title="Обновить" aria-label="Обновить">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
          </button>
        </div>
      </div>

      {#if shortcutsError}
        <div class="error-banner">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          <span>{shortcutsError}</span>
        </div>
      {/if}

      {#if shortcutsLoading && shortcuts.length === 0}
        <div class="loc-list">
          {#each canonicalLocations as loc (loc)}
            <div class="loc-row skeleton">
              <span class="loc-icon"></span>
              <div class="loc-text">
                <span class="loc-name shimmer"></span>
                <span class="loc-desc shimmer"></span>
              </div>
              <span class="switch off"></span>
            </div>
          {/each}
        </div>
      {:else}
        <ul class="loc-list">
          {#each canonicalLocations as loc (loc)}
            {@const meta = locationMeta[loc]}
            {@const on = enabledLocs.includes(loc)}
            {@const busy = pendingLoc === loc}
            <li class="loc-row">
              <span class="loc-icon">{@render locIcon(meta.icon)}</span>
              <div class="loc-text">
                <span class="loc-name">{meta.label}</span>
                <span class="loc-desc">{meta.desc}</span>
              </div>
              <button
                class="switch"
                class:on
                class:busy
                role="switch"
                aria-checked={on}
                aria-label={meta.label}
                disabled={busy}
                onclick={() => toggleLocation(loc, !on)}
              >
                <span class="switch-thumb"></span>
              </button>
            </li>
          {/each}
        </ul>

        {#if problemShortcuts.length > 0}
          <div class="problems">
            <div class="problems-head">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
              <span>Лишние ярлыки</span>
            </div>
            <p class="problems-hint">Найдены ярлыки со старым именем или дубликаты — их можно починить или удалить.</p>
            <ul class="prob-list">
              {#each problemShortcuts as s (s.path)}
                <li class="prob-row">
                  <span class="prob-icon">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                  </span>
                  <div class="prob-text">
                    <span class="prob-name">{s.name}</span>
                    <span class="prob-target" title={s.target}>{s.target}</span>
                  </div>
                  <div class="prob-actions">
                    <button class="mini-btn fix" onclick={() => onFix(s)}>Исправить</button>
                    <button class="mini-btn ghost" onclick={() => openFolder(s.path)}>Папка</button>
                    <button class="mini-btn danger" onclick={() => onRemove(s)}>Удалить</button>
                  </div>
                </li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if !shortcutsLoading && shortcuts.length === 0}
          <p class="muted empty-note">Ярлыки ещё не созданы. Включи нужные — и Груз появится в системе.</p>
        {/if}
      {/if}
    </section>
    </div>
    </div>
  {:else}
    <p class="muted">Загрузка...</p>
  {/if}
</div>

<style>
  .page {
    padding: var(--space-8) var(--space-9);
    max-width: 960px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .settings-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    align-items: start;
  }

  .col-left,
  .col-right {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    margin: 0;
    font-size: var(--text-xl);
    font-weight: 600;
    color: var(--text-primary);
  }

  .header-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .save-status {
    font-size: var(--text-xs);
    font-weight: 500;
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    transition: opacity var(--transition-slow);
  }
  .save-status.saving {
    color: var(--status-info);
    background: color-mix(in srgb, var(--status-info) 10%, transparent);
  }
  .save-status.saved {
    color: var(--status-success);
    background: color-mix(in srgb, var(--status-success) 10%, transparent);
  }
  .save-status.error {
    color: var(--status-error);
    background: color-mix(in srgb, var(--status-error) 10%, transparent);
  }

  /* Секции — как info-card в DownloadPage */
  .section {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  }

  h3 {
    margin: 0 0 2px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  label {
    font-size: 13px;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  /* Текстовые инпуты */
  input[type="text"] {
    flex: 1;
    padding: 8px 12px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-card);
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    outline: none;
    min-width: 0;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }
  input[type="text"]:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }
  input[type="text"]::placeholder { color: var(--text-muted); }

  /* Селекты */
  select {
    padding: 8px 12px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-card);
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    cursor: pointer;
    outline: none;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }
  select:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-subtle);
  }

  /* Кнопка «Выбрать папку» — стиль info-actions button */
  .dir-row {
    display: flex;
    gap: 8px;
    flex: 1;
  }
  .dir-row input { flex: 1; }
  .btn-browse {
    display: inline-flex; align-items: center;
    height: 34px; padding: 0 14px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: var(--radius-card); color: var(--text-secondary);
    font-size: 12px; font-weight: 500; cursor: pointer; white-space: nowrap;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .btn-browse:hover {
    background: var(--border-subtle);
    color: var(--text-primary);
    border-color: var(--border-strong);
  }

  /* Кастомный чекбокс — квадрат 16px, checked → акцент */
  .checkbox-label {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: pointer;
    flex-shrink: 0;
  }
  .checkbox-label input[type="checkbox"] {
    position: absolute;
    opacity: 0;
    width: 0; height: 0;
  }
  .checkbox-box {
    width: 16px; height: 16px;
    border-radius: var(--radius-sm);
    border: 1.5px solid var(--border-default);
    background: var(--bg-overlay);
    transition: background var(--transition-fast), border-color var(--transition-fast), box-shadow var(--transition-fast);
    display: grid; place-items: center;
  }
  .checkbox-label input:checked + .checkbox-box {
    background: var(--accent);
    border-color: var(--accent);
    box-shadow: 0 0 6px var(--accent-subtle);
  }
  /* Галочка внутри чекбокса */
  .checkbox-label input:checked + .checkbox-box::after {
    content: '';
    display: block;
    width: 9px; height: 5px;
    border-left: 2px solid var(--text-inverse);
    border-bottom: 2px solid var(--text-inverse);
    transform: rotate(-45deg) translateY(-1px);
  }
  .checkbox-label:hover .checkbox-box { border-color: var(--border-strong); }
  .checkbox-label input:focus-visible + .checkbox-box {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .muted {
    color: var(--text-muted);
    font-size: 13px;
  }

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

  /* ─── Ярлыки ──────────────────────────────────────────────────────────── */
  .shortcuts { gap: 16px; }

  .section-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
  }
  .section-head-text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .section-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .section-head-side {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 9px;
    border-radius: var(--radius-full);
    white-space: nowrap;
  }
  .status-pill svg { width: 13px; height: 13px; }
  .status-pill.ok {
    color: var(--status-success);
    background: color-mix(in srgb, var(--status-success) 12%, transparent);
  }
  .status-pill.warn {
    color: var(--status-warning);
    background: color-mix(in srgb, var(--status-warning) 12%, transparent);
  }

  .btn-refresh {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-card);
    border: 1px solid var(--border-default);
    background: var(--bg-overlay);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .btn-refresh svg { width: 15px; height: 15px; }
  .btn-refresh:hover {
    background: var(--border-subtle);
    color: var(--text-primary);
    border-color: var(--border-strong);
  }
  .btn-refresh.spin svg { animation: sc-spin 0.8s linear infinite; }
  @keyframes sc-spin { to { transform: rotate(360deg); } }

  /* Список локаций */
  .loc-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .loc-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }
  .loc-row:hover { border-color: var(--border-default); }

  .loc-icon {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border-radius: var(--radius-md);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
  }
  .loc-icon svg { width: 18px; height: 18px; }

  .loc-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1; }
  .loc-name { font-size: 14px; font-weight: 600; color: var(--text-primary); }
  .loc-desc { font-size: 12px; color: var(--text-muted); }

  /* iOS-переключатель */
  .switch {
    position: relative;
    width: 40px;
    height: 23px;
    flex-shrink: 0;
    border: none;
    padding: 0;
    margin: 0;
    border-radius: var(--radius-full);
    background: var(--border-strong);
    cursor: pointer;
    transition: background var(--transition-default);
  }
  .switch.on { background: var(--accent); }
  .switch.busy { cursor: progress; opacity: 0.65; }
  .switch:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .switch-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 19px;
    height: 19px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 1px 3px rgba(0,0,0,0.4);
    transition: transform var(--transition-default);
  }
  .switch.on .switch-thumb { transform: translateX(17px); }

  /* Скелетон загрузки */
  .loc-row.skeleton { pointer-events: none; }
  .shimmer {
    display: block;
    height: 10px;
    border-radius: var(--radius-sm);
    background: linear-gradient(90deg, var(--bg-elevated) 0%, var(--border-subtle) 50%, var(--bg-elevated) 100%);
    background-size: 200% 100%;
    animation: sc-shimmer 1.2s ease-in-out infinite;
  }
  .loc-name.shimmer { width: 90px; }
  .loc-desc.shimmer { width: 140px; height: 8px; }
  @keyframes sc-shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }

  /* Проблемные ярлыки */
  .problems {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 13px 14px;
    background: color-mix(in srgb, var(--status-warning) 6%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-warning) 28%, transparent);
    border-radius: var(--radius-card);
  }
  .problems-head {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    font-weight: 600;
    color: var(--status-warning);
  }
  .problems-head svg { width: 15px; height: 15px; }
  .problems-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .prob-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .prob-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
  }
  .prob-icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    color: var(--status-warning);
    background: color-mix(in srgb, var(--status-warning) 12%, transparent);
  }
  .prob-icon svg { width: 14px; height: 14px; }
  .prob-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1; }
  .prob-name { font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .prob-target {
    font-size: 11px;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prob-actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }

  .mini-btn {
    height: 28px;
    padding: 0 11px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-default);
    background: var(--bg-elevated);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .mini-btn:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .mini-btn.fix { color: var(--accent); border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
  .mini-btn.fix:hover { background: color-mix(in srgb, var(--accent) 14%, transparent); color: var(--accent-hover); }
  .mini-btn.danger { color: var(--status-error); border-color: color-mix(in srgb, var(--status-error) 35%, transparent); }
  .mini-btn.danger:hover { background: color-mix(in srgb, var(--status-error) 12%, transparent); }

  .empty-note { margin: 2px 0 0; }
</style>
