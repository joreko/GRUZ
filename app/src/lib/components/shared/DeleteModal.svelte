<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { tick } from 'svelte'
  import { delDialog, dismissDeleteDialog } from '$lib/stores/delete.svelte'
  import type { DeleteMode } from '$lib/stores/delete.svelte'

  const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches

  const d = $derived(delDialog.current)
  let selected = $state<DeleteMode>('history_only')
  let panel = $state<HTMLDivElement | null>(null)

  function select(mode: DeleteMode) { selected = mode }
  function doDelete() {
    const cur = d
    if (!cur) return
    const mode = selected
    delDialog.current = null
    cur.resolve(mode)
  }
  function cancel() { dismissDeleteDialog() }

  function onKey(e: KeyboardEvent) {
    if (!d) return
    if (e.key === 'Escape') { e.preventDefault(); cancel() }
    else if (e.key === 'Enter') { e.preventDefault(); doDelete() }
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (!d.hasFile) return
      selected = selected === 'history_only' ? 'with_file' : 'history_only'
    } else if (e.key === '1') { e.preventDefault(); select('history_only') }
    else if (e.key === '2' && d.hasFile) { e.preventDefault(); select('with_file') }
  }

  $effect(() => {
    if (d) {
      selected = 'history_only'
      tick().then(() => panel?.focus())
      window.addEventListener('keydown', onKey, true)
      return () => window.removeEventListener('keydown', onKey, true)
    }
  })
</script>

{#if d}
  <div
    class="dm-scrim"
    role="alertdialog"
    aria-modal="true"
    aria-label="Подтверждение удаления"
    transition:fade={{ duration: reduceMotion ? 0 : 160 }}
  >
    <div
      class="dm-panel"
      bind:this={panel}
      tabindex="-1"
      transition:scale={{ duration: reduceMotion ? 0 : 220, start: 0.96, opacity: 0 }}
    >
      <header class="dm-head">
        <div class="dm-icon" class:warn={!d.hasFile}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/>
          </svg>
        </div>
        <div class="dm-head-txt">
          <div class="dm-title">
            {#if d.context === 'single'}Удаление записи
            {:else if d.context === 'selection'}Удаление выбранного ({d.count})
            {:else}Очистить всё ({d.count})
            {/if}
          </div>
          <p class="dm-sub">Что сделать с {d.count === 1 ? 'записью' : 'записями'}?</p>
        </div>
        <button class="dm-close" aria-label="Закрыть" onclick={cancel}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </header>

      <div class="dm-body">
        <label
          class="dm-opt"
          class:active={selected === 'history_only'}
        >
          <input
            type="radio"
            name="dm-mode"
            value="history_only"
            checked={selected === 'history_only'}
            onchange={() => select('history_only')}
          />
          <div class="dm-opt-text">
            <span class="dm-opt-title">
              Только из истории
              <kbd>1</kbd>
            </span>
            <span class="dm-opt-sub">
              {#if d.context === 'clear-all'}Очистить историю загрузок.{:else}Убрать запись из истории.{/if}
              Файлы останутся на диске.
            </span>
          </div>
        </label>

        {#if d.hasFile}
          <label
            class="dm-opt danger"
            class:active={selected === 'with_file'}
          >
            <input
              type="radio"
              name="dm-mode"
              value="with_file"
              checked={selected === 'with_file'}
              onchange={() => select('with_file')}
            />
            <div class="dm-opt-text">
              <span class="dm-opt-title">
                Удалить с диска
                <kbd>2</kbd>
              </span>
              <span class="dm-opt-sub">
                {#if d.context === 'clear-all'}Очистить историю и стереть файлы на диске.{:else}Файл будет стёрт вместе с записью.{/if}
                Действие необратимо.
              </span>
            </div>
          </label>
        {/if}
      </div>

      <footer class="dm-foot">
        <button class="dm-btn dm-btn-ghost" onclick={cancel}>Отмена</button>
        <button
          class="dm-btn"
          class:dm-btn-danger={selected === 'with_file'}
          class:dm-btn-primary={selected === 'history_only'}
          onclick={doDelete}
        >
          {#if selected === 'with_file'}
            {d.context === 'clear-all' ? 'Очистить с диском' : 'Удалить с диска'}
          {:else}
            {d.context === 'clear-all' ? 'Очистить' : 'Убрать из истории'}
          {/if}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .dm-scrim {
    position: fixed;
    inset: 0;
    z-index: 100000;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--space-6);
    background: var(--scrim-overlay);
    backdrop-filter: blur(10px) saturate(120%);
  }
  .dm-panel {
    position: relative;
    width: 100%;
    max-width: 440px;
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-panel);
    overflow: hidden;
    outline: none;
  }
  .dm-head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5) var(--space-5) 0;
  }
  .dm-icon {
    width: 40px; height: 40px;
    flex-shrink: 0;
    border-radius: var(--radius-lg);
    display: grid; place-items: center;
    background: color-mix(in srgb, var(--status-error) 14%, transparent);
    color: var(--status-error);
  }
  .dm-icon.warn {
    background: color-mix(in srgb, var(--accent-warm) 14%, transparent);
    color: var(--accent-warm);
  }
  .dm-icon svg { width: 20px; height: 20px; }
  .dm-head-txt { flex: 1; min-width: 0; }
  .dm-title {
    font-size: var(--text-base);
    font-weight: 700;
    color: var(--text-primary);
    line-height: 1.3;
  }
  .dm-sub {
    margin: 2px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .dm-close {
    flex-shrink: 0;
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: transparent; border: none;
    color: var(--text-muted); cursor: pointer;
    border-radius: var(--radius-md);
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .dm-close:hover { color: var(--text-primary); background: var(--bg-overlay); }
  .dm-close svg { width: 16px; height: 16px; }

  .dm-body {
    padding: var(--space-4) var(--space-5);
    display: flex; flex-direction: column;
    gap: var(--space-2);
  }
  .dm-opt {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }
  .dm-opt:hover { background: var(--bg-overlay); border-color: var(--border-default); }
  .dm-opt.active { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 6%, transparent); }
  .dm-opt.danger.active { border-color: var(--status-error); background: color-mix(in srgb, var(--status-error) 6%, transparent); }
  .dm-opt input { margin-top: 3px; accent-color: var(--accent); }
  .dm-opt.danger input { accent-color: var(--status-error); }
  .dm-opt-text { display: flex; flex-direction: column; gap: 2px; }
  .dm-opt-title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
  }
  .dm-opt.danger.active .dm-opt-title { color: var(--status-error); }
  .dm-opt-sub { font-size: var(--text-xs); color: var(--text-muted); line-height: 1.4; }
  kbd {
    font-size: 10px;
    font-family: inherit;
    font-weight: 600;
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    color: var(--text-muted);
    line-height: 1.4;
  }

  .dm-foot {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: 0 var(--space-5) var(--space-5);
  }
  .dm-btn {
    height: 34px;
    padding: 0 var(--space-4);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast), opacity var(--transition-fast);
  }
  .dm-btn-ghost {
    background: transparent;
    border-color: var(--border-default);
    color: var(--text-secondary);
  }
  .dm-btn-ghost:hover { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }
  .dm-btn-primary {
    background: var(--accent);
    color: #fff;
    min-width: 120px;
  }
  .dm-btn-primary:hover { opacity: 0.88; }
  .dm-btn-danger {
    background: var(--status-error);
    color: #fff;
    min-width: 120px;
  }
  .dm-btn-danger:hover { opacity: 0.88; }
</style>
