<script lang="ts">
  import { commands } from '$lib/bridge/commands'
  import { open } from '@tauri-apps/plugin-dialog'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { isValidYoutubeUrl, detectUrlType, classifyRejectedUrl } from '$lib/utils/url'
  import { formatDuration, formatBytes } from '$lib/utils/format'
  import { store, updateSetting } from '$lib/stores/settings.svelte'
  import { dl } from '$lib/stores/download.svelte'

  import type { Route } from '$lib/bridge/types'
  let { route = $bindable<Route>('download') } = $props()

  // Свободное место на диске загрузки
  let freeSpace = $state<number | null>(null)
  $effect(() => {
    const path = store.settings ? store.settings.download_dir : '.'
    commands.getFreeSpace(path).then(n => { freeSpace = n }).catch(() => { freeSpace = null })
  })

  async function pickDownloadDir() {
    const dir = await open({ directory: true, defaultPath: store.settings?.download_dir })
    if (dir) await updateSetting('download_dir', dir)
  }
  import { emitThought } from '$lib/stores/thought.svelte'
  import { onDestroy } from 'svelte'
  import type { VideoInfo, VideoFormat } from '$lib/bridge/types'

  // ── Состояние живёт в сторе — сохраняется при навигации ───────────────────

  $effect(() => {
    if (store.settings && !dl.info && !dl.url) {
      dl.format       = store.settings.default_format
      dl.container    = store.settings.default_container
      dl.fps          = store.settings.default_fps ?? null
      dl.bitrate      = store.settings.default_bitrate ?? null
      dl.videoCodec   = store.settings.default_video_codec ?? null
      dl.audioCodec   = store.settings.default_audio_codec ?? null
    }
  })

  // Автосохранение параметров как дефолтов при каждом изменении
  $effect(() => {
    const f = dl.format, c = dl.container, fps = dl.fps,
          br = dl.bitrate, vc = dl.videoCodec, ac = dl.audioCodec
    if (!store.settings || dl.loading) return
    updateSetting('default_format', f)
    updateSetting('default_container', c)
    updateSetting('default_fps', fps != null ? String(fps) : '')
    updateSetting('default_bitrate', br != null ? String(br) : '')
    updateSetting('default_video_codec', vc ?? '')
    updateSetting('default_audio_codec', ac ?? '')
  })

  const urlType    = $derived(detectUrlType(dl.url))
  const urlValid   = $derived(isValidYoutubeUrl(dl.url))
  const canFetch   = $derived(urlValid && !dl.loading)
  const canDownload = $derived(!!dl.info && !!dl.quality && !dl.queuing && !dl.queued)

  // Реакция оркестратора на неправильную ссылку (один раз при появлении)
  let lastRejectedUrl = ''
  $effect(() => {
    if (rejected && dl.url !== lastRejectedUrl) {
      lastRejectedUrl = dl.url
      const phrases: Record<string, [string, string]> = {
        nsfw:        ['не по адресу.', 'pink'],
        video_other: ['это не youtube.', 'warning'],
        social:      ['это не ссылка на видео.', 'warning'],
        not_url:     ['это не ссылка.', 'muted'],
        other_site:  ['не мой сайт.', 'muted'],
      }
      const [t, c] = phrases[rejected.kind] ?? ['не то.', 'muted']
      emitThought(t, c, 1)
    }
    if (!dl.url) lastRejectedUrl = ''
  })

  // Фильтрация форматов
  const allVideoFormats = $derived(
    (dl.info?.formats ?? [])
      .filter(f => !f.is_audio_only && f.resolution && f.vcodec)
      .sort((a, b) => {
        const h = (f: VideoFormat) => parseInt(f.resolution?.split('x')[1] ?? '0')
        return h(b) - h(a)
      })
  )

  // Уникальные разрешения для первой строки
  const videoFormats = $derived(
    allVideoFormats.reduce<VideoFormat[]>((acc, f) => {
      const res = f.resolution ?? ''
      if (!acc.find(x => x.resolution === res)) acc.push(f)
      return acc
    }, [])
  )

  // format_id лучшего формата для данного разрешения (первый = наилучший кодек)
  function bestFormatId(resolution: string | null): string {
    return allVideoFormats.find(f => f.resolution === resolution)?.format_id ?? ''
  }

  // Каталоги кодеков
  const VIDEO_CODECS = [
    { id: 'h264',   label: 'H.264',  lib: 'libx264',    desc: 'Максимальная совместимость. Работает везде — After Effects, Premiere, любые плееры и устройства. Стандарт индустрии.' },
    { id: 'h265',   label: 'H.265',  lib: 'libx265',    desc: 'В 2× меньше размер при том же качестве. Нативно на iPhone, Apple Silicon, современных TV. Не поддерживается в старом ПО.' },
    { id: 'vp9',    label: 'VP9',    lib: 'libvpx-vp9', desc: 'Открытый кодек Google. Используется на YouTube. Отличное качество, широкая поддержка браузеров.' },
    { id: 'av1',    label: 'AV1',    lib: 'libaom-av1', desc: 'Наилучшее сжатие из всех. Открытый стандарт будущего. Конвертация очень медленная.' },
    { id: 'prores', label: 'ProRes', lib: 'prores_ks',   desc: 'Профессиональный кодек Apple для монтажа. Большой файл, без потерь качества. Только MOV контейнер.' },
  ]

  const AUDIO_CODECS = [
    { id: 'aac',  label: 'AAC',  lib: 'aac',        desc: 'Стандарт для MP4 и MOV. Максимальная совместимость с любым ПО и устройствами.' },
    { id: 'opus', label: 'Opus', lib: 'libopus',     desc: 'Открытый кодек. Лучшее качество при низком битрейте. Идеален для WebM.' },
    { id: 'mp3',  label: 'MP3',  lib: 'libmp3lame',  desc: 'Универсальный формат. Поддерживается буквально везде, включая старые устройства.' },
    { id: 'flac', label: 'FLAC', lib: 'flac',        desc: 'Без потерь. Идеальное качество, большой размер файла. Для архивирования.' },
    { id: 'ac3',  label: 'AC3',  lib: 'ac3',         desc: 'Dolby Digital. Стандарт для фильмов с объёмным звуком. Поддерживается в MKV/MOV.' },
  ]

  // Видимость пикеров кодеков
  let showVideoCodecPicker = $state(false)
  let showAudioCodecPicker = $state(false)

  // Действие: закрыть при клике вне элемента
  function clickOutside(node: HTMLElement, handler: () => void) {
    const onClick = (e: MouseEvent) => { if (!node.contains(e.target as Node)) handler() }
    document.addEventListener('click', onClick, true)
    return { destroy() { document.removeEventListener('click', onClick, true) } }
  }

  // Доступные fps для выбранного разрешения
  const selectedResolution = $derived(
    allVideoFormats.find(f => f.format_id === dl.quality)?.resolution ?? null
  )
  // Максимальный fps оригинала для выбранного разрешения
  const maxFps = $derived.by((): number => {
    if (!selectedResolution) return 0
    const resFormats = allVideoFormats.filter(f => f.resolution === selectedResolution && (f.fps ?? 0) > 0)
    if (resFormats.length) return Math.max(...resFormats.map(f => f.fps!))
    // Fallback: fps из любого формата
    return allVideoFormats.find(f => (f.fps ?? 0) > 0)?.fps ?? 0
  })
  // Стандартные fps варианты не превышающие оригинал
  const fpsOptions = $derived(
    maxFps > 0 ? [60, 30, 24].filter(f => f < maxFps) : []
  )

  const audioFormats = $derived(
    (dl.info?.formats ?? [])
      .filter(f => f.is_audio_only && f.acodec)
      .reduce<VideoFormat[]>((acc, f) => {
        const key = Math.round((f.abr ?? 0) / 32) * 32
        if (!acc.find(x => Math.round((x.abr ?? 0) / 32) * 32 === key)) acc.push(f)
        return acc
      }, [])
      .sort((a, b) => (b.abr ?? 0) - (a.abr ?? 0))
  )

  const selectedFormat = $derived(
    [...allVideoFormats, ...audioFormats].find(f => f.format_id === dl.quality) ?? null
  )

  // Подсчёт размера файла с учётом формата и битрейта
  const estimatedSize = $derived.by((): number | null => {
    if (!selectedFormat) return null
    const duration = dl.info?.duration ?? null
    // Коэффициент fps: при понижении fps размер видео уменьшается пропорционально
    const fpsRatio = (dl.fps && maxFps > 0) ? dl.fps / maxFps : 1

    if (dl.format === 'audio') {
      if (dl.bitrate && duration) return Math.round(dl.bitrate * 1000 / 8 * duration)
      return selectedFormat.filesize ?? null
    }

    if (dl.format === 'video_only') {
      if (dl.bitrate && duration) return Math.round(dl.bitrate * 1000 / 8 * duration * fpsRatio)
      if (selectedFormat.filesize) return Math.round(selectedFormat.filesize * fpsRatio)
      return null
    }

    // video = видео + лучший аудио поток
    const bestAudio = audioFormats[0] ?? null
    const videoSize = selectedFormat.filesize
    const audioSize = bestAudio?.filesize ?? null

    if (dl.bitrate && duration) {
      const audioBytes = bestAudio ? Math.round((bestAudio.abr ?? 128) * 1000 / 8 * duration) : 0
      return Math.round(dl.bitrate * 1000 / 8 * duration * fpsRatio) + audioBytes
    }

    if (videoSize && audioSize) return Math.round(videoSize * fpsRatio) + audioSize
    if (videoSize && bestAudio?.abr && duration)
      return Math.round(videoSize * fpsRatio) + Math.round(bestAudio.abr * 1000 / 8 * duration)
    if (videoSize) return Math.round(videoSize * fpsRatio)
    return null
  })

  // Битрейт: для видео берём vbr выбранного формата, для аудио — abr всех форматов
  const codecTip = $derived(
    (() => {
      const c = selectedFormat ? qualityDesc(selectedFormat) : ''
      if (c === 'AV1') return 'AV1 — лучшее сжатие, не все плееры поддерживают'
      if (c === 'VP9') return 'VP9 — хорошее сжатие, широкая совместимость'
      return 'AVC (H.264) — максимальная совместимость, не поддерживается After Effects'
    })()
  )
  const containerTip = $derived(
    dl.container === 'mov' ? 'MOV — для Adobe (Premiere, After Effects)' :
    dl.container === 'mkv' ? 'MKV — без ограничений, не все устройства поддерживают' :
    dl.container === 'webm' ? 'WebM — для веба, VP9/AV1' : 'MP4 — универсальный контейнер'
  )
  const resTip = $derived(`${selectedFormat?.resolution ?? ''} · ${selectedFormat ? qualityLabel(selectedFormat) : ''}`)

  const maxBitrate = $derived(
    selectedFormat
      ? Math.round(selectedFormat.is_audio_only
          ? Math.max(...audioFormats.map(f => f.abr ?? 0))
          : (selectedFormat.vbr ?? 0))
      : 0
  )
  const minBitrate = $derived(
    selectedFormat && maxBitrate > 0 ? Math.round(maxBitrate * 0.1) : 0
  )
  const currentBitrate = $derived(dl.bitrate ?? maxBitrate)

  function formatSubs(n: number): string {
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1) + ' млн подписчиков'
    if (n >= 1_000) return (n / 1_000).toFixed(n >= 10_000 ? 0 : 1) + ' тыс. подписчиков'
    return n + ' подписчиков'
  }

  function qualityLabel(f: VideoFormat) {
    if (f.is_audio_only) return f.abr ? `${Math.round(f.abr)}k` : f.ext.toUpperCase()
    const height = f.resolution?.split('x')[1]
    if (!height) return f.format_note ?? f.resolution ?? '?'
    const n = parseInt(height)
    if (n >= 2160) return '4K'
    if (n >= 1440) return '1440p'
    if (n >= 1080) return '1080p'
    if (n >= 720)  return '720p'
    if (n >= 480)  return '480p'
    if (n >= 360)  return '360p'
    return height + 'p'
  }

  function qualityDesc(f: VideoFormat) {
    if (f.is_audio_only) return f.acodec?.split('.')[0]?.toUpperCase() ?? 'AAC'
    const codec = f.vcodec?.split('.')[0]?.toUpperCase() ?? ''
    if (codec === 'AV01') return 'AV1'
    if (codec === 'VP09') return 'VP9'
    if (codec.startsWith('AVC')) return 'AVC'
    return codec || 'VIDEO'
  }

  // Выбрать качество по умолчанию с учётом настроек пользователя
  function pickDefaultQuality() {
    if ((dl.format === 'video' || dl.format === 'video_only') && videoFormats.length) {
      const preferred = store.settings?.default_quality
      // Проверяем доступно ли текущее качество
      const current = dl.quality ? videoFormats.find(f => f.format_id === dl.quality) : null
      const match = current ?? (preferred && preferred !== 'best'
        ? videoFormats.find(f => f.resolution?.includes(preferred.replace('p', '')))
        : null)
      dl.quality = (match ?? videoFormats[0]).format_id
    } else if (dl.format === 'audio' && audioFormats.length) {
      const current = dl.quality ? audioFormats.find(f => f.format_id === dl.quality) : null
      dl.quality = (current ?? audioFormats[0]).format_id
    }
  }

  // Пасхалка — muriko
  const isMuriko = $derived(
    /^(muriko|мурико|murico|мурик|murик|муrico)$/i.test(dl.url.trim())
  )
  $effect(() => {
    if (isMuriko) {
      const phrases: [string, string][] = [
        ['нашёл пасхалку.', 'pink'],
        ['привет, исследователь.', 'pink'],
        ['любопытный.', 'info'],
        ['это секрет.', 'pink'],
        ['muriko здесь был.', 'success'],
      ]
      const [t, c] = phrases[Math.floor(Math.random() * phrases.length)]
      emitThought(t, c, 2)
    }
  })

  // Пасхалка — груз 200
  const isGruz200 = $derived(
    /^(груз(\s*-?\s*200)?|gruz(\s*-?\s*200)?|200|g200|г200)$/i.test(dl.url.trim())
  )
  let g200video = $state<HTMLVideoElement | null>(null)
  $effect(() => {
    if (isGruz200) {
      emitThought('...', 'muted', 2)
      setTimeout(() => g200video?.play().catch(() => {}), 50)
    }
  })

  const rejected = $derived(dl.url && !urlValid && !dl.loading && !isMuriko && !isGruz200 ? classifyRejectedUrl(dl.url) : null)

  // Автозапуск анализа при изменении валидного URL
  $effect(() => {
    if (dl.url !== dl.lastFetchedUrl) dl.error = null
    if (!urlValid && dl.url) { dl.info = null; dl.quality = '' }
    if (urlValid && dl.url !== dl.lastFetchedUrl && !dl.loading) {
      dl.lastFetchedUrl = dl.url
      fetchInfo()
    }
  })

  // ── Действия ───────────────────────────────────────────────────────────────

  async function fetchInfo() {
    if (!canFetch) return
    const prevQuality = dl.quality
    dl.loading = true; dl.error = null; dl.info = null; dl.quality = ''; stopEmojiTimer()
    try {
      dl.info = await commands.fetchInfo(dl.url)
      dl.quality = prevQuality
      pickDefaultQuality()
    } catch (e: unknown) {
      dl.error = e instanceof Error ? e.message : String(e)
      startEmojiTimer()
      const phrases: [string, string][] = [
        ['не вышло.', 'error'], ['ошибка.', 'error'],
        ['что-то не так.', 'error'], ['не смог.', 'error'],
        ['ссылка сломана?', 'warning'], ['yt-dlp недоволен.', 'error'],
        ['увы.', 'error'], ['не получилось.', 'error'],
      ]
      const [t, c] = phrases[Math.floor(Math.random() * phrases.length)]
      emitThought(t, c, 2)
    } finally {
      dl.loading = false
    }
  }

  async function startDownload() {
    if (!canDownload || !dl.info) return
    dl.queuing = true
    try {
      await commands.startDownload({
        url: dl.info.url,
        format: dl.format,
        quality: dl.quality,
        fps: dl.fps,
        bitrate: dl.bitrate,
        container: dl.container,
        title: dl.info.title ?? null,
        thumbnail: dl.info.thumbnail ?? null,
        channel: dl.info.channel ?? null,
        duration: dl.info.duration ?? null,
        is_playlist: dl.info.is_playlist,
        audio_codec: dl.audioCodec,
        video_codec: dl.videoCodec,
      })
      dl.queued = true
      setTimeout(() => { dl.queued = false }, 2500)
    } finally {
      dl.queuing = false
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && canFetch) fetchInfo()
  }

  const PASTE_PHRASES: [string, string][] = [
    ['о, ссылка.', 'info'],
    ['вижу ссылку.', 'info'],
    ['youtube?', 'info'],
    ['принял.', 'muted'],
    ['смотрю...', 'info'],
    ['интересно.', 'pink'],
    ['что за видео?', 'info'],
  ]

  function onPaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData('text') ?? ''
    if (isValidYoutubeUrl(text)) {
      const [t, c] = PASTE_PHRASES[Math.floor(Math.random() * PASTE_PHRASES.length)]
      emitThought(t, c, 1)
    }
  }

  function clearUrl() {
    dl.url = ''; dl.info = null; dl.error = null; dl.quality = ''; dl.audioCodec = null; dl.videoCodec = null; dl.lastFetchedUrl = ''; lastRejectedUrl = ''; stopEmojiTimer()
  }

  function setFormat(f: string) {
    dl.format = f
    if (f === 'audio') {
      if (!['mp3','m4a','opus'].includes(dl.container)) dl.container = 'm4a'
    } else {
      if (!['mp4','mkv','webm','mov'].includes(dl.container)) dl.container = 'mp4'
    }
    if (dl.info) pickDefaultQuality()
  }

  const urlTypeLabels: Record<string, string> = {
    video: 'YouTube', short: 'Shorts', playlist: 'Плейлист',
    live: 'Live', music: 'YouTube Music',
  }

  const SAD_EMOJIS = ['(╥_╥)', '(T_T)', '(；＿；)', '(╯︵╰,)', '(｡•́︿•̀｡)', '(＞﹏＜)', 'o(TヘTo)', '( ˘︹˘ )']

  function pickNewEmoji() {
    const pool = SAD_EMOJIS.filter(e => e !== dl.currentEmoji)
    dl.currentEmoji = pool[Math.floor(Math.random() * pool.length)]
  }

  function startEmojiTimer() { pickNewEmoji() }
  function stopEmojiTimer() { }

  onDestroy(() => { })

  // Символы-глаза для каждого эмодзи
  const EYES: Record<string, Set<string>> = {
    '(╥_╥)':   new Set(['╥']),
    '(T_T)':   new Set(['T']),
    '(；＿；)': new Set(['；']),
    '(╯︵╰,)': new Set(['╯','╰']),
    '(｡•́︿•̀｡)': new Set(['•́','•̀']),
    '(＞﹏＜)': new Set(['＞','＜']),
    'o(TヘTo)': new Set(['T']),
    '( ˘︹˘ )': new Set(['˘']),
    '(◕‿◕)':   new Set(['◕']),
    '( T_T)':   new Set(['T']),
  }

  interface EmojiChar { char: string; isEye: boolean }

  function splitEmoji(s: string): EmojiChar[] {
    const eyes = EYES[s] ?? new Set()
    // Собираем с учётом комбинирующих символов (•́ = • + combining)
    const chars: EmojiChar[] = []
    const glyphs = [...s]
    let i = 0
    while (i < glyphs.length) {
      let g = glyphs[i]
      // Присоединяем следующий символ если он комбинирующий (U+0300–U+036F, U+0200–U+02FF)
      while (i + 1 < glyphs.length) {
        const code = glyphs[i + 1].codePointAt(0) ?? 0
        if ((code >= 0x0300 && code <= 0x036F) || (code >= 0x0200 && code <= 0x02FF)) {
          g += glyphs[++i]
        } else break
      }
      chars.push({ char: g, isEye: eyes.has(g) })
      i++
    }
    return chars
  }

  interface ErrorInfo { title: string; hint: string }

  function classifyError(msg: string): ErrorInfo {
    const m = msg.toLowerCase()
    if (m.includes('private') || m.includes('sign in') || m.includes('unavailable'))
      return { title: 'Видео недоступно', hint: 'Видео приватное или заблокировано в вашем регионе.' }
    if (m.includes('not found') || m.includes('does not exist') || m.includes('removed'))
      return { title: 'Видео не найдено', hint: 'Видео было удалено или ссылка неправильная.' }
    if (m.includes('age') || m.includes('confirm your age'))
      return { title: 'Возрастное ограничение', hint: 'Это видео доступно только авторизованным пользователям.' }
    if (m.includes('network') || m.includes('connection') || m.includes('timeout') || m.includes('timed out'))
      return { title: 'Нет соединения', hint: 'Проверьте интернет или попробуйте позже.' }
    if (m.includes('playlist'))
      return { title: 'Плейлист не поддерживается', hint: 'Вставьте ссылку на отдельное видео.' }
    if (m.includes('live'))
      return { title: 'Прямой эфир', hint: 'Запись прямых эфиров пока не поддерживается.' }
    if (m.includes('yt-dlp') || m.includes('ytdlp'))
      return { title: 'Ошибка загрузчика', hint: 'yt-dlp не смог обработать ссылку.' }
    return { title: 'Не удалось загрузить', hint: 'Попробуйте другую ссылку или обновите yt-dlp.' }
  }
