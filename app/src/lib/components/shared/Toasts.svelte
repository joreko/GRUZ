<script lang="ts">
  import { fly } from 'svelte/transition'
  import { toasts, dismissToast } from '$lib/stores/toast.svelte'

  // Тосты прижаты к нижнему краю, над FAB
  let el = $state<HTMLElement | null>(null)
</script>

<div class="toast-wrap" role="region" aria-label="Уведомления" bind:this={el}>
  {#each toasts as t (t.id)}
    <div class="toast" class:toast-success={t.type === 'success'} class:toast-error={t.type === 'error'} class:toast-info={t.type === 'info'} class:toast-warning={t.type === 'warning'} transition:fly={{ y: 24, duration: 220 }}>
      {#if t.type}
        <span class="toast-icon" aria-hidden="true">
          {#if t.type === 'success'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
          {:else if t.type === 'error'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          {:else if t.type === 'warning'}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
          {/if}
        </span>
      {/if}
      <div class="toast-body">
        {#if t.title}<span class="toast-title">{t.title}</span>{/if}
        <span class="toast-msg">{t.message}</span>
      </div>
      {#if t.actionLabel}
        <button class="toast-action" onclick={() => { t.onAction?.(); dismissToast(t.id) }}>
          {t.actionLabel}
        </button>
      {/if}
      <button class="toast-close" aria-label="Закрыть" onclick={() => dismissToast(t.id)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-wrap {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    z-index: 500;
    pointer-events: none;
    max-width: calc(100vw - 32px);
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px 10px 14px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-left-width: 3px;
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.55);
    pointer-events: auto;
    min-width: 260px;
    max-width: 100%;
  }
  .toast-success { border-left-color: var(--status-success); }
  .toast-error   { border-left-color: var(--status-error); }
  .toast-info    { border-left-color: var(--status-info); }
  .toast-warning { border-left-color: var(--status-warning); }
  .toast-icon {
    flex-shrink: 0;
    width: 18px; height: 18px;
    display: grid; place-items: center;
  }
  .toast-icon svg { width: 18px; height: 18px; }
  .toast-success .toast-icon { color: var(--status-success); }
  .toast-error   .toast-icon { color: var(--status-error); }
  .toast-info    .toast-icon { color: var(--status-info); }
  .toast-warning .toast-icon { color: var(--status-warning); }
  .toast-body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .toast-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.3;
  }
  .toast-msg {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    line-height: 1.35;
  }
  .toast-action {
    flex-shrink: 0;
    height: 28px;
    padding: 0 12px;
    background: transparent;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    color: var(--accent);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast);
  }
  .toast-action:hover { background: color-mix(in srgb, var(--accent) 12%, transparent); border-color: color-mix(in srgb, var(--accent) 45%, transparent); }
  .toast-close {
    flex-shrink: 0;
    width: 24px; height: 24px;
    display: flex; align-items: center; justify-content: center;
    background: transparent; border: none; color: var(--text-muted); cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .toast-close:hover { color: var(--text-primary); background: var(--bg-overlay); }
  .toast-close svg { width: 14px; height: 14px; }
</style>
