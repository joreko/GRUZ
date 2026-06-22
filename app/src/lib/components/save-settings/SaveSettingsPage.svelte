<!-- TODO: страница управления папками и шаблонами сохранения — планируется в будущем.
     Сейчас скрыта из навигации, файлы сохраняются в папку из основных настроек. -->
<script lang="ts">
  import { store, updateSetting } from '$lib/stores/settings.svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import type { Settings } from '$lib/bridge/types'

  const CARDS: {
    key: 'video' | 'audio' | 'playlist' | 'shorts' | 'trimmed'
    label: string
    defaultTpl: string
    extraVars?: string[]
  }[] = [
    { key: 'video',    label: 'Видео',      defaultTpl: '%(title)s.%(ext)s' },
    { key: 'audio',    label: 'Аудио',      defaultTpl: '%(title)s.%(ext)s' },
    { key: 'playlist', label: 'Плейлист',   defaultTpl: '%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s', extraVars: ['%(playlist_title)s', '%(playlist_index)s'] },
    { key: 'shorts',   label: 'Shorts',     defaultTpl: 'Shorts/%(title)s.%(ext)s' },
    { key: 'trimmed',  label: 'Обрезанное', defaultTpl: '%(title)s [trimmed].%(ext)s' },
  ]

  const BASE_VARS = ['%(title)s', '%(uploader)s', '%(date)s', '%(id)s']

  async function pickDir(key: keyof Settings) {
    const dir = await open({ directory: true, multiple: false })
    if (typeof dir === 'string') await updateSetting(key, dir)
  }

  function appendVar(tplKey: keyof Settings, variable: string) {
    if (!store.settings) return
    // Читаем актуальное значение из DOM чтобы не потерять несохранённый ручной ввод
    const input = document.getElementById(`tpl-${tplKey.replace('save_tpl_', '')}`) as HTMLInputElement | null
    const current = input?.value ?? (store.settings as unknown as Record<string, string>)[tplKey] ?? ''
    const newVal = current.endsWith('.%(ext)s')
      ? current.slice(0, -8) + variable + '.%(ext)s'
      : current + variable
    if (input) input.value = newVal
    updateSetting(tplKey, newVal)
  }

  function preview(dirKey: keyof Settings, tplKey: keyof Settings): string {
    if (!store.settings) return ''
    const dir = (store.settings as unknown as Record<string, string>)[dirKey] || store.settings.download_dir || '~/Downloads'
    const tpl = (store.settings as unknown as Record<string, string>)[tplKey] || ''
    return `${dir}/${tpl}`
  }
</script>

