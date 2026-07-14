<script lang="ts">
  import { commands } from '$lib/bridge/commands'
  import { REPO } from '$lib/utils/changelog'

  const repoUrl = `https://github.com/${REPO}`
  const releasesUrl = `${repoUrl}/releases`

  function openRepo() {
    commands.openUrl(repoUrl).catch(() => {})
  }

  function openReleases(e: MouseEvent) {
    e.stopPropagation()
    commands.openUrl(releasesUrl).catch(() => {})
  }
</script>

<button class="repo-card" onclick={openRepo} aria-label="Открыть репозиторий {REPO} на GitHub">
  <span class="repo-icon" aria-hidden="true">
    <svg viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 .5C5.37.5 0 5.78 0 12.29c0 5.21 3.44 9.63 8.21 11.19.6.11.82-.25.82-.56 0-.28-.01-1.02-.02-2-3.34.71-4.04-1.58-4.04-1.58-.55-1.36-1.34-1.72-1.34-1.72-1.09-.73.08-.72.08-.72 1.2.08 1.84 1.21 1.84 1.21 1.07 1.79 2.81 1.27 3.5.97.11-.76.42-1.27.76-1.56-2.67-.3-5.47-1.29-5.47-5.74 0-1.27.46-2.31 1.21-3.13-.12-.29-.53-1.46.12-3.05 0 0 .98-.31 3.21 1.19a11.3 11.3 0 0 1 5.84 0c2.23-1.5 3.21-1.19 3.21-1.19.65 1.59.24 2.76.12 3.05.76.82 1.21 1.86 1.21 3.13 0 4.46-2.81 5.44-5.49 5.73.43.36.81 1.08.81 2.18 0 1.57-.01 2.84-.01 3.23 0 .31.21.68.83.56A12.01 12.01 0 0 0 24 12.29C24 5.78 18.63.5 12 .5z"/>
    </svg>
  </span>
  <span class="repo-info">
    <span class="repo-name">{REPO}</span>
    <span class="repo-meta">Репозиторий на GitHub</span>
  </span>
  <span class="repo-releases" role="presentation" onclick={openReleases} aria-label="Открыть релизы">
    Релизы
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M7 17L17 7M17 7H8M17 7v9"/>
    </svg>
  </span>
</button>

<style>
  .repo-card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 6px 8px 6px 10px;
    height: 38px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card);
    cursor: pointer;
    transition: border-color var(--transition-fast), background var(--transition-fast), box-shadow var(--transition-fast);
  }
  .repo-card:hover {
    border-color: var(--border-strong);
    background: var(--bg-overlay);
    box-shadow: var(--shadow-card);
  }
  .repo-icon {
    display: grid;
    place-items: center;
    color: var(--text-primary);
  }
  .repo-icon svg { width: 20px; height: 20px; }
  .repo-info {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
    min-width: 0;
  }
  .repo-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
  }
  .repo-meta {
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .repo-releases {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 4px;
    padding: 5px 9px;
    border-radius: var(--radius-md);
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-subtle);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .repo-releases:hover {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
    color: var(--accent-hover);
  }
  .repo-releases svg { width: 12px; height: 12px; }
</style>
