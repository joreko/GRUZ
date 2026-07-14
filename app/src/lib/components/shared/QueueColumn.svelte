<script lang="ts">
  import { queue } from '$lib/stores/queue.svelte'

  const activeDownloads = $derived(
    queue.tasks.filter(t =>
      t.state === 'downloading' || t.state === 'converting' || t.state === 'waiting'
    )
  )

  const thumbGradients = ['var(--thumb-g1)','var(--thumb-g2)','var(--thumb-g3)','var(--thumb-g4)']
  function taskGradient(id: string) {
    let h = 0
    for (let i = 0; i < id.length; i++) h = ((h << 5) - h + id.charCodeAt(i)) | 0
    return thumbGradients[Math.abs(h) % thumbGradients.length]
  }
</script>

{#if activeDownloads.length > 0}
  <div class="column">
    {#each activeDownloads.slice(0, 8) as task (task.id)}
      <div class="item"
        data-item-type="task"
        data-item-id={task.id}
        data-url={task.url || undefined}
        data-title={task.title || undefined}
        data-task-state={task.state}
        title="{task.title ?? task.url} — {task.progress.toFixed(0)}%"
      >
        <div class="thumb" style="background:{taskGradient(task.id)}">
          {#if task.thumbnail}
            <img src={task.thumbnail} alt="" />
          {/if}
          <div class="bar"><div class="fill" style="width:{task.progress}%"></div></div>
          <div class="dot" class:converting={task.state === 'converting'}></div>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .column {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 0;
    width: 64px;
  }
  .item {
    width: 40px; height: 32px;
    display: grid; place-items: center;
    border-radius: var(--radius-sm); cursor: default;
    transition: background 0.15s;
  }
  .item:hover { background: rgba(0,0,0,0.3); }
  .thumb {
    position: relative;
    width: 36px; height: 26px;
    border-radius: var(--radius-sm); overflow: hidden;
  }
  .thumb img { width: 100%; height: 100%; object-fit: cover; }
  .bar {
    position: absolute; bottom: 0; left: 0; right: 0; height: 3px;
    background: var(--bg-overlay);
  }
  .fill { height: 100%; background: var(--accent); transition: width 0.3s; }
  .dot {
    position: absolute; top: 2px; right: 2px;
    width: 6px; height: 6px; border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 4px color-mix(in srgb, var(--accent) 50%, transparent);
    animation: pulse 1.5s ease-in-out infinite;
  }
  .dot.converting { background: var(--accent-warm); }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
