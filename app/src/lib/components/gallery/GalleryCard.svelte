<script lang="ts">
  import { tooltip } from '$lib/utils/tooltip'
  import { formatDuration, formatParams } from '$lib/utils/format'
  import { lazyImage } from '$lib/utils/lazyImage'
  import { assetUrl } from '$lib/utils/media.svelte'
  import { aspectStore } from '$lib/gallery/aspectStore.svelte'
  import type { CardModel } from './types'

  interface Props {
    entry: CardModel
    selected: boolean
    showParams: boolean
    favorite: boolean
    localThumbnail: string | null
    fill?: boolean
    delay?: number
    onAspect?: (id: string, width: number, height: number) => void
    onToggleSelect: (id: string, shift: boolean) => void
    onOpen: (id: string, origin?: HTMLElement) => void
    onToggleFavorite: (id: string) => void
  }

  let {
    entry,
    selected,
    showParams,
    favorite,
    localThumbnail,
    fill = false,
    delay = 0,
    onAspect,
    onToggleSelect,
    onOpen,
    onToggleFavorite,
  }: Props = $props()

  let loaded = $state(false)
  let failed = $state(false)
  let useRetry = $state(false)

  const localUrl = $derived(assetUrl(localThumbnail))
  const localSrc = $derived(localUrl ? `${localUrl}?v=${entry.width ?? 0}x${entry.height ?? 0}` : null)

  // Приоритет: YouTube custom thumbnail → локальный ffmpeg-кадр
  const thumbSrc = $derived(
    !useRetry
      ? (entry.thumbnail || localSrc || null)
      : (localSrc || entry.thumbnail || null),
  )
  const hasThumb = $derived(!!thumbSrc)

  // Фон-плейсхолдер: переиспользуем градиенты из theme.css (--thumb-g1..g4)
  const phGradient = $derived(`background: var(--thumb-g${(hashId(entry.id) % 4) + 1})`)

  const stateLabel: Record<string, string> = {
    waiting: 'Ожидание',
    downloading: 'Загрузка',
    converting: 'Конвертация',
    paused: 'Пауза',
    completed: 'Готово',
    failed: 'Ошибка',
    cancelled: 'Отменено',
  }

  function stateText(e: CardModel): string {
    if (e.kind === 'task' && (e.state === 'downloading' || e.state === 'converting')) {
      if (e.streamType === 'video') return 'Видео'
      if (e.streamType === 'audio') return 'Аудио'
      if (e.streamType === 'converting') return 'Слияние'
      if (e.state === 'downloading') return e.isAudio ? 'Аудио' : 'Видео'
    }
    return stateLabel[e.state ?? ''] ?? e.state ?? ''
  }

  const isActive = $derived(entry.kind === 'task' && entry.state !== 'completed' && entry.state !== 'cancelled')
  const showProgress = $derived(
    entry.kind === 'task' &&
      (entry.state === 'downloading' || entry.state === 'converting'),
  )
  const isFailed = $derived(entry.kind === 'task' && entry.state === 'failed')

  const initial = $derived(
    (entry.title ?? entry.channel ?? entry.platform ?? '?').trim().charAt(0).toUpperCase(),
  )

  function hashId(s: string): number {
    let h = 0
    for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0
    return Math.abs(h)
  }

  function handleCardClick(e: MouseEvent) {
    if (e.shiftKey) {
      onToggleSelect(entry.id, true)
      return
    }
    onOpen(entry.id, e.currentTarget as HTMLElement)
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onOpen(entry.id)
    }
  }

  // Hover video preview
  let isHovering = $state(false)
  let videoReady = $state(false)
  let videoEl: HTMLVideoElement | null = $state(null)

  const hasVideo = $derived(entry.kind === 'history' && !!entry.file_path)
  const videoUrl = $derived(hasVideo ? assetUrl(entry.file_path!) : null)

  $effect(() => {
    if (!videoEl || !hasVideo) return
    if (isHovering) {
      videoReady = false
      if (!videoEl.src && videoUrl) videoEl.src = videoUrl
      videoEl.currentTime = (entry.duration ?? 0) * 0.25
      videoEl.play().catch(() => {})
    } else {
      videoReady = false
      videoEl.pause()
    }
  })

  function onVideoReady() {
    videoReady = true
  }

  function onImg(el: HTMLImageElement) {
    if (thumbSrc) lazyImage(el)
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="card"
  style={delay ? `animation-delay: ${delay}ms` : ''}
  class:card-failed={isFailed}
  class:card-selected={selected}
  class:card-active={isActive}
  class:card-fav={favorite}
  class:card-fill={fill}
  class:card-audio={entry.isAudio}
  class:card-video-shown={videoUrl && isHovering && videoReady}
  role="button"
  tabindex="0"
  data-card
  data-item-id={entry.id}
  data-item-type={entry.kind}
  data-url={entry.url || undefined}
  data-file-path={entry.file_path || undefined}
  data-title={entry.title || undefined}
  aria-label={entry.title ?? 'Элемент галереи'}
  aria-pressed={selected}
  onclick={handleCardClick}
  onkeydown={handleKey}
  onmouseenter={() => isHovering = true}
  onmouseleave={() => isHovering = false}
>
  {#if !entry.isAudio}
  <button
    class="card-check"
    class:card-check-selected={selected}
    aria-label={selected ? 'Снять выбор' : 'Выбрать'}
    aria-pressed={selected}
    onclick={(e) => { e.stopPropagation(); onToggleSelect(entry.id, e.shiftKey) }}
    use:tooltip={selected ? 'Снять выбор' : 'Выбрать'}
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
  </button>
  {:else}
  <button
    class="card-check card-check-audio"
    class:card-check-selected={selected}
    aria-label={selected ? 'Снять выбор' : 'Выбрать'}
    aria-pressed={selected}
    onclick={(e) => { e.stopPropagation(); onToggleSelect(entry.id, e.shiftKey) }}
    use:tooltip={selected ? 'Снять выбор' : 'Выбрать'}
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
  </button>
  {/if}

  <div class="card-top-right">
    <button
      class="card-fav-btn"
      class:is-fav={favorite}
      aria-label={favorite ? 'Убрать из избранного' : 'В избранное'}
      aria-pressed={favorite}
      onclick={(e) => { e.stopPropagation(); onToggleFavorite(entry.id) }}
      use:tooltip={favorite ? 'В избранном' : 'В избранное'}
    >
      <svg viewBox="0 0 24 24" fill={favorite ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
    </button>
  </div>

  {#if entry.isAudio}
    <!-- Аудио — только визуал пластинки -->
    <div class="vinyl">
      <div class="vinyl-disc" class:spin={isHovering}>
        {#if hasThumb}
          <img
            class="vinyl-cover"
            class:is-loaded={loaded}
            alt={entry.title ?? 'Обложка'}
            data-src={thumbSrc}
            use:onImg
            onload={() => { loaded = true; failed = false; }}
            onerror={() => {
              if (!useRetry && localSrc) useRetry = true
              else { failed = true; loaded = false }
            }}
            loading="lazy"
            decoding="async"
          />
        {:else}
          <div class="vinyl-cover vinyl-cover-empty" style={phGradient}>
            <span class="ph-letter">{initial}</span>
            <svg class="ph-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
          </div>
        {/if}
        <div class="vinyl-grooves"></div>
        <div class="vinyl-scrim"></div>
        <div class="vinyl-hole"></div>
      </div>
    </div>

    <!-- Мета сверху — состояние / платформа (только если есть что показать) -->
    {#if (entry.kind === 'task' && entry.state) || (entry.kind !== 'task' && entry.platform)}
    <div class="audio-meta-top">
      {#if entry.kind === 'task'}
        <span class="card-state" data-state={entry.state!}>{stateText(entry)}</span>
      {:else}
        <span class="card-platform">{entry.platform}</span>
      {/if}
    </div>
    {/if}

    <!-- Мета снизу — длительность, название, параметры -->
    <div class="audio-meta-bottom">
      {#if entry.duration}
        <span class="card-duration">{formatDuration(entry.duration)}</span>
      {/if}
      {#if entry.title}
        <div class="card-title-wrap audio-title-wrap">
          <p class="card-title">{entry.title}</p>
        </div>
      {/if}
      {#if entry.kind === 'history' && showParams}
        <p class="card-params hover-only">{formatParams(entry)}</p>
      {/if}
      {#if isFailed && entry.error}
        <p class="card-error hover-only">{entry.error}</p>
      {/if}
    </div>
  {:else if hasThumb}
    <div class="card-ph" style={phGradient}></div>
    <img
      class="card-img"
      class:is-loaded={loaded}
      alt={entry.title ?? 'Превью'}
      data-src={thumbSrc}
      use:onImg
      onload={(e) => {
        loaded = true
        failed = false
        if (fill) {
          const img = e.currentTarget as HTMLImageElement
          if (img.naturalWidth > 0 && img.naturalHeight > 0) {
            aspectStore[entry.id] = [img.naturalWidth, img.naturalHeight]
            onAspect?.(entry.id, img.naturalWidth, img.naturalHeight)
          }
        }
      }}
      onerror={() => {
        if (!useRetry && localSrc) {
          useRetry = true
        } else {
          failed = true; loaded = false
        }
      }}
      loading="lazy"
      decoding="async"
    />
    {#if videoUrl}
      <video
        class="card-video-preview"
        class:visible={isHovering && videoReady}
        muted loop playsinline preload="metadata"
        bind:this={videoEl}
        oncanplay={onVideoReady}
      ></video>
    {/if}
  {:else}
    <div class="card-img card-img-empty" style={phGradient}>
      <span class="ph-letter">{initial}</span>
      <svg class="ph-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>
    </div>
  {/if}

  {#if !entry.isAudio}
    <div class="card-overlay">
      <div class="card-meta">
        {#if entry.kind === 'task'}
          <span class="card-state" data-state={entry.state ?? ''}>{stateText(entry)}</span>
          {#if showProgress && !(entry.state === 'converting' && entry.progress <= 0)}
            <span class="card-pct">{entry.progress.toFixed(0)}%</span>
          {/if}
        {:else if entry.platform}
          <span class="card-platform hover-only">{entry.platform}</span>
        {/if}
        {#if entry.duration}
          <span class="card-duration">{formatDuration(entry.duration)}</span>
        {/if}
      </div>
      {#if entry.title}
        <div class="card-title-wrap">
          <p class="card-title">{entry.title}</p>
        </div>
      {/if}
      {#if entry.kind === 'history' && showParams}
        <p class="card-params hover-only">{formatParams(entry)}</p>
      {/if}
      {#if isFailed && entry.error}
        <p class="card-error hover-only">{entry.error}</p>
      {/if}
    </div>
  {/if}

   {#if showProgress}
    <div class="card-progress" class:indeterminate={entry.state === 'converting' && entry.progress <= 0}>
      {#if entry.state === 'converting' && entry.progress <= 0}
        <div class="card-progress-indet"></div>
      {:else}
        <div class="card-progress-fill" style="width:{entry.progress}%"></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    box-shadow: var(--shadow-card);
    transition: transform 0.25s, box-shadow 0.25s, border-color 0.25s;
    aspect-ratio: 16 / 9;
    animation: card-in 0.45s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  @keyframes card-in {
    from { opacity: 0; transform: translateY(10px); }
    to { opacity: 1; transform: none; }
  }
  .card.card-fill {
    aspect-ratio: auto;
    width: 100%;
    height: 100%;
    border-radius: 16px;
  }
  .card:hover {
    transform: translateY(-5px);
    box-shadow: var(--shadow-card-hover);
    border-color: var(--border-default);
  }
  .card:hover .card-img { transform: scale(1.06); }
  .card-active { border-color: color-mix(in srgb, var(--status-downloading) 30%, transparent); }
  .card-failed { border-color: color-mix(in srgb, var(--accent) 35%, transparent); }
  .card-fav { border-color: color-mix(in srgb, var(--accent-warm) 45%, transparent); }
  .card-selected {
    border: 2px solid var(--accent);
    box-shadow: var(--shadow-card-hover);
  }

  /* Аудио-карточка — пластинка в конверте.
     Карточка сохраняет фон конверта (card bg), но без прямоугольной рамки.
     Сам винил — круг диаметром 55% высоты ячейки, центрирован,
     с циркулярной рамкой. */
  .card-audio { border: none; box-shadow: none; background: transparent; overflow: visible; animation: none; }
  .card-audio:hover { border-color: transparent; box-shadow: none; }
  .card-audio.card-selected .vinyl { box-shadow: 0 0 0 2px var(--accent); }
  .card-audio.card-fav .vinyl { box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-warm) 45%, transparent); }
  .card-audio.card-failed .vinyl { box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent); }
  .card-audio.card-active .vinyl { box-shadow: 0 0 0 2px color-mix(in srgb, var(--status-downloading) 30%, transparent); }
  .card-check-audio { backdrop-filter: none; }
  .card-audio .card-fav-btn { backdrop-filter: none; }
  .vinyl {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    aspect-ratio: 1;
    height: 100%;
    width: auto;
    max-width: 100%;
    border-radius: 50%;
    border: 1px solid color-mix(in srgb, var(--on-scrim) 22%, transparent);
    box-sizing: border-box;
  }
  .vinyl-disc {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    overflow: hidden;
  }
  .vinyl-disc.spin { animation: vinyl-spin 5s linear infinite; }
  @keyframes vinyl-spin {
    to { transform: rotate(360deg); }
  }
  .vinyl-cover {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.3s ease;
  }
  .vinyl-cover.is-loaded { opacity: 1; }
  .vinyl-cover-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--text-muted);
    background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 14%, transparent) 0%, var(--scrim-soft) 100%);
  }
  .vinyl-grooves {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    pointer-events: none;
    background:
      radial-gradient(circle at center, transparent 56%, color-mix(in srgb, var(--scrim) 32%, transparent) 100%),
      repeating-radial-gradient(circle at center, transparent 0 2px, color-mix(in srgb, var(--on-scrim) 9%, transparent) 2px 3px);
  }
  .vinyl-scrim {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    pointer-events: none;
    background: linear-gradient(
      to top,
      color-mix(in srgb, var(--scrim) 88%, transparent) 0%,
      color-mix(in srgb, var(--scrim) 42%, transparent) 42%,
      transparent 66%
    );
  }
  .vinyl-hole {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 5%;
    height: 5%;
    min-width: 5px;
    min-height: 5px;
    border-radius: 50%;
    background: var(--bg-content);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--on-scrim) 28%, transparent);
  }

  /* Мета-информация снаружи винила — на конверте */
  .audio-meta-top {
    position: absolute;
    top: 5px;
    left: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 6px;
    pointer-events: none;
    z-index: 2;
  }
  .audio-meta-top .card-state,
  .audio-meta-top .card-platform {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--bg-elevated) 80%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
  }
  .audio-meta-top .card-platform {
    color: color-mix(in srgb, var(--text-muted) 85%, transparent);
  }
  .audio-meta-bottom {
    position: absolute;
    bottom: 6px;
    left: 10px;
    right: 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    pointer-events: none;
    z-index: 2;
  }
  .audio-meta-bottom .card-duration {
    font-size: 10px;
    font-weight: 600;
    color: color-mix(in srgb, var(--on-scrim) 90%, transparent);
    background: color-mix(in srgb, var(--scrim) 65%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    margin-bottom: 0;
    width: fit-content;
  }
  .audio-title-wrap {
    max-height: 0;
    opacity: 0;
    overflow: hidden;
    transition: opacity 200ms ease, max-height 200ms ease;
  }
  .card-audio:hover .audio-title-wrap { max-height: 48px; opacity: 1; }

  .card-ph {
    position: absolute;
    inset: 0;
    z-index: 0;
  }
  .card-img {
    position: relative;
    z-index: 1;
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.3s ease, transform 0.25s ease;
  }
  .card-img.is-loaded {
    opacity: 1;
  }

  .card-img-empty {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 100%;
    color: var(--text-muted);
  }
  .ph-letter {
    font-size: 30px;
    font-weight: 700;
    line-height: 1;
    color: var(--text-muted);
    opacity: 0.5;
    text-transform: uppercase;
  }
  .ph-icon { width: 26px; height: 26px; opacity: 0.5; }

  .card-overlay {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 10px 12px 6px;
    background: transparent;
    pointer-events: none;
    backface-visibility: hidden;
  }
  .card-overlay::before,
  .card-overlay::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: -1px;
    z-index: -1;
    pointer-events: none;
  }
  .card-overlay::before {
    background: var(--card-scrim-persistent);
    opacity: 1;
    transition: opacity var(--transition-default);
  }
  .card-overlay::after {
    background: var(--card-scrim-hover);
    opacity: 0;
    transition: opacity var(--transition-default);
  }
  .card:hover .card-overlay::before,
  .card-active .card-overlay::before,
  .card-selected .card-overlay::before { opacity: 0; }
  .card:hover .card-overlay::after,
  .card-active .card-overlay::after,
  .card-selected .card-overlay::after { opacity: 1; }
  .hover-only {
    opacity: 0;
    transform: translateY(4px);
    transition: opacity var(--transition-default), transform var(--transition-default);
  }
  .card:hover .hover-only { opacity: 1; transform: none; }
  .card-title-wrap {
    max-height: 0;
    opacity: 0;
    overflow: hidden;
    transition: opacity 200ms ease, max-height 200ms ease;
  }
  .card:hover .card-title-wrap {
    max-height: 60px;
    opacity: 1;
  }
  .card-title {
    margin: 0;
    min-height: 0;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.35;
    color: var(--text-primary);
    display: -webkit-box;
    -webkit-line-clamp: 1;
    line-clamp: 1;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 1px 6px color-mix(in srgb, var(--scrim) 90%, transparent), 0 0 2px color-mix(in srgb, var(--scrim) 60%, transparent);
  }
  .card-params {
    margin: 3px 0 0;
    font-size: 10px;
    font-weight: 500;
    color: color-mix(in srgb, var(--on-scrim) 75%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 4px color-mix(in srgb, var(--scrim) 90%, transparent);
  }
  .card-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
    flex-wrap: wrap;
  }
  .card-state {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--text-primary);
    text-shadow: 0 1px 4px color-mix(in srgb, var(--scrim) 90%, transparent);
  }
  .card-state[data-state='downloading'] { color: var(--status-downloading); }
  .card-state[data-state='converting'] { color: var(--thought-warning); }
  .card-state[data-state='failed'] { color: var(--status-error); }
  .card-state[data-state='completed'] { color: var(--status-success); }
  .card-pct { font-size: 10px; font-weight: 700; color: var(--text-primary); text-shadow: 0 1px 4px color-mix(in srgb, var(--scrim) 90%, transparent); }
  .card-platform {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--on-scrim) 85%, transparent);
    background: color-mix(in srgb, var(--scrim) 50%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
  }
  .card-duration {
    font-size: 10px;
    font-weight: 600;
    color: color-mix(in srgb, var(--on-scrim) 90%, transparent);
    background: color-mix(in srgb, var(--scrim) 65%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    margin-bottom: 0;
    width: fit-content;
  }
  .card-error {
    margin: 4px 0 0;
    font-size: 10px;
    color: color-mix(in srgb, var(--status-error) 95%, transparent);
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-shadow: 0 1px 6px color-mix(in srgb, var(--scrim) 100%, transparent);
  }

  .card-progress {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: color-mix(in srgb, var(--on-scrim) 6%, transparent);
    z-index: 3;
  }
  .card-progress-fill {
    height: 100%;
    background: var(--accent);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 60%, transparent);
    transition: width 0.4s ease;
  }
  /* Индетерминантный бар во время конвертации: длительность не всегда
     известна, поэтому процент может быть 0 — движущийся блик показывает,
     что процесс живой и не завис. */
  .card-progress.indeterminate {
    background: color-mix(in srgb, var(--on-scrim) 10%, transparent);
  }
  .card-progress-indet {
    height: 100%;
    width: 100%;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--accent) 0%, transparent) 0%,
      color-mix(in srgb, var(--accent) 85%, transparent) 50%,
      color-mix(in srgb, var(--accent) 0%, transparent) 100%
    );
    background-size: 42% 100%;
    background-repeat: no-repeat;
    animation: indet-slide 1.1s ease-in-out infinite;
  }
  @keyframes indet-slide {
    0%   { background-position: -45% 0; }
    100% { background-position: 145% 0; }
  }

  .card-check {
    position: absolute;
    top: 8px;
    left: 8px;
    z-index: 4;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, var(--scrim) 55%, transparent);
    backdrop-filter: blur(6px);
    border: 1px solid color-mix(in srgb, var(--on-scrim) 18%, transparent);
    color: var(--on-scrim);
    cursor: pointer;
    opacity: 0;
    transition: background var(--transition-fast), opacity var(--transition-fast), transform var(--transition-fast);
  }
  .card-check svg { width: 14px; height: 14px; }
  .card:hover .card-check { opacity: 1; }
  .card-check:hover { background: var(--accent); border-color: var(--accent); }
  .card-check.card-check-selected { background: var(--accent); border-color: var(--accent); opacity: 1; }
  .card-selected .card-check { background: var(--accent); border-color: var(--accent); opacity: 1; }

  .card-top-right {
    position: absolute;
    top: 8px;
    right: 8px;
    z-index: 4;
  }

  .card-fav-btn {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, var(--scrim) 55%, transparent);
    backdrop-filter: blur(6px);
    border: 1px solid color-mix(in srgb, var(--on-scrim) 18%, transparent);
    color: color-mix(in srgb, var(--on-scrim) 85%, transparent);
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--transition-fast), background var(--transition-fast), color var(--transition-fast), transform var(--transition-fast);
  }
  .card:hover .card-fav-btn,
  .card-selected .card-fav-btn { opacity: 1; }
  .card-fav-btn svg { width: 15px; height: 15px; }
  .card-fav-btn:hover { color: var(--accent-warm); transform: scale(1.12); }
  .card-fav-btn.is-fav { color: var(--accent-warm); border-color: color-mix(in srgb, var(--accent-warm) 60%, transparent); }
  .card-fav-btn.is-fav:hover { background: color-mix(in srgb, var(--accent-warm) 18%, transparent); }

  .card:focus-visible { box-shadow: 0 0 0 2px var(--accent), var(--shadow-card-hover); }
  .card-fav-btn:focus-visible { box-shadow: 0 0 0 2px var(--accent); }
  .card-check:focus-visible { box-shadow: 0 0 0 2px var(--accent); }

  .card-video-preview {
    position: absolute; inset: 0; z-index: 1;
    width: 100%; height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.2s ease;
    pointer-events: none;
  }
  .card-video-preview.visible { opacity: 1; }
  .card-video-shown .card-img.is-loaded { opacity: 0; }
</style>
