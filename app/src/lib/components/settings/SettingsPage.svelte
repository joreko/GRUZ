<script lang="ts">
  import { store, updateSetting } from '$lib/stores/settings.svelte'
  import { open } from '@tauri-apps/plugin-dialog'

  async function pickDir() {
    const dir = await open({ directory: true, multiple: false })
    if (typeof dir === 'string') {
      await updateSetting('download_dir', dir)
    }
  }
</script>

<div class="page">
  <h2>Настройки</h2>

  {#if store.settings}
    <section class="section">
      <h3>Загрузка</h3>

      <div class="row">
        <label for="download-dir">Папка загрузки</label>
        <div class="dir-row">
          <input id="download-dir" type="text" value={store.settings.download_dir} readonly placeholder="Не выбрана" />
          <button class="btn-browse" onclick={pickDir}>Выбрать</button>
        </div>
      </div>

      <div class="row">
        <label for="max-concurrent">Параллельных загрузок</label>
        <select
          id="max-concurrent"
          value={String(store.settings.max_concurrent)}
          onchange={e => updateSetting('max_concurrent', e.currentTarget.value)}
        >
          {#each [1,2,3,4,5] as n}
            <option value={String(n)}>{n}</option>
          {/each}
        </select>
      </div>

      <div class="row">
        <label for="ytdlp-autoupdate">Авто-обновление yt-dlp</label>
        <label class="checkbox-label">
          <input
            id="ytdlp-autoupdate"
            type="checkbox"
            checked={store.settings.ytdlp_auto_update}
            onchange={e => updateSetting('ytdlp_auto_update', String(e.currentTarget.checked))}
          />
          <span class="checkbox-box"></span>
        </label>
      </div>
    </section>

    <section class="section">
      <h3>По умолчанию</h3>

      <div class="row">
        <label for="default-format">Формат</label>
        <select
          id="default-format"
          value={store.settings.default_format}
          onchange={e => updateSetting('default_format', e.currentTarget.value)}
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
          onchange={e => updateSetting('default_quality', e.currentTarget.value)}
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
          onchange={e => updateSetting('default_container', e.currentTarget.value)}
        >
          <option value="mp4">MP4</option>
          <option value="webm">WebM</option>
          <option value="mp3">MP3</option>
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
          onchange={e => updateSetting('proxy', e.currentTarget.value)}
        />
      </div>

      <div class="row">
        <label for="extra-args">Доп. аргументы yt-dlp</label>
        <input
          id="extra-args"
          type="text"
          value={store.settings.ytdlp_extra_args}
          placeholder="--no-check-certificate ..."
          onchange={e => updateSetting('ytdlp_extra_args', e.currentTarget.value)}
        />
      </div>

      <div class="row">
        <label for="minimize-tray">Свернуть в трей</label>
        <label class="checkbox-label">
          <input
            id="minimize-tray"
            type="checkbox"
            checked={store.settings.minimize_to_tray}
            onchange={e => updateSetting('minimize_to_tray', String(e.currentTarget.checked))}
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
    padding: 32px 36px;
    max-width: 600px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
  }

  /* Секции — как info-card в DownloadPage */
  .section {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 14px;
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
    border-radius: 10px;
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
    border-radius: 10px;
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
    border-radius: 10px; color: var(--text-secondary);
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
    border-radius: 4px;
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
</style>
