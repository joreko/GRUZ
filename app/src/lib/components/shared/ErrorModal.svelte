<script lang="ts">
  import { fade, scale } from 'svelte/transition'
  import { crash, dismissCrash, copyCrashReport } from '$lib/stores/crash.svelte'
  import { commands } from '$lib/bridge/commands'

  const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)').matches
  const SUPPORT_URL = 'https://t.me/GRUZ_official'

  let copied = $state(false)
  let expanded = $state(false)
  let copyTimer: ReturnType<typeof setTimeout> | null = null

  const report = $derived(crash.report)
  const previewLines = $derived(report ? report.text.split('\n').slice(0, 3) : [])
  const hiddenCount = $derived(
    report ? Math.max(0, report.text.split('\n').length - 3) : 0,
  )

  async function copy() {
    const ok = await copyCrashReport()
    if (ok) {
      copied = true
      if (copyTimer) clearTimeout(copyTimer)
      copyTimer = setTimeout(() => (copied = false), 2000)
    }
  }

  function writeDev() {
    commands.openUrl(SUPPORT_URL).catch(() => {})
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') dismissCrash()
  }

  // Escape закрывает модалку, пока она открыта
  $effect(() => {
    if (report) {
      window.addEventListener('keydown', onKey)
      return () => window.removeEventListener('keydown', onKey)
    }
  })
</script>

{#if report}
  <div
    class="em-scrim"
    role="alertdialog"
    aria-modal="true"
    aria-label="Необработанная ошибка"
    transition:fade={{ duration: reduceMotion ? 0 : 160 }}
  >
    <div
      class="em-panel"
      transition:scale={{ duration: reduceMotion ? 0 : 220, start: 0.96, opacity: 0 }}
    >
      <header class="em-head">
        <span class="em-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
            <line x1="12" y1="9" x2="12" y2="13"/>
            <line x1="12" y1="17" x2="12.01" y2="17"/>
          </svg>
        </span>
        <div class="em-head-txt">
          <div class="em-title">Упс, что-то пошло не так</div>
          <div class="em-sub">
            {new Date(report.time).toLocaleString()} · {report.type}
          </div>
        </div>
        <button class="em-close" aria-label="Закрыть" onclick={dismissCrash}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      </header>

      <div class="em-body">
        <p class="em-msg">
          Приложение поймало необработанную ошибку. Детали ниже можно скопировать
          и отправить разработчику — так мы быстрее её поправим.
        </p>

        <pre class="em-pre">{expanded ? report.text : previewLines.join('\n')}{!expanded && hiddenCount > 0 ? '\n…' : ''}</pre>

        {#if hiddenCount > 0}
          <button class="em-toggle" onclick={() => (expanded = !expanded)}>
            {expanded ? 'Свернуть' : `Показать ещё ${hiddenCount} строк(и)`}
          </button>
        {/if}
      </div>

      <footer class="em-foot">
        <button class="btn btn-ghost" onclick={writeDev}>
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M13.9 2.3L1.6 7c-.8.3-.8.8-.1 1l3.1 1 1.2 3.6c.2.5.3.7.7.7.3 0 .5-.1.7-.3l1.5-1.4 3.1 2.3c.6.3 1 .1 1.1-.5l2-9.4c.2-.8-.3-1.2-.9-.7z"/>
          </svg>
          Написать разработчику
        </button>
        <div class="em-foot-right">
          <button class="btn btn-ghost" onclick={dismissCrash}>Закрыть</button>
          <button class="btn btn-primary" onclick={copy}>
            {#if copied}
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <polyline points="13 4 6 11 3 8"/>
              </svg>
              Скопировано
            {:else}
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/>
                <path d="M3.5 10.5V3a1 1 0 0 1 1-1h6.5"/>
              </svg>
              Копировать отчёт
            {/if}
          </button>
        </div>
      </footer>
    </div>
  </div>
{/if}

<style>
  .em-scrim {
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
  .em-scrim::before {
    content: '';
    position: absolute;
    inset: 0;
    background: radial-gradient(60% 50% at 50% 28%,
      color-mix(in srgb, var(--status-error) 14%, transparent), transparent 70%);
    pointer-events: none;
  }

  .em-panel {
    position: relative;
    width: 100%;
    max-width: 540px;
    max-height: calc(100vh - var(--space-12));
    display: flex;
    flex-direction: column;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-xl);
    box-shadow:
      var(--shadow-panel),
      0 0 0 1px color-mix(in srgb, var(--status-error) 18%, transparent);
    overflow: hidden;
  }

  .em-head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-5) var(--space-5) var(--space-3);
  }
  .em-icon {
    width: 42px;
    height: 42px;
    flex-shrink: 0;
    border-radius: var(--radius-lg);
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--status-error) 14%, transparent);
    color: var(--status-error);
    box-shadow: 0 0 24px color-mix(in srgb, var(--status-error) 28%, transparent);
  }
  .em-icon svg { width: 22px; height: 22px; }
  .em-head-txt { flex: 1; min-width: 0; padding-top: 2px; }
  .em-title {
    font-size: var(--text-lg);
    font-weight: 700;
    color: var(--text-primary);
    line-height: 1.3;
  }
  .em-sub {
    margin-top: 2px;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .em-close {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    border-radius: var(--radius-md);
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .em-close:hover { color: var(--text-primary); background: var(--bg-overlay); }
  .em-close svg { width: 16px; height: 16px; }

  .em-body {
    padding: 0 var(--space-5) var(--space-2);
    overflow-y: auto;
  }
  .em-msg {
    margin: 0 0 var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .em-pre {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.55;
    color: var(--text-tertiary);
    background: var(--bg-base);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: var(--space-3);
    max-height: 220px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .em-toggle {
    margin-top: var(--space-2);
    padding: 0;
    background: transparent;
    border: none;
    color: var(--accent);
    font-size: var(--text-xs);
    font-weight: 600;
    cursor: pointer;
  }
  .em-toggle:hover { color: var(--accent-hover); }

  .em-foot {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-5) var(--space-5);
    border-top: 1px solid var(--border-subtle);
  }
  .em-foot-right { display: flex; gap: var(--space-2); margin-left: auto; }

  .btn {
    height: 34px;
    padding: 0 var(--space-4);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
  }
  .btn svg { width: 15px; height: 15px; }
  .btn-ghost {
    background: transparent;
    border-color: var(--border-default);
    color: var(--text-secondary);
  }
  .btn-ghost:hover { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }
  .btn-primary {
    background: var(--accent);
    color: #fff;
  }
  .btn-primary:hover { background: var(--accent-hover); }
  .btn-primary:active { background: var(--accent-active); }
</style>
