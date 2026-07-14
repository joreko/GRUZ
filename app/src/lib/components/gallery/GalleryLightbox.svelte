<script lang="ts">
  import { tooltip } from '$lib/utils/tooltip'
  import { formatDuration, formatParams, formatBytes, formatDate } from '$lib/utils/format'
  import { commands } from '$lib/bridge/commands'
  import { flipOpen, flipClose, prefersReducedMotion } from '$lib/gallery/hero'
  import { assetUrl } from '$lib/utils/media.svelte'
  import type { CardModel, CardAction } from './types'

  interface Props {
    entry: CardModel | null
    hasPrev: boolean
    hasNext: boolean
    originRect?: DOMRect | null
    returnFocusEl?: HTMLElement | null
    onClose: () => void
    onNavigate: (dir: -1 | 1) => void
    onAction: (action: CardAction, entry: CardModel) => void
  }

  let {
    entry,
    hasPrev,
    hasNext,
    originRect = null,
    returnFocusEl = null,
    onClose,
    onNavigate,
    onAction,
  }: Props = $props()

  let loaded = $state(false)
  let failed = $state(false)
  let mediaUrl = $state<string | null>(null)
  let mediaError = $state(false)
  let videoReady = $state(false)

  // Зум / пан
  let zoom = $state(1)
  let panX = $state(0)
  let panY = $state(0)

  // Слайд-шоу
  let playing = $state(false)
  let slideTimer: ReturnType<typeof setInterval> | null = null
  const SLIDE_MS = 4000

  let mediaEl = $state<HTMLElement | null>(null)
  let imgEl = $state<HTMLImageElement | null>(null)
  let mediaWrapEl = $state<HTMLElement | null>(null)
  let dialogEl = $state<HTMLDivElement | null>(null)
  let closeBtnEl = $state<HTMLButtonElement | null>(null)

  // Hero-морф и авто-скрытие интерфейса
  let morphing = $state(false)
  let uiVisible = $state(true)
  let infoOpen = $state(false)
  let pointerDown = $state(false)
  let uiTimer: ReturnType<typeof setTimeout> | null = null

  const posterSrc = $derived(entry ? (assetUrl(entry.localThumbnail) || entry.thumbnail || null) : null)

  const isVideo = $derived(
    !!entry && entry.kind === 'history' && !!entry.file_path && !entry.isAudio,
  )
  const isAudioFile = $derived(
    !!entry && entry.kind === 'history' && !!entry.file_path && entry.isAudio,
  )
  // Есть ли что морфить: видео/аудио с постером или обложкой, либо картинка
  const hasVisual = $derived(
    isVideo || isAudioFile ? !!posterSrc : !!entry?.thumbnail && !failed && !mediaError,
  )

  // Сброс состояния при смене элемента
  $effect(() => {
    if (entry) {
      loaded = false
      failed = false
      mediaError = false
      videoReady = false
      zoom = 1
      panX = 0
      panY = 0
      mediaUrl = null
      if ((isVideo || isAudioFile)) {
        commands.getMediaUrl(entry.id)
          .then((u) => {
            if (!u) { mediaError = true; mediaUrl = null }
            else mediaUrl = assetUrl(u)
          })
          .catch(() => { mediaError = true; mediaUrl = null })
      }
    }
  })

  // Hero-transition при открытии: морфим обёртку медиа из карточки.
  // Хром прячем на время морфа и показываем после оседания.
  $effect(() => {
    if (entry && mediaWrapEl && originRect && !prefersReducedMotion() && hasVisual) {
      morphing = true
      uiVisible = false
      flipOpen(originRect, mediaWrapEl, () => {
        morphing = false
        uiVisible = true
      })
    } else if (entry) {
      morphing = false
      uiVisible = true
    }
  })

  // Слайд-шоу: авто-перелистывание
  $effect(() => {
    if (playing && hasNext) {
      slideTimer = setInterval(() => onNavigate(1), SLIDE_MS)
    } else if (slideTimer) {
      clearInterval(slideTimer)
      slideTimer = null
    }
    return () => { if (slideTimer) clearInterval(slideTimer) }
  })

  function pauseSlideshow() {
    if (playing) playing = false
  }

  function togglePlay() {
    playing = !playing
  }

  function requestClose() {
    pauseSlideshow()
    if (originRect && mediaWrapEl && !prefersReducedMotion() && hasVisual) {
      flipClose(mediaWrapEl, originRect, () => {
        returnFocus()
        onClose()
      })
    } else {
      returnFocus()
      onClose()
    }
  }

  function returnFocus() {
    returnFocusEl?.focus?.()
  }

  function onMediaError() {
    mediaError = true
  }

  function pokeUI() {
    uiVisible = true
    if (uiTimer) clearTimeout(uiTimer)
    uiTimer = setTimeout(() => { if (!morphing) uiVisible = false }, 2600)
  }

  function onKey(e: KeyboardEvent) {
    if (!entry) return
    if (e.key === 'Escape') { e.preventDefault(); requestClose(); return }
    if (e.key === 'ArrowLeft' && hasPrev) { e.preventDefault(); pauseSlideshow(); onNavigate(-1); return }
    if (e.key === 'ArrowRight' && hasNext) { e.preventDefault(); pauseSlideshow(); onNavigate(1); return }
    if (e.key === ' ' || e.key === 'Spacebar') {
      const ae = document.activeElement
      if (ae && (ae.tagName === 'BUTTON' || ae.tagName === 'VIDEO' || ae.tagName === 'AUDIO')) return
      e.preventDefault(); togglePlay(); return
    }
    if (e.key === 'Tab') trapFocus(e)
  }

  function trapFocus(e: KeyboardEvent) {
    const root = dialogEl
    if (!root) return
    const focusables = root.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, video, audio, [tabindex]:not([tabindex="-1"])',
    )
    if (focusables.length === 0) return
    const first = focusables[0]
    const last = focusables[focusables.length - 1]
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault(); last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault(); first.focus()
    }
  }

  // Зум колесом
  function onWheel(e: WheelEvent) {
    e.preventDefault()
    pauseSlideshow()
    const delta = -e.deltaY * 0.0015
    const next = Math.min(4, Math.max(1, zoom + delta))
    if (next === 1) { panX = 0; panY = 0 }
    zoom = next
  }

  function onDblClick() {
    pauseSlideshow()
    if (zoom > 1) { zoom = 1; panX = 0; panY = 0 }
    else { zoom = 2.4 }
  }

  // Пан + тач-жесты
  let pStart = $state<{ x: number; y: number } | null>(null)
  let pMoved = $state(false)

  function onPointerDown(e: PointerEvent) {
    pauseSlideshow()
    pointerDown = true
    pStart = { x: e.clientX, y: e.clientY }
    pMoved = false
    if (zoom > 1) (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onPointerMove(e: PointerEvent) {
    if (!pStart) return
    const dx = e.clientX - pStart.x
    const dy = e.clientY - pStart.y
    if (Math.abs(dx) > 4 || Math.abs(dy) > 4) pMoved = true
    if (zoom > 1) {
      panX += e.movementX
      panY += e.movementY
    }
  }

  function onPointerUp(e: PointerEvent) {
    pointerDown = false
    if (!pStart) return
    const dx = e.clientX - pStart.x
    const dy = e.clientY - pStart.y
    pStart = null
    if (zoom > 1) return
    // Тач-жесты только для пальца и без заметного пана
    if (e.pointerType === 'touch' && !pMoved) return
    if (e.pointerType === 'touch') {
      if (Math.abs(dx) > 50 && Math.abs(dx) > Math.abs(dy)) {
        if (dx < 0 && hasNext) onNavigate(1)
        else if (dx > 0 && hasPrev) onNavigate(-1)
      } else if (dy > 80 && Math.abs(dy) > Math.abs(dx)) {
        requestClose()
      }
    }
  }

  const isActive = $derived(!!entry && entry.kind === 'task' && entry.state !== 'completed' && entry.state !== 'cancelled')
  const showProgress = $derived(!!entry && entry.kind === 'task' && (entry.state === 'downloading' || entry.state === 'converting'))
  const isFailed = $derived(!!entry && entry.kind === 'task' && entry.state === 'failed')

  const stateLabel: Record<string, string> = {
    waiting: 'Ожидание',
    downloading: 'Загрузка',
    converting: 'Конвертация',
    paused: 'Пауза',
    completed: 'Готово',
    failed: 'Ошибка',
    cancelled: 'Отменено',
  }

  const metaRows = $derived.by(() => {
    if (!entry) return []
    const rows: { label: string; value: string }[] = []
    if (entry.channel) rows.push({ label: 'Канал', value: entry.channel })
    if (entry.platform) rows.push({ label: 'Платформа', value: entry.platform })
    if (entry.duration) rows.push({ label: 'Длительность', value: formatDuration(entry.duration) })
    if (entry.file_size != null) rows.push({ label: 'Размер', value: formatBytes(entry.file_size) })
    if (entry.kind === 'history') {
      rows.push({ label: 'Параметры', value: formatParams(entry) })
      rows.push({ label: 'Добавлено', value: formatDate(entry.created_at) })
    }
    return rows
  })

  const mediaTransform = $derived(`translate(${panX}px, ${panY}px) scale(${zoom})`)
</script>

<svelte:window onkeydown={onKey} />

{#if entry}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="lb-overlay"
    role="dialog"
    aria-modal="true"
    aria-label={entry.title ?? 'Просмотр'}
    tabindex="-1"
    bind:this={dialogEl}
    onclick={(e) => { if (e.target === e.currentTarget) requestClose() }}
    onmousemove={pokeUI}
  >
    <!-- Верхняя панель управления (авто-скрывается) -->
    <div class="lb-chrome" class:ui-hidden={!uiVisible}>
      <button class="lb-btn-icon lb-info-toggle" class:active={infoOpen} aria-label="Информация" onclick={() => infoOpen = !infoOpen} use:tooltip={{text:'Информация', placement:'bottom'}}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
      </button>
      <button class="lb-btn-icon lb-close" aria-label="Закрыть" bind:this={closeBtnEl} onclick={requestClose}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>

    {#if hasPrev}
      <button class="lb-nav lb-nav-prev ui-hidden-el" class:ui-hidden={!uiVisible} aria-label="Назад" onclick={() => { pauseSlideshow(); onNavigate(-1) }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
      </button>
    {/if}
    {#if hasNext}
      <button class="lb-nav lb-nav-next ui-hidden-el" class:ui-hidden={!uiVisible} aria-label="Вперёд" onclick={() => { pauseSlideshow(); onNavigate(1) }}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
      </button>
    {/if}

    <div class="lb-stage" class:is-zoomable={zoom > 1} class:lb-stage--info-open={infoOpen}>
      <div
        class="lb-stage-inner"
        onwheel={onWheel}
        ondblclick={onDblClick}
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
      >
        {#key entry.id}
          <div
            class="lb-media-wrap"
            class:no-transition={pointerDown}
            bind:this={mediaWrapEl}
            style={morphing ? '' : `transform: ${mediaTransform};`}
          >
            {#if mediaError}
              <div class="lb-media-error">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                <p class="lb-error-title">Файл не найден</p>
                {#if entry.file_path}
                  <code class="lb-error-path">{entry.file_path}</code>
                {/if}
                {#if entry.file_path}
                  <button class="lb-btn lb-btn-accent" onclick={() => commands.openFolder(entry.file_path!)}>Открыть папку</button>
                {/if}
              </div>
            {:else if isVideo && mediaUrl}
              <!-- svelte-ignore a11y_media_has_caption -->
              <video
                bind:this={mediaEl}
                class="lb-video"
                src={mediaUrl}
                poster={posterSrc || undefined}
                controls
                autoplay={playing}
                oncanplay={() => { videoReady = true }}
                onloadeddata={() => { videoReady = true }}
                onplay={pauseSlideshow}
                onerror={onMediaError}
              ></video>
              {#if !videoReady && posterSrc}
                <img class="lb-poster" src={posterSrc} alt="" />
              {/if}
            {:else if isAudioFile && mediaUrl}
              <div class="lb-audio-wrap">
                {#if posterSrc}
                  <img class="lb-cover" src={posterSrc} alt={entry.title ?? ''} onerror={onMediaError} />
                {:else}
                  <div class="lb-cover lb-cover-empty">
                    <span>{(entry.title ?? entry.platform ?? '?').trim().charAt(0).toUpperCase()}</span>
                  </div>
                {/if}
                <!-- svelte-ignore a11y_media_has_caption -->
                <audio class="lb-audio" controls src={mediaUrl} onerror={onMediaError}></audio>
              </div>
            {:else if entry.thumbnail && !failed}
              <img
                class="lb-img"
                class:is-loaded={loaded}
                alt={entry.title ?? 'Превью'}
                src={posterSrc || undefined}
                bind:this={imgEl}
                onload={() => { loaded = true }}
                onerror={() => { failed = true; loaded = false }}
              />
            {:else}
              <div class="lb-empty">
                <span class="lb-empty-letter">{(entry.title ?? entry.platform ?? '?').trim().charAt(0).toUpperCase()}</span>
              </div>
            {/if}
          </div>
        {/key}

        {#if showProgress}
          <div class="lb-progress">
            <div class="lb-progress-fill" style="width:{entry.progress}%"></div>
          </div>
        {/if}

        {#if zoom > 1}
          <button class="lb-zoom-reset" onclick={() => { zoom = 1; panX = 0; panY = 0 }} use:tooltip={'Сбросить зум'}>1:1</button>
        {/if}
      </div>

      <!-- Боковая панель информации (опциональна) -->
      {#if infoOpen}
        <aside class="lb-info" aria-label="Информация">
          <div class="lb-head">
            {#if entry.kind === 'task'}
              <span class="lb-badge" data-state={entry.state ?? ''}>{stateLabel[entry.state ?? ''] ?? entry.state}</span>
            {:else if entry.platform}
              <span class="lb-badge lb-badge-platform">{entry.platform}</span>
            {/if}
            {#if entry.duration}
              <span class="lb-duration">{formatDuration(entry.duration)}</span>
            {/if}
            <button class="lb-slide-btn" class:is-playing={playing} onclick={togglePlay} aria-label={playing ? 'Пауза' : 'Слайд-шоу'} use:tooltip={playing ? 'Пауза' : 'Слайд-шоу'}>
              {#if playing}
                <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="7 4 20 12 7 20 7 4"/></svg>
              {/if}
            </button>
          </div>

          <h2 class="lb-title">{entry.title ?? 'Без названия'}</h2>
          {#if entry.channel}
            <p class="lb-channel">{entry.channel}</p>
          {/if}

          {#if isFailed && entry.error}
            <p class="lb-error">{entry.error}</p>
          {/if}

          {#if metaRows.length}
            <dl class="lb-meta">
              {#each metaRows as row}
                <div class="lb-meta-row">
                  <dt>{row.label}</dt>
                  <dd>{row.value}</dd>
                </div>
              {/each}
            </dl>
          {/if}

          <div class="lb-actions">
            {#if isActive}
              <button class="lb-btn" onclick={() => onAction('cancel', entry)}>Отменить</button>
            {:else if isFailed}
              <button class="lb-btn lb-btn-accent" onclick={() => onAction('retry', entry)}>Повторить</button>
            {:else if entry.kind === 'history'}
              <button class="lb-btn lb-btn-accent" onclick={() => onAction('open-file', entry)}>Открыть</button>
              <button class="lb-btn" onclick={() => onAction('open-folder', entry)}>Папка</button>
              {#if entry.url}
                <button class="lb-btn" onclick={() => onAction('copy-link', entry)} use:tooltip={'Скопировать ссылку'}>Ссылка</button>
              {/if}
              <button class="lb-btn" onclick={() => onAction('redownload', entry)} use:tooltip={'Скачать заново'}>Перекачать</button>
            {/if}
            <button class="lb-btn lb-btn-danger" onclick={() => onAction('delete', entry)}>Удалить</button>
          </div>
        </aside>
      {/if}
    </div>
  </div>
{/if}

<style>
  .lb-overlay {
    position: absolute;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--scrim);
    animation: lb-fade 0.16s ease forwards;
  }
  @keyframes lb-fade { from { opacity: 0; } to { opacity: 1; } }

  .lb-stage {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    touch-action: none;
    background: var(--scrim);
  }
  .lb-stage-inner {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: grab;
  }
  .lb-stage.is-zoomable .lb-stage-inner { cursor: grab; }

  .lb-media-wrap {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.12s ease-out;
    will-change: transform;
  }
  .lb-media-wrap.no-transition { transition: none; }
  .lb-stage--info-open {
    justify-content: flex-start;
  }
  .lb-stage--info-open .lb-stage-inner {
    width: calc(100% - min(360px, 86vw));
    transition: width 0.2s ease;
  }
  .lb-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    opacity: 0;
    transition: opacity 0.3s ease;
    user-select: none;
    -webkit-user-drag: none;
  }
  .lb-img.is-loaded { opacity: 1; }
  .lb-video {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    background: var(--scrim);
    user-select: none;
    border-radius: var(--radius-xl);
    border: 1px solid var(--border-subtle);
  }
  .lb-poster {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
    animation: lb-fade 0.2s ease forwards;
  }
  .lb-audio-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    width: 100%;
    height: 100%;
    padding: 24px;
  }
  .lb-cover {
    max-width: min(70%, 420px);
    max-height: 60%;
    object-fit: contain;
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-panel);
  }
  .lb-cover-empty {
    display: flex; align-items: center; justify-content: center;
    width: 200px; height: 200px;
    background: linear-gradient(135deg, var(--bg-overlay) 0%, var(--scrim-soft) 100%);
    border-radius: var(--radius-lg);
  }
  .lb-cover-empty span { font-size: 72px; font-weight: 700; color: var(--text-muted); opacity: 0.5; text-transform: uppercase; }
  .lb-audio {
    width: min(80%, 520px);
    max-width: 100%;
    filter: invert(0.92) hue-rotate(180deg);
  }
  .lb-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, var(--bg-overlay) 0%, var(--scrim-soft) 100%);
  }
  .lb-empty-letter { font-size: 64px; font-weight: 700; color: var(--text-muted); opacity: 0.5; text-transform: uppercase; }

  .lb-media-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    max-width: 520px;
    padding: 24px;
    text-align: center;
  }
  .lb-media-error svg { width: 40px; height: 40px; color: var(--status-error); opacity: 0.8; }
  .lb-error-title { margin: 0; font-size: 16px; font-weight: 600; color: var(--text-primary); }
  .lb-error-path {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-secondary);
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-sm);
    padding: 6px 10px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lb-progress {
    position: absolute;
    bottom: 0; left: 0; right: 0;
    height: 4px;
    background: color-mix(in srgb, var(--on-scrim) 8%, transparent);
  }
  .lb-progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.4s ease;
  }
  .lb-zoom-reset {
    position: absolute;
    bottom: 14px; right: 14px;
    height: 28px; padding: 0 12px;
    background: color-mix(in srgb, var(--scrim) 60%, transparent);
    border: 1px solid color-mix(in srgb, var(--on-scrim) 18%, transparent);
    border-radius: var(--radius-sm);
    color: var(--on-scrim); font-size: 11px; font-weight: 600; cursor: pointer;
    backdrop-filter: blur(6px);
  }
  .lb-zoom-reset:hover { background: color-mix(in srgb, var(--scrim) 80%, transparent); }

  /* Боковая панель информации */
  .lb-info {
    position: absolute;
    top: 0; right: 0; bottom: 0;
    width: min(360px, 86vw);
    padding: 56px 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    background: color-mix(in srgb, var(--bg-elevated) 92%, transparent);
    backdrop-filter: blur(14px);
    border-left: 1px solid var(--border-subtle);
    box-shadow: var(--shadow-panel);
    animation: lb-info-in 0.2s ease forwards;
    z-index: 5;
  }
  @keyframes lb-info-in { from { opacity: 0; transform: translateX(16px); } to { opacity: 1; transform: none; } }
  .lb-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .lb-badge {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    background: var(--bg-overlay);
    color: var(--text-secondary);
  }
  .lb-badge[data-state='downloading'] { color: var(--status-downloading); }
  .lb-badge[data-state='converting'] { color: var(--thought-warning); }
  .lb-badge[data-state='failed'] { color: var(--status-error); }
  .lb-badge[data-state='completed'] { color: var(--status-success); }
  .lb-badge-platform { color: var(--text-primary); }
  .lb-duration {
    font-size: 11px;
    font-weight: 600;
    color: color-mix(in srgb, var(--on-scrim) 90%, transparent);
    background: color-mix(in srgb, var(--scrim) 45%, transparent);
    border-radius: var(--radius-sm);
    padding: 1px 6px;
  }
  .lb-slide-btn {
    margin-left: auto;
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .lb-slide-btn svg { width: 14px; height: 14px; }
  .lb-slide-btn:hover { color: var(--text-primary); border-color: var(--border-strong); }
  .lb-slide-btn.is-playing { color: var(--accent-warm); border-color: color-mix(in srgb, var(--accent-warm) 45%, transparent); }
  .lb-title {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    line-height: 1.35;
    color: var(--text-primary);
  }
  .lb-channel { margin: 0; font-size: 13px; color: var(--text-secondary); }
  .lb-error {
    margin: 0;
    font-size: 12px;
    color: var(--status-error);
    line-height: 1.45;
    background: color-mix(in srgb, var(--status-error) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-error) 30%, transparent);
    border-radius: var(--radius-sm);
    padding: 8px 10px;
  }
  .lb-meta { margin: 0; display: flex; flex-direction: column; gap: 6px; }
  .lb-meta-row { display: flex; justify-content: space-between; gap: 12px; font-size: 12px; }
  .lb-meta-row dt { color: var(--text-muted); }
  .lb-meta-row dd { margin: 0; color: var(--text-secondary); text-align: right; font-weight: 500; }

  .lb-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: auto; padding-top: 8px; }
  .lb-btn {
    height: 34px;
    padding: 0 14px;
    background: var(--bg-overlay);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
  }
  .lb-btn:hover { background: var(--border-subtle); border-color: var(--border-strong); }
  .lb-btn-accent { background: var(--accent); border-color: var(--accent); color: var(--text-inverse); }
  .lb-btn-accent:hover { filter: brightness(1.1); background: var(--accent); }
  .lb-btn-danger { color: var(--status-error); border-color: color-mix(in srgb, var(--status-error) 40%, transparent); }
  .lb-btn-danger:hover { background: color-mix(in srgb, var(--status-error) 15%, transparent); border-color: var(--status-error); }

  /* Верхняя панель + навигация (авто-скрытие) */
  .lb-chrome {
    position: absolute;
    top: 16px; right: 16px;
    z-index: 201;
    display: flex; gap: 8px;
    transition: opacity var(--transition-default), transform var(--transition-default);
  }
  .lb-btn-icon {
    width: 38px; height: 38px;
    display: flex; align-items: center; justify-content: center;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, var(--scrim) 50%, transparent);
    border: 1px solid color-mix(in srgb, var(--on-scrim) 15%, transparent);
    color: var(--on-scrim);
    cursor: pointer;
    backdrop-filter: blur(6px);
    transition: background var(--transition-fast);
  }
  .lb-btn-icon svg { width: 18px; height: 18px; }
  .lb-btn-icon:hover { background: color-mix(in srgb, var(--scrim) 75%, transparent); }
  .lb-info-toggle.active { color: var(--accent-warm); border-color: color-mix(in srgb, var(--accent-warm) 50%, transparent); }

  .lb-nav {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    z-index: 201;
    width: 46px; height: 46px;
    display: flex; align-items: center; justify-content: center;
    border-radius: var(--radius-full);
    background: color-mix(in srgb, var(--scrim) 50%, transparent);
    border: 1px solid color-mix(in srgb, var(--on-scrim) 15%, transparent);
    color: var(--on-scrim);
    cursor: pointer;
    backdrop-filter: blur(6px);
    transition: background var(--transition-fast), opacity var(--transition-default), transform var(--transition-default);
  }
  .lb-nav:hover { background: color-mix(in srgb, var(--scrim) 75%, transparent); }
  .lb-nav svg { width: 22px; height: 22px; }
  .lb-nav-prev { left: 18px; }
  .lb-nav-next { right: 18px; }

  .ui-hidden, .ui-hidden-el.ui-hidden {
    opacity: 0;
    pointer-events: none;
  }
  .lb-nav.ui-hidden { transform: translateY(-50%) scale(0.9); }

  .lb-nav:focus-visible,
  .lb-btn:focus-visible,
  .lb-btn-icon:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
