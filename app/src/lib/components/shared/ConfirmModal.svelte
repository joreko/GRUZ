<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { dialog, dismissDialog } from '$lib/stores/confirm.svelte'

  let inputEl = $state<HTMLInputElement | null>(null)

  const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches

  function confirm() {
    const d = dialog.current
    if (!d) return dismissDialog()
    if (d.type === 'prompt') {
      const v = inputEl?.value ?? d.defaultValue
      dialog.current = null
      d.resolve(v)
    } else {
      dialog.current = null
      d.resolve(true)
    }
  }

  function cancel() {
    const d = dialog.current
    if (!d) return dismissDialog()
    if (d.type === 'prompt') {
      dialog.current = null
      d.resolve(null)
    } else {
      dialog.current = null
      d.resolve(false)
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') cancel()
    if (e.key === 'Enter' && d?.type === 'confirm') confirm()
  }

  const d = $derived(dialog.current)
  $effect(() => {
    if (d) {
      window.addEventListener('keydown', onKey)
      return () => window.removeEventListener('keydown', onKey)
    }
  })
</script>

{#if d}
  <div
    class="cm-scrim"
    role="alertdialog"
    aria-modal="true"
    aria-label={d.title}
    transition:fade={{ duration: reduceMotion ? 0 : 160 }}
  >
    <div
      class="cm-panel"
      transition:scale={{ duration: reduceMotion ? 0 : 220, start: 0.96, opacity: 0 }}
    >
      <header class="cm-head">
        <div class="cm-icon">
          {#if d.type === 'confirm'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
            </svg>
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>
            </svg>
          {/if}
        </div>
        <div class="cm-head-txt">
          <div class="cm-title">{d.title}</div>
          <p class="cm-msg">{d.message}</p>
        </div>
        <button class="cm-close" aria-label="Закрыть" onclick={cancel}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </header>

      {#if d.type === 'prompt'}
        <div class="cm-body">
          <input
            class="cm-input"
            type="text"
            bind:value={d.defaultValue}
            bind:this={inputEl}
            onkeydown={(e) => { if (e.key === 'Enter') confirm() }}
          />
        </div>
      {/if}

      <footer class="cm-foot">
        <div class="cm-foot-right">
          <button class="cm-btn cm-btn-ghost" onclick={cancel}>Отмена</button>
          <button class="cm-btn cm-btn-primary" onclick={confirm}>
            {d.type === 'confirm' ? 'Да, сделать' : 'Ок'}
          </button>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .cm-scrim {
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
  .cm-panel {
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
  }
  .cm-head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5) var(--space-5) 0;
  }
  .cm-icon {
    width: 40px;
    height: 40px;
    flex-shrink: 0;
    border-radius: var(--radius-lg);
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
  }
  .cm-icon svg { width: 20px; height: 20px; }
  .cm-head-txt { flex: 1; min-width: 0; }
  .cm-title {
    font-size: var(--text-base);
    font-weight: 700;
    color: var(--text-primary);
    line-height: 1.3;
  }
  .cm-msg {
    margin: 4px 0 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .cm-close {
    flex-shrink: 0;
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: transparent; border: none;
    color: var(--text-muted); cursor: pointer;
    border-radius: var(--radius-md);
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .cm-close:hover { color: var(--text-primary); background: var(--bg-overlay); }
  .cm-close svg { width: 16px; height: 16px; }

  .cm-body {
    padding: var(--space-4) var(--space-5) 0;
  }
  .cm-input {
    width: 100%;
    height: 36px;
    padding: 0 var(--space-3);
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: var(--text-sm);
    font-family: inherit;
    outline: none;
    transition: border-color var(--transition-fast);
    box-sizing: border-box;
  }
  .cm-input:focus { border-color: var(--accent); }
  .cm-input::placeholder { color: var(--text-muted); }

  .cm-foot {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-5) var(--space-5);
  }
  .cm-foot-right { display: flex; gap: var(--space-2); }

  .cm-btn {
    height: 34px;
    padding: 0 var(--space-4);
    display: inline-flex;
    align-items: center;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
  }
  .cm-btn-ghost {
    background: transparent;
    border-color: var(--border-default);
    color: var(--text-secondary);
  }
  .cm-btn-ghost:hover { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }
  .cm-btn-primary {
    background: var(--accent);
    color: #fff;
  }
  .cm-btn-primary:hover { background: var(--accent-hover); }
  .cm-btn-primary:active { background: var(--accent-active); }
</style>
