<script lang="ts">
  import { store, loadSettings, updateSetting } from '$lib/stores/settings.svelte'
  import type { Settings } from '$lib/bridge/types'
  import { commands } from '$lib/bridge/commands'
  import { onMount } from 'svelte'

  let saveStatus = $state<'saved'|'saving'|'error'|''>('')
  let saveTimer: ReturnType<typeof setTimeout> | undefined

  onMount(() => {
    if (!store.settings) loadSettings()
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
      await updateSetting(key as keyof Settings, value)
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
  {:else}
    <p class="muted">Загрузка...</p>
  {/if}
</div>

<style>
  .page {
    padding: var(--space-8) var(--space-9);
    max-width: 600px;
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
</style>