</script>

<div class="page">
  <!-- URL bar -->
  <div class="url-bar" class:focused={false} class:valid={urlValid} class:loading={dl.loading}>
    <span class="url-icon" class:lit={urlValid}>
      {#if urlType !== 'unknown' && urlValid}
        {#if urlType === 'music'}
          <!-- YouTube Music логотип -->
          <svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.477 2 2 6.477 2 12s4.477 10 10 10 10-4.477 10-10S17.523 2 12 2zm0 14.5a4.5 4.5 0 1 1 0-9 4.5 4.5 0 0 1 0 9zm0-7a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z"/></svg>
        {:else}
          <!-- YouTube логотип -->
          <svg viewBox="0 0 24 24" fill="currentColor"><path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z"/></svg>
        {/if}
      {:else}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>
      {/if}
    </span>

    <input
      class="url-input"
      type="text"
      placeholder="Вставьте ссылку YouTube..."
      bind:value={dl.url}
      onkeydown={onKeydown}
      onpaste={onPaste}
      autocomplete="off"
      spellcheck="false"
      aria-label="Ссылка YouTube"
    />

    {#if dl.url}
      <button class="btn-clear" onclick={clearUrl} aria-label="Очистить">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    {/if}

    <button class="btn-fetch" onclick={fetchInfo} disabled={!canFetch} aria-label="Получить информацию">
      {#if dl.loading}
        <svg class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
      {:else}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      {/if}
    </button>
  </div>

  <!-- Muriko easter egg -->
  {#if isMuriko}
    <div class="error-full">
      <span class="error-emoji">
        {#each splitEmoji('(◕‿◕)') as {char, isEye}, i}
          {#if isEye}
            <span class="ec" style="animation-delay:{i * 0.06}s">
              <span class="ec-eye" style="animation-delay:{i * 0.06}s">{char}</span>
            </span>
          {:else}
            <span class="ec" style="animation-delay:{i * 0.06}s">{char}</span>
          {/if}
        {/each}
      </span>
      <p class="error-title" style="color:var(--thought-pink)">ты нашёл пасхалку</p>
      <p class="error-hint">это приложение сделал <strong>muriko</strong> — разработчик, который любит детали. если ты читаешь это, значит ты тоже любишь.</p>
    </div>
  {/if}

  <!-- Груз 200 easter egg -->
  {#if isGruz200}
    <div class="g200-wrap">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={g200video}
        src="/bg.mp4"
        class="g200-video"
        autoplay
        playsinline
        loop={false}
      ></video>
    </div>
  {/if}

  <!-- Rejected URL -->
  {#if rejected}
    <div class="error-full">
      {#key rejected.emoji}
        <span class="error-emoji">
          {#each splitEmoji(rejected.emoji) as {char, isEye}, i}
            {#if isEye}
              <span class="ec" style="animation-delay:{i * 0.06}s">
                <span class="ec-eye" style="animation-delay:{i * 0.06}s">{char}</span>
              </span>
            {:else}
              <span class="ec" style="animation-delay:{i * 0.06}s">{char}</span>
            {/if}
          {/each}
        </span>
      {/key}
      <p class="error-title">{rejected.title}</p>
      <p class="error-hint">{rejected.hint}</p>
    </div>
  {/if}

  <!-- Error -->
  {#if dl.error}
    {@const e = classifyError(dl.error)}
    <div class="error-full">
      {#key dl.currentEmoji}
        <span class="error-emoji">
          {#each splitEmoji(dl.currentEmoji) as {char, isEye}, i}
            {#if isEye}
              <span class="ec" style="animation-delay:{i * 0.06}s">
                <span class="ec-eye" style="animation-delay:{i * 0.06}s">{char}</span>
              </span>
            {:else}
              <span class="ec" style="animation-delay:{i * 0.06}s">{char}</span>
            {/if}
          {/each}
        </span>
      {/key}
      <p class="error-title">{e.title}</p>
      <p class="error-hint">{e.hint}</p>
      <button class="error-retry" onclick={fetchInfo}>попробовать снова</button>
    </div>
  {/if}

  <!-- Skeleton -->
  {#if dl.loading}
    <div class="preview-grid skeleton-grid">
      <div class="thumb-card">
        <div class="thumb-skeleton shimmer"></div>
      </div>
      <div class="info-card">
        <div class="sk-row">
          <div class="sk sk-avatar shimmer"></div>
          <div class="sk sk-name shimmer"></div>
        </div>
        <div class="sk sk-title shimmer"></div>
        <div class="sk sk-meta shimmer"></div>
      </div>
    </div>
    <div class="options-skeleton">
      <div class="sk sk-toggle shimmer"></div>
      <div class="sk-cards">
        {#each Array(4) as _}
          <div class="sk sk-card shimmer"></div>
        {/each}
      </div>
      <div class="sk sk-action shimmer"></div>
    </div>
  {/if}

  <!-- Видео карточка -->
  {#if dl.info && !dl.loading}
    <div class="preview-grid fadein">
      <!-- Превью -->
      <div class="thumb-card">
        <div class="thumb-wrap">
          {#if dl.info.thumbnail}
            <img src={dl.info.thumbnail} alt={dl.info.title} />
          {:else}
            <div class="thumb-placeholder"></div>
          {/if}
        </div>
        {#if dl.info.duration}
          <div class="duration-badge">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
            {formatDuration(dl.info.duration)}
          </div>
        {/if}
      </div>

      <!-- Правая колонка: мета + тогл -->
      <div class="right-col">
      <!-- Мета -->
      <div class="info-card">
        {#if dl.info.channel}
          <div class="channel-row">
            <div class="channel-avatar">
              {#if dl.info.channel_avatar}
                <img src={dl.info.channel_avatar} alt={dl.info.channel} onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display='none'; (e.currentTarget.nextElementSibling as HTMLElement).style.display='grid' }} />
                <span style="display:none">{dl.info.channel[0]}</span>
              {:else}
                {dl.info.channel[0]}
              {/if}
            </div>
            <div class="channel-info">
              <span class="channel-name">{dl.info.channel}</span>
              {#if dl.info.channel_followers}
                <span class="channel-subs">{formatSubs(dl.info.channel_followers)}</span>
              {/if}
            </div>
          </div>
        {/if}
        <p class="video-title">{dl.info.title}</p>
        {#if dl.info.is_playlist}
          <div class="playlist-warning">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
            Это плейлист — скачается только первое видео. Поддержка плейлистов скоро.
          </div>
        {/if}
        <div class="video-meta">
          {#if dl.info.is_playlist && dl.info.playlist_count}
            <span class="meta-badge">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
              {dl.info.playlist_count} видео
            </span>
          {/if}
        </div>
        <div class="info-actions">
          <button onclick={() => openUrl(dl.info!.url)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
            Открыть
          </button>
        </div>
      </div>

      <!-- Тогл под карточкой описания -->
      <div class="format-toggle">
        <button class:active={dl.format === 'video'} onclick={() => setFormat('video')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
          Видео со звуком
        </button>
        <button class:active={dl.format === 'video_only'} onclick={() => setFormat('video_only')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/><line x1="1" y1="22" x2="23" y2="2"/></svg>
          Только видео
        </button>
        <button class:active={dl.format === 'audio'} onclick={() => setFormat('audio')}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
          Только аудио
        </button>
      </div>
      </div><!-- end right-col -->
    </div>

    <!-- Опции -->
    <div class="options">
      <!-- Качество -->
      {#if dl.format === 'video' || dl.format === 'video_only'}
        <div class="quality-grid">
          {#each videoFormats as f (f.format_id)}
            <button
              class="quality-card"
              class:selected={bestFormatId(f.resolution) === dl.quality || allVideoFormats.filter(fmt => fmt.resolution === f.resolution).some(fmt => fmt.format_id === dl.quality)}
              onclick={() => { dl.quality = bestFormatId(f.resolution); dl.fps = null; dl.bitrate = null }}
            >
              <span class="q-value">{qualityLabel(f)}</span>
            </button>
          {/each}
        </div>

        <!-- FPS — только если есть варианты -->
        {#if maxFps > 0}
          <div class="fps-grid">
            <button
              class="quality-card"
              class:selected={dl.fps === null}
              onclick={() => dl.fps = null}
            >
              <span class="q-value">{maxFps}</span>
              <span class="q-desc">fps</span>
            </button>
            {#each fpsOptions.filter(f => f < maxFps) as f}
              <button
                class="quality-card"
                class:selected={dl.fps === f}
                onclick={() => { dl.fps = f; dl.bitrate = null }}
              >
                <span class="q-value">{f}</span>
                <span class="q-desc">fps</span>
              </button>
            {/each}
          </div>
        {/if}

        <!-- Контейнер -->
        <div class="container-grid">
          {#each ['mp4','webm','mkv','mov'] as c}
            <button
              class="quality-card"
              class:selected={dl.container === c}
              onclick={() => dl.container = c}
            >
              <span class="q-value">{c.toUpperCase()}</span>
            </button>
          {/each}
        </div>

        <!-- Видео-кодек -->
        {#if dl.format === 'video' || dl.format === 'video_only'}
        <div class="codec-cards-grid">
          {#each VIDEO_CODECS as codec}
            <button class="quality-card codec-card" class:selected={dl.videoCodec === codec.id} onclick={() => dl.videoCodec = codec.id}>
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
        {/if}

        <!-- Аудио-кодек — только при видео со звуком -->
        {#if dl.format === 'video'}
        <div class="codec-cards-grid">
          {#each AUDIO_CODECS as codec}
            <button class="quality-card codec-card" class:selected={dl.audioCodec === codec.id} onclick={() => dl.audioCodec = codec.id}>
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
        {/if}
      {:else}
        <div class="quality-grid">
          {#each audioFormats as f (f.format_id)}
            <button
              class="quality-card"
              class:selected={dl.quality === f.format_id}
              onclick={() => { dl.quality = f.format_id; dl.bitrate = null }}
            >
              <span class="q-value">{qualityLabel(f)}</span>
              <span class="q-desc">{qualityDesc(f)}</span>
            </button>
          {/each}
        </div>

        <div class="container-grid">
          {#each ['mp3','m4a','opus'] as c}
            <button
              class="quality-card"
              class:selected={dl.container === c}
              onclick={() => dl.container = c}
            >
              <span class="q-value">{c.toUpperCase()}</span>
            </button>
          {/each}
        </div>

        <!-- Аудио-кодек -->
        <div class="codec-cards-grid">
          {#each AUDIO_CODECS as codec}
            <button class="quality-card codec-card" class:selected={dl.audioCodec === codec.id} onclick={() => dl.audioCodec = codec.id}>
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
      {/if}

      <!-- Битрейт -->
      {#if maxBitrate > 0}
        {@const qualityLabel = currentBitrate >= maxBitrate * 0.8 ? 'высокое' : currentBitrate >= maxBitrate * 0.4 ? 'среднее' : 'низкое'}
        {@const estSize = estimatedSize ? Math.round(estimatedSize / maxBitrate * currentBitrate / 1024 / 1024) : null}
        <div class="bitrate-card">
          <div class="bitrate-tooltip" style="left: clamp(60px, {((maxBitrate - currentBitrate) / (maxBitrate - minBitrate) * 100).toFixed(1)}%, calc(100% - 60px))">
            <span class="bt-value">{currentBitrate} kbps</span>
            <span class="bt-quality">{qualityLabel}</span>
            {#if estSize}<span class="bt-size">~{estSize} MB</span>{/if}
          </div>
          {#key `${maxBitrate}-${minBitrate}`}
          <input
            class="bitrate-slider"
            type="range"
            min={minBitrate}
            max={maxBitrate}
            step={Math.max(1, Math.round(maxBitrate / 100))}
            value={maxBitrate - currentBitrate + minBitrate}
            oninput={(e) => { const inv = maxBitrate - +(e.target as HTMLInputElement).value + minBitrate; dl.bitrate = inv >= maxBitrate ? null : inv }}
          />
          {/key}
        </div>
      {/if}

      <!-- Кнопка скачать -->
      <div class="action-row">
        <div class="size-info">
          <div class="size-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          </div>
          <span class="size-badge size-badge-main">{estimatedSize ? formatBytes(estimatedSize) : '—'}</span>
          {#if freeSpace !== null}
            <span class="free-space">{formatBytes(freeSpace)} свободно</span>
          {/if}
          {#if dl.quality}
            <div class="size-badges">
              <span class="size-badge" data-tooltip={resTip}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><polyline points="8 21 12 17 16 21"/></svg>
                {qualityLabel(selectedFormat!)}
              </span>
              <span class="size-badge" data-tooltip={codecTip}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
                {dl.videoCodec ? VIDEO_CODECS.find(c => c.id === dl.videoCodec)?.label : qualityDesc(selectedFormat!)}
              </span>
              {#if dl.audioCodec}
                <span class="size-badge" data-tooltip="аудио кодек">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                  {AUDIO_CODECS.find(c => c.id === dl.audioCodec)?.label ?? dl.audioCodec}
                </span>
              {/if}
              {#if maxFps > 0}
                <span class="size-badge" data-tooltip="кадров в секунду">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/><line x1="17" y1="7" x2="22" y2="7"/><line x1="17" y1="17" x2="22" y2="17"/><line x1="2" y1="17" x2="7" y2="17"/></svg>
                  {dl.fps ?? maxFps} fps
                </span>
              {/if}
              {#if dl.info?.duration}
                <span class="size-badge" data-tooltip="длительность видео">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a10 10 0 1 0 0 20A10 10 0 0 0 12 2"/><polyline points="12 6 12 12 16 14"/></svg>
                  {formatDuration(dl.info.duration)}
                </span>
              {/if}
              <span class="size-badge" data-tooltip="битрейт видео">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                {dl.bitrate !== null ? dl.bitrate : maxBitrate} kbps
              </span>
              <span class="size-badge" data-tooltip={containerTip}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                {dl.container.toUpperCase()}
              </span>
            </div>
          {/if}
        </div>

        <button
          class="btn-download"
          onclick={startDownload}
          disabled={!canDownload}
        >
          {#if dl.queued}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
            В очереди
          {:else if dl.queuing}
            <svg class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
            Добавление...
          {:else}
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
            Скачать
          {/if}
        </button>
      </div>
    </div>
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

  /* ── URL bar ── */
  .url-bar {
    display: flex;
    align-items: center;
    height: 52px;
    background: #1e1e1e;
    border: 1.5px solid #333;
    border-radius: 14px;
    padding: 0 6px 0 16px;
    transition: border-color 0.2s, box-shadow 0.2s;
  }
  .url-bar:focus-within {
    border-color: #ff3d3d;
    box-shadow: 0 0 0 3px rgba(255,61,61,0.12);
  }
  .url-icon {
    flex-shrink: 0; width: 18px; height: 18px;
    display: grid; place-items: center;
    color: #666; transition: color 0.2s, filter 0.2s; pointer-events: none;
  }
  .url-icon svg { width: 20px; height: 20px; }
  .url-icon.lit { color: #ff3d3d; filter: drop-shadow(0 1px 3px rgba(255,61,61,0.35)) drop-shadow(0 3px 8px rgba(255,61,61,0.15)); }
  .url-input {
    flex: 1; height: 100%;
    background: transparent; border: none; outline: none;
    color: #f0f0f0; font-size: 13px; padding: 0 12px;
    min-width: 0; caret-color: #ff3d3d;
    letter-spacing: -0.01em; text-align: center;
  }
  .url-input::placeholder { color: #555; }
  .btn-clear {
    width: 30px; height: 30px; display: grid; place-items: center;
    background: none; border: none; border-radius: 7px;
    color: #555; cursor: pointer; flex-shrink: 0;
    transition: background 0.15s, color 0.15s;
  }
  .btn-clear svg { width: 12px; height: 12px; }
  .btn-clear:hover { background: rgba(255,255,255,0.08); color: #aaa; }
  .btn-fetch {
    width: 38px; height: 38px; margin-left: 4px; flex-shrink: 0;
    display: grid; place-items: center;
    background: linear-gradient(135deg, #ff3d3d, #ff6b3d);
    border: none; border-radius: 10px; color: #fff; cursor: pointer;
    transition: filter 0.2s, transform 0.15s, box-shadow 0.2s;
  }
  .btn-fetch svg { width: 14px; height: 14px; }
  .btn-fetch:hover:not(:disabled) {
    filter: brightness(1.1);
    box-shadow: 0 4px 18px rgba(255,61,61,0.4);
    transform: translateY(-1px);
  }
  .btn-fetch:active:not(:disabled) { transform: scale(0.96); }
  .btn-fetch:disabled { opacity: 0.25; cursor: default; }

  /* ── Error full-page ── */
  .g200-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  .g200-video {
    max-width: 100%;
    max-height: 100%;
    border-radius: 12px;
    outline: none;
  }

  .error-full {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 40px;
  }
  .error-emoji {
    font-size: 64px;
    font-family: var(--font-mono);
    color: var(--text-secondary);
    letter-spacing: -3px;
    line-height: 1;
    display: inline-flex;
  }
  .ec {
    display: inline-block;
    animation: charDrop 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) both, charFloat 3s ease-in-out infinite;
  }
  @keyframes charDrop {
    from { opacity: 0; transform: translateY(-20px) scale(0.6); }
    to   { opacity: 1; transform: none; }
  }
  @keyframes charFloat {
    0%, 100% { transform: translateY(0); }
    50%       { transform: translateY(-4px); }
  }
  .ec-eye {
    display: inline-block;
    animation: blink 4s 1s ease-in-out infinite;
    transform-origin: center 60%;
  }
  @keyframes blink {
    0%, 88%, 100% { transform: scaleY(1); }
    92%            { transform: scaleY(0.08); }
  }
  .error-title {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--thought-error);
    animation: fadeUp 0.35s 0.1s ease both;
  }
  .error-hint {
    margin: 0;
    font-size: 13px;
    color: var(--text-primary);
    text-align: center;
    max-width: 300px;
    line-height: 1.6;
    animation: fadeUp 0.35s 0.18s ease both;
  }
  @keyframes fadeUp {
    from { opacity: 0; transform: translateY(6px); }
    to   { opacity: 1; transform: none; }
  }
  .error-retry {
    margin-top: 8px;
    padding: 9px 22px;
    background: rgba(255,255,255,0.055);
    border: none;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    animation: fadeUp 0.35s 0.26s ease both;
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.12),
      inset 0 -1px 0 rgba(0,0,0,0.2);
  }
  .error-retry:hover {
    background: rgba(255,255,255,0.09);
    color: var(--text-primary);
  }

  /* ── Shimmer skeleton ── */
  .shimmer {
    position: relative; overflow: hidden; background: #252525;
  }
  .shimmer::after {
    content: ''; position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.035) 45%, rgba(255,255,255,0.055) 50%, rgba(255,255,255,0.035) 55%, transparent 100%);
    animation: shimmer 2.2s infinite;
  }
  @keyframes shimmer { 0% { transform: translateX(-100%); } 100% { transform: translateX(100%); } }

  .skeleton-grid { margin-top: 16px; }
  .thumb-skeleton { width: 100%; aspect-ratio: 16/9; border-radius: 14px; }
  .sk { border-radius: 6px; }
  .sk-row { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
  .sk-avatar { width: 26px; height: 26px; border-radius: 50%; flex-shrink: 0; }
  .sk-name  { height: 12px; width: 110px; }
  .sk-title { height: 18px; width: 88%; margin-bottom: 8px; }
  .sk-meta  { height: 12px; width: 50%; }
  .options-skeleton { display: flex; flex-direction: column; gap: 10px; margin-top: 12px; }
  .sk-toggle { height: 44px; border-radius: 11px; }
  .sk-cards  { display: grid; grid-template-columns: repeat(4,1fr); gap: 8px; }
  .sk-card   { height: 88px; border-radius: 12px; }
  .sk-action { height: 58px; border-radius: 12px; }

  /* ── Preview ── */
  .preview-grid {
    display: grid;
    grid-template-columns: 5fr 6fr;
    gap: 14px;
    margin-top: 16px;
    isolation: isolate;
    align-items: stretch;
  }
  .right-col { display: flex; flex-direction: column; justify-content: space-between; gap: 10px; }
  .fadein { animation: fadein 0.3s cubic-bezier(0.4,0,0.2,1); }
  @keyframes fadein { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }

  /* Превью */
  .thumb-card {
    border-radius: 14px; overflow: visible; position: relative;
    background: #1e1e1e;
    box-shadow: 0 4px 16px rgba(0,0,0,0.3);
  }
  .thumb-wrap { width: 100%; aspect-ratio: 16/9; background: #252525; overflow: hidden; border-radius: 14px; }
  .thumb-card::after { content: ''; position: absolute; inset: 0; border-radius: 14px; box-shadow: inset 0 1px 0 rgba(0,0,0,0.4), inset 0 -1px 0 rgba(0,0,0,0.4); pointer-events: none; z-index: 1; }
  .thumb-wrap img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .thumb-placeholder { width: 100%; height: 100%; background: #252525; }
  .duration-badge {
    position: absolute; bottom: 10px; right: 10px;
    display: inline-flex; align-items: center; gap: 4px;
    padding: 4px 9px;
    background: rgba(0,0,0,0.82);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    font-size: 11px; font-weight: 600; color: #ddd;
    font-variant-numeric: tabular-nums;
    backdrop-filter: blur(6px);
    letter-spacing: 0.02em;
  }
  .duration-badge svg { width: 10px; height: 10px; opacity: 0.5; }

  /* Инфо */
  .info-card {
    background: #1e1e1e;
    border: 1px solid #2e2e2e;
    border-radius: 14px;
    padding: 20px 22px;
    flex: 1;
    display: flex; flex-direction: column; gap: 10px; align-items: flex-start;
    min-width: 0;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  }
  .channel-row { display: inline-flex; align-items: center; gap: 8px; padding: 6px 8px; margin: -6px -8px; border-radius: 10px; cursor: pointer; transition: background 0.2s, box-shadow 0.2s; }
  .channel-row:hover { background: rgba(0,0,0,0.35); box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 1px 0 rgba(120,120,120,0.5); }
  .channel-avatar {
    width: 36px; height: 36px; border-radius: 50%;
    background: #2a2a2a;
    display: grid; place-items: center; overflow: hidden;
    font-size: 13px; font-weight: 700; color: #fff; flex-shrink: 0;
  }
  .channel-avatar img { width: 100%; height: 100%; object-fit: cover; display: block; outline: none; border-radius: 50%; }
  .channel-info { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .channel-name {
    font-size: 13px; font-weight: 600; color: #aaa;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .channel-subs { font-size: 11px; color: #555; font-weight: 500; }
  .video-title {
    font-size: 15px; font-weight: 600; line-height: 1.45;
    color: #f0f0f0; margin: 0;
    display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3;
    -webkit-box-orient: vertical; overflow: hidden;
    letter-spacing: -0.01em;
  }
  .playlist-warning {
    display: flex; align-items: flex-start; gap: 6px;
    padding: 7px 10px; border-radius: 8px;
    background: rgba(245,158,11,0.08); border: 1px solid rgba(245,158,11,0.25);
    font-size: 11px; color: #f59e0b; line-height: 1.4;
  }
  .playlist-warning svg { width: 12px; height: 12px; flex-shrink: 0; margin-top: 1px; }
  .video-meta { display: flex; gap: 8px; flex-wrap: wrap; }
  .meta-badge {
    display: inline-flex; align-items: center; gap: 4px;
    padding: 2px 8px; background: transparent;
    border: 1px solid #333; border-radius: 4px;
    font-size: 12px; font-weight: 600; color: #666;
    letter-spacing: 0.04em; text-transform: uppercase;
  }
  .meta-badge svg { width: 10px; height: 10px; }
  .info-actions { display: flex; gap: 8px; margin-top: auto; padding-top: 6px; }
  .info-actions button {
    display: inline-flex; align-items: center; gap: 5px;
    height: 28px; padding: 0 11px;
    background: #252525; border: 1px solid #333;
    border-radius: 7px; color: #888;
    font-size: 11px; font-weight: 500; cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }
  .info-actions button svg { width: 11px; height: 11px; }
  .info-actions button:hover { background: #2e2e2e; color: #f0f0f0; border-color: #444; }

  /* ── Options ── */
  .options { display: flex; flex-direction: column; gap: 14px; margin-top: 6px; flex: 1; }

  /* Format toggle */
  .format-toggle {
    display: flex;
    background: #1e1e1e; border: 1px solid #2e2e2e;
    border-radius: 11px; padding: 3px;
  }
  .format-toggle button {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 7px;
    height: 36px; background: transparent; border: none; border-radius: 8px;
    color: #666; font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background 0.2s, color 0.2s;
  }
  .format-toggle button svg { width: 13px; height: 13px; opacity: 0.5; }
  .format-toggle button:hover { color: #ccc; }
  .format-toggle button.active {
    background: rgba(0,0,0,0.35); color: #f0f0f0; font-weight: 600;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
  }
  .format-toggle button.active svg { opacity: 1; color: #ff3d3d; }

  /* Quality cards */
  .quality-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(84px, 1fr));
    gap: 8px;
  }
  .fps-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(68px, 1fr));
    gap: 8px;
  }
  .container-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(68px, 1fr));
    gap: 8px;
  }


  .quality-card {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 5px; min-height: 84px; padding: 14px 10px;
    background: #1e1e1e; border: 1px solid #2e2e2e;
    border-radius: 12px; cursor: pointer;
    transition: border-color 0.18s, background 0.18s, transform 0.12s, box-shadow 0.18s;
  }
  .quality-card:hover {
    border-color: #3e3e3e; background: #252525;
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(0,0,0,0.3);
  }
  .quality-card:active { transform: translateY(0) scale(0.96); }
  .quality-card.selected {
    border-color: transparent;
    background: rgba(0,0,0,0.35);
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
    transform: none;
  }
  .quality-card.selected .q-value { color: #ff3d3d; }
  .q-value {
    font-size: 18px; font-weight: 700; letter-spacing: -0.03em;
    color: #f0f0f0; line-height: 1;
  }
  .q-desc {
    font-size: 9px; font-weight: 600; color: #444;
    letter-spacing: 0.04em; text-transform: uppercase;
    text-align: center; line-height: 1.3;
  }


  /* Container cards (меньше) */
  .container-grid .quality-card { min-height: 56px; padding: 10px; }
  .fps-grid .quality-card { min-height: 56px; padding: 10px; }
  .container-grid .q-value { font-size: 13px; font-weight: 700; letter-spacing: 0.03em; }

  /* Action row */
  .action-row {
    display: grid; grid-template-columns: 1fr auto;
    gap: 10px; align-items: stretch; margin-top: auto;
  }
  .size-info {
    display: flex; align-items: center; gap: 14px;
    background: #1e1e1e; border: 1px solid #2e2e2e;
    border-radius: 12px; padding: 0 16px; overflow: visible;
  }
  .size-icon {
    width: 32px; height: 32px; border-radius: 5px;
    background: rgba(0,0,0,0.35); border: 1px solid transparent;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03);
    display: grid; place-items: center; flex-shrink: 0;
  }
  .size-icon svg { width: 16px; height: 16px; color: #ff3d3d; opacity: 0.65; }
  .size-badge[data-tooltip] { position: relative; }
  .size-badge[data-tooltip]::after {
    content: attr(data-tooltip);
    position: absolute; bottom: calc(100% + 8px); left: 50%;
    transform: translateX(-50%) translateY(4px);
    background: #1e1e1e; border: 1px solid #333;
    border-radius: 8px; padding: 8px 12px;
    font-size: 11px; font-weight: 500; color: #f0f0f0;
    white-space: nowrap; pointer-events: none; opacity: 0;
    transition: opacity 0.15s, transform 0.15s;
    box-shadow: 0 4px 16px rgba(0,0,0,0.5);
    z-index: 100; text-transform: none; letter-spacing: 0;
  }
  .size-badge[data-tooltip]:hover::after {
    opacity: 1; transform: translateX(-50%) translateY(0);
  }
  .size-badge svg { width: 11px; height: 11px; opacity: 0.6; flex-shrink: 0; }
  .size-badge-main { font-size: 16px !important; font-weight: 700 !important; letter-spacing: -0.01em !important; color: rgba(255,255,255,1) !important; background: rgba(0,0,0,0.35) !important; padding: 5px 12px !important; box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03) !important; }
  .free-space { font-size: 10px; color: rgba(255,255,255,0.3); font-weight: 500; letter-spacing: 0.02em; cursor: pointer; transition: color 0.15s; }
  .free-space:hover { color: rgba(255,255,255,0.6); }
  .size-badges { display: flex; gap: 5px; margin-left: auto; flex-shrink: 0; }
  .size-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; font-weight: 600; color: rgba(255,255,255,0.4); background: transparent; border: 1px solid transparent; border-radius: 5px; padding: 5px 4px; letter-spacing: 0.04em; text-transform: uppercase; transition: all 0.2s; box-shadow: none; }
  .size-badge:hover { color: rgba(255,255,255,1); background: rgba(0,0,0,0.35); padding: 5px 12px; box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03); }

  .btn-download {
    height: 58px; padding: 0 36px;
    display: flex; align-items: center; gap: 10px;
    background: #2a2a2a;
    border: 1px solid rgba(0,0,0,1); border-radius: 12px;
    color: #fff; font-size: 14px; font-weight: 700;
    cursor: pointer; white-space: nowrap; letter-spacing: 0.02em;
    transition: filter 0.2s, box-shadow 0.2s, transform 0.15s;
    position: relative;
    box-shadow: inset 0 1px 0 rgba(120,120,120,0.5), inset 0 -1px 0 rgba(80,80,80,0.15);
  }
  .btn-download svg { width: 15px; height: 15px; }
  .btn-download:hover:not(:disabled) {
    background: rgba(255,61,61,0.13);
    border-color: rgba(255,61,61,0.8);
    box-shadow: 0 0 20px rgba(255,61,61,0.15), inset 0 1px 0 rgba(255,61,61,0.15);
    transform: translateY(-1px);
  }
  .btn-download:active:not(:disabled) { transform: scale(0.97); }
  .btn-download:disabled { opacity: 0.25; cursor: default; box-shadow: none; }

  /* Битрейт */
  .bitrate-card { padding: 4px 0; position: relative; }
  .bitrate-card:hover .bitrate-tooltip { opacity: 1; transform: translateX(-50%) translateY(0); }
  .bitrate-tooltip {
    position: absolute; bottom: calc(100% + 8px);
    transform: translateX(-50%) translateY(4px);
    background: #1e1e1e; border: 1px solid #333;
    border-radius: 8px; padding: 8px 12px;
    display: flex; flex-direction: column; gap: 2px;
    pointer-events: none; opacity: 0;
    transition: opacity 0.15s, transform 0.15s;
    white-space: nowrap; z-index: 10;
    box-shadow: 0 4px 16px rgba(0,0,0,0.5);
  }
  .bt-value { font-size: 13px; font-weight: 700; color: #f0f0f0; font-variant-numeric: tabular-nums; }
  .bt-quality { font-size: 10px; font-weight: 500; color: #666; text-transform: uppercase; letter-spacing: 0.05em; }
  .bt-size { font-size: 11px; color: #888; }
  .bitrate-slider {
    -webkit-appearance: none; appearance: none;
    width: 100%; height: 4px; border-radius: 2px;
    background: #2e2e2e; outline: none; cursor: pointer;
  }
  .bitrate-slider::-webkit-slider-thumb {
    -webkit-appearance: none; appearance: none;
    width: 16px; height: 16px; border-radius: 50%;
    background: #ff3d3d; cursor: pointer;
    box-shadow: 0 0 0 3px rgba(255,61,61,0.2);
    transition: box-shadow 0.15s;
  }
  .bitrate-slider::-webkit-slider-thumb:hover {
    box-shadow: 0 0 0 5px rgba(255,61,61,0.25);
  }
  /* Пикеры кодеков */
  
  
  
  
  
  
  
  
  
  
  
  
  
  

  /* Animations */
  .spin { animation: spin 0.75s linear infinite; }
  @keyframes spin    { to { transform: rotate(360deg); } }
  @keyframes pulse   { 0%,100% { opacity: 1; } 50% { opacity: 0.35; } }

  .codec-cards-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 8px; }
  .codec-card { min-height: 34px !important; padding: 8px !important; }
  .codec-card .q-value { font-size: 13px; font-weight: 700; letter-spacing: 0.03em; }
  
  ::-webkit-scrollbar { width: 2px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }
</style>
