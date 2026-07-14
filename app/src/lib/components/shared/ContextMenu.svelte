<script lang="ts">
  export interface ContextMenuItem {
    label: string
    icon?: 'copy' | 'paste' | 'select-all' | 'folder' | 'file' | 'delete' | 'nav' | 'link' | 'retry'
    action: () => void
    danger?: boolean
    disabled?: boolean
  }

  export interface ContextMenuGroup {
    items: ContextMenuItem[]
  }

  interface Props {
    x: number
    y: number
    groups: ContextMenuGroup[]
    onclose: () => void
  }

  let { x, y, groups, onclose }: Props = $props()

  // Подстраиваем позицию чтобы не выходить за экран
  let menuEl = $state<HTMLDivElement | null>(null)
  let adjX = $state(0)
  let adjY = $state(0)
  let origin = $state('top left')
  let ready = $state(false)

  $effect(() => {
    if (!menuEl) return
    const cx = x, cy = y
    const r = menuEl.getBoundingClientRect()
    const flipX = cx + r.width  > window.innerWidth
    const flipY = cy + r.height > window.innerHeight
    adjX = flipX ? cx - r.width  : cx
    adjY = flipY ? cy - r.height : cy
    origin = `${flipY ? 'bottom' : 'top'} ${flipX ? 'right' : 'left'}`
    ready = true
  })

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
  }

  $effect(() => {
    // capture: перехватываем клик/mousedown раньше любого элемента
    function onOutside(e: MouseEvent) {
      if (menuEl && !menuEl.contains(e.target as Node)) onclose()
    }
    document.addEventListener('mousedown', onOutside, true)
    return () => document.removeEventListener('mousedown', onOutside, true)
  })
</script>

<svelte:window onkeydown={handleKey} />

<!-- Menu -->
<div
  bind:this={menuEl}
  class="cm"
  style="left:{adjX}px;top:{adjY}px;transform-origin:{origin};visibility:{ready ? 'visible' : 'hidden'}"
  role="menu"
  tabindex="-1"
  aria-label="Контекстное меню"
  oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation() }}
>
  {#each groups as group, gi}
    {#if gi > 0}<div class="cm-sep" role="separator"></div>{/if}
    {#each group.items as item, ii}
      {@const idx = groups.slice(0, gi).reduce((s, g) => s + g.items.length, 0) + ii}
      <button
        class="cm-item"
        class:cm-item-danger={item.danger}
        disabled={item.disabled}
        role="menuitem"
        style="animation-delay:{idx * 0.03}s"
        onclick={() => { item.action(); onclose() }}
      >
        {#if item.icon === 'copy'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="4" y="4" width="8" height="9" rx="1"/><path d="M2 10V3a1 1 0 011-1h7"/>
          </svg>
        {:else if item.icon === 'paste'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="4" width="8" height="9" rx="1"/><path d="M5 4V2.5A0.5.5 0 015.5 2h3a0.5.5 0 01.5.5V4"/>
          </svg>
        {:else if item.icon === 'select-all'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <rect x="1" y="1" width="12" height="12" rx="1" stroke-dasharray="3 1.5"/>
          </svg>
        {:else if item.icon === 'folder'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 3.5A1.5 1.5 0 012.5 2h2.586a1 1 0 01.707.293L7 4h4.5A1.5 1.5 0 0113 5.5v5A1.5 1.5 0 0111.5 12h-9A1.5 1.5 0 011 10.5V3.5z"/>
          </svg>
        {:else if item.icon === 'file'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M8 1H3a1 1 0 00-1 1v10a1 1 0 001 1h8a1 1 0 001-1V5L8 1z"/><path d="M8 1v4h4"/>
          </svg>
        {:else if item.icon === 'delete'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M2 4h10M5 4V2.5a.5.5 0 01.5-.5h3a.5.5 0 01.5.5V4M11 4l-.8 7.5a1 1 0 01-1 .9H4.8a1 1 0 01-1-.9L3 4"/>
          </svg>
        {:else if item.icon === 'nav'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M7 1L13 7 7 13M1 7h12" />
          </svg>
        {:else if item.icon === 'link'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M5.5 8.5a3.5 3.5 0 005 0l2-2a3.5 3.5 0 00-5-5L6 3"/><path d="M8.5 5.5a3.5 3.5 0 00-5 0l-2 2a3.5 3.5 0 005 5L8 11"/>
          </svg>
        {:else if item.icon === 'retry'}
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M1 4a6 6 0 1110.6-1.4"/><polyline points="7,1 11,1 11,5"/>
          </svg>
        {/if}
        <span>{item.label}</span>
      </button>
    {/each}
  {/each}
</div>

<style>
  .cm {
    position: fixed;
    z-index: 9999;
    min-width: 180px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-lg);
    padding: 4px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.6), 0 2px 8px rgba(0,0,0,0.4);
    animation: cm-unfurl 0.2s cubic-bezier(0.22, 1, 0.36, 1) forwards;
  }

  @keyframes cm-unfurl {
    from {
      opacity: 0;
      transform: scaleY(0.4) scaleX(0.96);
      clip-path: inset(0 0 100% 0 round var(--radius-lg));
    }
    to {
      opacity: 1;
      transform: scaleY(1) scaleX(1);
      clip-path: inset(0 0 0% 0 round var(--radius-lg));
    }
  }

  /* Пункты появляются каскадом — delay задаётся через inline style */
  .cm-item {
    animation: cm-item-in 0.2s cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes cm-item-in {
    from { opacity: 0; transform: translateY(-4px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  .cm-sep {
    height: 1px;
    background: var(--border-subtle);
    margin: 3px 0;
  }

  .cm-item {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 6px 10px;
    border-radius: var(--radius-md);
    border: none; background: transparent;
    color: var(--text-secondary);
    font-size: 12px; font-weight: 500;
    cursor: pointer; text-align: left;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .cm-item svg { width: 13px; height: 13px; flex-shrink: 0; opacity: 0.7; }
  .cm-item:hover:not(:disabled) { background: var(--bg-overlay); color: var(--text-primary); }
  .cm-item:hover:not(:disabled) svg { opacity: 1; }
  .cm-item:disabled { opacity: 0.35; cursor: not-allowed; }
  .cm-item.cm-item-danger { color: var(--accent); }
  .cm-item.cm-item-danger:hover:not(:disabled) { background: color-mix(in srgb, var(--accent) 10%, transparent); }
</style>