<div class="page">
  {#if store.settings}
    {#each CARDS as card}
      {@const dirKey = `save_dir_${card.key}` as keyof Settings}
      {@const tplKey = `save_tpl_${card.key}` as keyof Settings}
      {@const dirVal = (store.settings as unknown as Record<string, string>)[dirKey] ?? ''}
      {@const tplVal = (store.settings as unknown as Record<string, string>)[tplKey] ?? ''}
      {@const vars = [...BASE_VARS, ...(card.extraVars ?? [])]}

      <div class="card">
        <div class="card-header">
          <div class="card-icon">
            {#if card.key === 'video'}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>
            {:else if card.key === 'audio'}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
            {:else if card.key === 'playlist'}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
            {:else if card.key === 'shorts'}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="2" width="14" height="20" rx="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>
            {:else}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>
            {/if}
          </div>
          <span class="card-title">{card.label}</span>
        </div>

        <div class="fields">
          <div class="field-row">
            <span class="field-label">Папка</span>
            <div class="dir-row">
              <span class="dir-path" class:muted={!dirVal}>{dirVal || 'По умолчанию'}</span>
              <button class="btn-browse" onclick={() => pickDir(dirKey)}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                Обзор
              </button>
            </div>
          </div>

          <div class="field-row">
            <label class="field-label" for="tpl-{card.key}">Имя файла</label>
            <input
              id="tpl-{card.key}"
              class="tpl-input"
              type="text"
              value={tplVal}
              placeholder={card.defaultTpl}
              onblur={e => updateSetting(tplKey, e.currentTarget.value)}
              spellcheck={false}
              autocomplete="off"
            />
            <div class="vars">
              {#each vars as v}
                <button class="chip" onclick={() => appendVar(tplKey, v)}>{v}</button>
              {/each}
            </div>
          </div>
        </div>

        <div class="preview-row">
          <span class="preview-label">Результат</span>
          <span class="preview-path">{preview(dirKey, tplKey) || `~/Downloads/${card.defaultTpl}`}</span>
        </div>
      </div>
    {/each}
  {:else}
    {#each Array(5) as _}
      <div class="card skeleton">
        <div class="sk-head shimmer"></div>
        <div class="sk-line shimmer"></div>
        <div class="sk-line shimmer"></div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .page {
    padding: 32px 36px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 100%;
  }

  .card {
    background: #1e1e1e;
    border: 1px solid #2e2e2e;
    border-radius: 14px;
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  }

  .card-header { display: flex; align-items: center; gap: 10px; }
  .card-icon {
    width: 28px; height: 28px;
    display: grid; place-items: center;
    background: rgba(0,0,0,0.35);
    border: 1px solid transparent;
    border-radius: 7px;
    color: #ff3d3d;
    flex-shrink: 0;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 1px 0 rgba(120,120,120,0.5);
  }
  .card-icon svg { width: 13px; height: 13px; }
  .card-title { font-size: 13px; font-weight: 600; color: #f0f0f0; letter-spacing: 0.01em; }

  .fields { display: flex; flex-direction: column; gap: 12px; }
  .field-row { display: flex; flex-direction: column; gap: 6px; }
  .field-label { font-size: 10px; font-weight: 600; color: #555; text-transform: uppercase; letter-spacing: 0.06em; }

  .dir-row {
    display: flex; align-items: center; gap: 8px;
    background: #252525; border: 1px solid #333;
    border-radius: 10px; padding: 6px 6px 6px 12px;
  }
  .dir-path { flex: 1; min-width: 0; font-size: 12px; color: #ccc; font-family: monospace; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dir-path.muted { color: #555; font-family: inherit; font-style: italic; }

  .btn-browse {
    display: flex; align-items: center; gap: 5px;
    padding: 5px 11px;
    background: #2a2a2a; border: 1px solid #3a3a3a;
    border-radius: 7px; color: #888;
    font-size: 11px; font-weight: 500; cursor: pointer;
    white-space: nowrap; flex-shrink: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    box-shadow: inset 0 1px 0 rgba(120,120,120,0.15);
  }
  .btn-browse svg { width: 11px; height: 11px; }
  .btn-browse:hover { background: #333; color: #f0f0f0; border-color: #444; }
  .btn-browse:active { transform: scale(0.97); }

  .tpl-input {
    width: 100%; box-sizing: border-box;
    padding: 8px 12px;
    background: #252525; border: 1px solid #333;
    border-radius: 10px; color: #f0f0f0;
    font-size: 12px; font-family: monospace;
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
  }
  .tpl-input:focus { border-color: #ff3d3d; box-shadow: 0 0 0 3px rgba(255,61,61,0.12); }
  .tpl-input::placeholder { color: #444; }

  .vars { display: flex; flex-wrap: wrap; gap: 5px; }
  .chip {
    padding: 3px 8px;
    background: #252525; border: 1px solid #2e2e2e;
    border-radius: 20px; color: #666;
    font-size: 11px; font-family: monospace;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .chip:hover { background: rgba(255,61,61,0.08); border-color: rgba(255,61,61,0.4); color: #ff3d3d; }
  .chip:active { transform: scale(0.95); }

  .preview-row {
    display: flex; align-items: baseline; gap: 8px;
    padding: 8px 12px;
    background: #161616; border: 1px solid #272727;
    border-radius: 8px;
  }
  .preview-label { font-size: 10px; font-weight: 600; color: #444; text-transform: uppercase; letter-spacing: 0.06em; flex-shrink: 0; }
  .preview-path { font-size: 11px; font-family: monospace; color: #666; word-break: break-all; line-height: 1.5; }

  .shimmer { position: relative; overflow: hidden; background: #252525; }
  .shimmer::after {
    content: ''; position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.035) 45%, rgba(255,255,255,0.055) 50%, rgba(255,255,255,0.035) 55%, transparent 100%);
    animation: shimmer 2.2s infinite;
  }
  @keyframes shimmer { 0% { transform: translateX(-100%); } 100% { transform: translateX(100%); } }
  .sk-head { height: 28px; width: 120px; border-radius: 7px; }
  .sk-line { height: 36px; width: 100%; border-radius: 10px; }

  ::-webkit-scrollbar { width: 2px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }
</style>