<script lang="ts">
  import { commands } from '$lib/bridge/commands'
  import { isValidYoutubeUrl, detectUrlType, classifyRejectedUrl } from '$lib/utils/url'
  import { formatDuration, formatBytes } from '$lib/utils/format'
  import { store, updateSetting } from '$lib/stores/settings.svelte'
  import { dl } from '$lib/stores/download.svelte'
  import { tooltip } from '$lib/utils/tooltip'

  import type { Route } from '$lib/bridge/types'

  let { route = $bindable<Route>('download') } = $props()

  // Свободное место на диске загрузки
  let freeSpace = $state<number | null>(null)
  $effect(() => {
    const path = store.settings ? store.settings.download_dir : '.'
    commands.getFreeSpace(path).then(n => { freeSpace = n }).catch(() => { freeSpace = null })
  })

  async function pickDownloadDir() {
    const dir = await commands.pickDirectory(store.settings?.download_dir)
    if (dir) await updateSetting('download_dir', dir)
  }
  import { emitThought } from '$lib/stores/thought.svelte'
  import { onMount } from 'svelte'
  import type { VideoInfo, VideoFormat } from '$lib/bridge/types'

  // ── Состояние живёт в сторе — сохраняется при навигации ───────────────────

  onMount(() => {
    if (store.settings && !dl.info && !dl.url) {
      dl.format       = store.settings.default_format
      dl.container    = store.settings.default_container
      dl.fps          = store.settings.default_fps ?? null
      dl.bitrate      = store.settings.default_bitrate ?? null
      dl.videoCodec   = store.settings.default_video_codec ?? null
      dl.audioCodec   = store.settings.default_audio_codec ?? null
    }
  })

  // Автосохранение параметров как дефолтов при каждом изменении (с дебаунсом)
  const saveTimers = new Map<string, ReturnType<typeof setTimeout>>()
  function debouncedSaveSetting(key: string, value: string) {
    clearTimeout(saveTimers.get(key))
    saveTimers.set(key, setTimeout(() => updateSetting(key as 'default_format', value), 300))
  }
  $effect(() => {
    const f = dl.format, c = dl.container, fps = dl.fps,
          br = dl.bitrate, vc = dl.videoCodec, ac = dl.audioCodec
    if (!store.settings || dl.loading) return
    debouncedSaveSetting('default_format', f)
    debouncedSaveSetting('default_container', c)
    debouncedSaveSetting('default_fps', fps != null ? String(fps) : '')
    debouncedSaveSetting('default_bitrate', br != null ? String(br) : '')
    debouncedSaveSetting('default_video_codec', vc ?? '')
    debouncedSaveSetting('default_audio_codec', ac ?? '')
  })

  const urlType    = $derived(detectUrlType(dl.url))
  const urlValid   = $derived(isValidYoutubeUrl(dl.url))
  const canFetch   = $derived(urlValid && !dl.loading)
  const canDownload = $derived(
    !!dl.info && !!dl.quality
    && !dl.queuing && !dl.queued
    && (dl.format === 'audio' ? !!dl.audioCodec : !!dl.videoCodec)
    && (dl.format === 'video' ? !!dl.audioCodec : true)
    && (dl.format === 'audio' || dl.fps !== null || (() => {
      if (!dl.quality) return true
      const res = (dl.info?.formats ?? []).find(f => f.format_id === dl.quality)?.resolution ?? null
      if (!res) return true
      const fps = Math.max(...(dl.info?.formats ?? []).filter(f => f.resolution === res).map(f => f.fps ?? 0))
      return fps <= 0
    })())
  )

  let downloadShake = $state(false)
  let shakeQuality = $state(false)

  function triggerShake() {
    downloadShake = true
    if (!dl.quality) shakeQuality = true
    setTimeout(() => { downloadShake = false; shakeQuality = false }, 500)
  }

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
        const ha = parseInt(a.resolution?.split('x')[1] ?? '0')
        const hb = parseInt(b.resolution?.split('x')[1] ?? '0')
        if (ha !== hb) return hb - ha
        // Внутри одного разрешения: предпочитаем по приоритету кодека
        const prio = (c: string | null) => {
          const v = (c ?? '').toLowerCase()
          if (v.startsWith('av01')) return 0  // AV1 — лучшее сжатие
          if (v.startsWith('vp09')) return 1  // VP9
          if (v.startsWith('hev'))  return 2  // H.265
          if (v.startsWith('avc'))  return 3  // H.264
          if (v.startsWith('pro'))  return 4  // ProRes
          return 5
        }
        return prio(a.vcodec) - prio(b.vcodec)
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

  // format_id лучшего формата для данного разрешения с учётом выбранного кодека
  function bestFormatId(resolution: string | null): string {
    const codecPref = dl.videoCodec
    const prefix = codecPref ? codecPrefix(codecPref) : null
    if (prefix) {
      const match = allVideoFormats.find(f => f.resolution === resolution && f.vcodec?.toLowerCase().startsWith(prefix))
      if (match) return match.format_id
    }
    return allVideoFormats.find(f => f.resolution === resolution)?.format_id ?? ''
  }

  // Маппинг ID кодека → префикс vcodec из yt-dlp
  function codecPrefix(id: string): string {
    switch (id) {
      case 'h264': return 'avc'
      case 'h265': return 'hev'
      case 'vp9':  return 'vp09'
      case 'av1':  return 'av01'
      case 'prores': return 'pro'
      default: return ''
    }
  }

  // Каталоги кодеков
  const h = (s: string) => s  // псевдоним для html-строк — подсветка в IDE

  type CodecEntry = { id: string; label: string; lib: string; lines: string[] }
  let codecTipData = $state<{ codec: CodecEntry; rect: DOMRect } | null>(null)

  function onCodecEnter(e: MouseEvent, codec: CodecEntry) {
    codecTipData = { codec, rect: (e.currentTarget as HTMLElement).getBoundingClientRect() }
  }
  function onCodecLeave() { codecTipData = null }

  const VIDEO_CODECS = [
    { id: 'h264',   label: 'H.264',  lib: 'libx264',    lines: [
      h('Самый совместимый кодек. Работает везде.'),
      h('Быстрое кодирование, умеренный размер файла.'),
    ]},
    { id: 'h265',   label: 'H.265',  lib: 'libx265',    lines: [
      h('На <em>30–50%</em> меньше размер при том же качестве.'),
      h('Аппаратное декодирование на <em>Apple Silicon, Intel 6+, NVIDIA Maxwell+</em>.'),
      h('<warn>Не поддерживается в Firefox и старых плеерах.</warn>'),
    ]},
    { id: 'vp9',    label: 'VP9',    lib: 'libvpx-vp9', lines: [
      h('Открытый кодек Google. Используется на <em>YouTube</em>.'),
      h('Эффективность сравнима с H.265.'),
      h('<warn>Программное кодирование медленнее H.264.</warn>'),
    ]},
    { id: 'av1',    label: 'AV1',    lib: 'libaom-av1', lines: [
      h('На <em>20–30%</em> эффективнее VP9 и H.265.'),
      h('<warn>Кодирование в 10–50× медленнее H.264.</warn>'),
    ]},
    { id: 'prores', label: 'ProRes', lib: 'prores_ks',   lines: [
      h('Intra-frame кодек Apple для нелинейного монтажа.'),
      h('Большой файл, минимальная нагрузка при декодировании.'),
      h('<warn>Только контейнер MOV.</warn>'),
    ]},
  ]

  const AUDIO_CODECS = [
    { id: 'aac',  label: 'AAC',  lib: 'aac',        lines: [
      h('Lossy кодек, стандарт для <em>MP4/MOV</em>.'),
      h('Хорошее качество от <em>128 kbps</em>. Поддерживается везде.'),
    ]},
    { id: 'opus', label: 'Opus', lib: 'libopus',     lines: [
      h('Лучший lossy кодек при битрейте до <em>96 kbps</em>.'),
      h('<warn>Только MKV/WebM — не поддерживается в MP4/MOV.</warn>'),
    ]},
    { id: 'mp3',  label: 'MP3',  lib: 'libmp3lame',  lines: [
      h('Устаревший lossy кодек. Воспроизводится буквально везде.'),
      h('<warn>Уступает AAC при том же битрейте.</warn>'),
    ]},
    { id: 'flac', label: 'FLAC', lib: 'flac',        lines: [
      h('Lossless — без потерь качества.'),
      h('Файл в <em>3–5×</em> больше AAC. Только <em>MKV/MOV</em>.'),
    ]},
    { id: 'ac3',  label: 'AC3',  lib: 'ac3',         lines: [
      h('Dolby Digital — до <em>640 kbps</em>, до <em>5.1</em> каналов.'),
      h('Стандарт DVD и Blu-ray. Поддерживается в MKV/MOV/MP4.'),
    ]},
  ]

  // Матрица совместимости: какие кодеки поддерживает каждый контейнер
  const VIDEO_COMPAT: Record<string, string[]> = {
    mp4:  ['h264', 'h265', 'av1'],
    mkv:  ['h264', 'h265', 'vp9', 'av1'],
    webm: ['vp9', 'av1'],
    mov:  ['h264', 'h265', 'prores'],
  }
  const AUDIO_COMPAT: Record<string, string[]> = {
    mp4:  ['aac', 'mp3', 'ac3'],
    mkv:  ['aac', 'opus', 'mp3', 'flac', 'ac3'],
    webm: ['opus'],
    mov:  ['aac', 'mp3', 'flac', 'ac3'],
  }

  function isVideoCodecOk(codecId: string | null) {
    if (!codecId) return true
    return (VIDEO_COMPAT[dl.container] ?? []).includes(codecId)
  }
  function isAudioCodecOk(codecId: string | null) {
    if (!codecId) return true
    return (AUDIO_COMPAT[dl.container] ?? []).includes(codecId)
  }

  // При смене контейнера — сбросить несовместимые кодеки и подставить первый подходящий
  $effect(() => {
    const c = dl.container
    let reset = false
    if (dl.videoCodec && !(VIDEO_COMPAT[c] ?? []).includes(dl.videoCodec)) {
      dl.videoCodec = null; reset = true
    }
    if (dl.audioCodec && !(AUDIO_COMPAT[c] ?? []).includes(dl.audioCodec)) {
      dl.audioCodec = null; reset = true
    }
    if (reset) pickDefaultCodecs()
  })

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
    maxFps > 0 ? [60, 30, 24].filter(f => f <= maxFps) : []
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
    dl.videoCodec
      ? (VIDEO_CODECS.find(c => c.id === dl.videoCodec)?.lines
          .map(l => l.replace(/<[^>]+>/g, ''))
          .join(' ') ?? '')
      : (() => {
          const c = selectedFormat ? qualityDesc(selectedFormat) : ''
          if (c === 'AV1') return 'AV1 — наилучшее сжатие. Поддержка растёт, но не все плееры декодируют аппаратно.'
          if (c === 'VP9') return 'VP9 — используется на YouTube. Широкая поддержка в браузерах.'
          return 'AVC/H.264 — максимальная совместимость. Поддерживается везде.'
        })()
  )
  const containerTip = $derived(
    dl.container === 'mov' ? 'MOV (QuickTime) — контейнер Apple. Требуется для ProRes. Поддерживается в macOS, iOS, Adobe CC.' :
    dl.container === 'mkv' ? 'MKV (Matroska) — открытый контейнер без ограничений по кодекам. Не поддерживается на некоторых SmartTV и консолях.' :
    dl.container === 'webm' ? 'WebM — открытый контейнер Google для VP9/AV1+Opus. Нативно в браузерах, не поддерживается в большинстве плееров.' :
    'MP4 (MPEG-4 Part 14) — универсальный контейнер. Поддерживается везде. Только H.264/H.265/AV1 + AAC/MP3/AC3.'
  )
  const resTip = $derived(`${selectedFormat?.resolution ?? ''} · ${selectedFormat ? qualityLabel(selectedFormat) : ''}`)

  const maxBitrate = $derived(
    selectedFormat
      ? Math.round(selectedFormat.is_audio_only
          ? (selectedFormat.abr ?? 0)
          : (selectedFormat.vbr ?? 0))
      : 0
  )
  const currentBitrate = $derived(dl.bitrate ?? maxBitrate)
  // Минимальный осмысленный битрейт: для аудио ≥32 kbps, для видео ≥100 kbps
  const minBitrate = $derived(
    maxBitrate > 0
      ? (selectedFormat?.is_audio_only ? Math.max(32, Math.round(maxBitrate * 0.1)) : Math.max(100, Math.round(maxBitrate * 0.05)))
      : 1
  )

  // Защита: сбрасываем bitrate если он вдруг превышает maxBitrate
  $effect(() => {
    if (dl.bitrate !== null && maxBitrate > 0 && dl.bitrate > maxBitrate) {
      dl.bitrate = null
    }
  })

  function formatSubs(n: number): string {
    const fmt = (v: number, dec = 1) => v.toLocaleString('ru-RU', { maximumFractionDigits: dec, minimumFractionDigits: dec })
    if (n >= 1_000_000_000) return fmt(n / 1_000_000_000, n >= 10_000_000_000 ? 0 : 1) + ' млрд подписчиков'
    if (n >= 1_000_000) return fmt(n / 1_000_000, n >= 10_000_000 ? 0 : 1) + ' млн подписчиков'
    if (n >= 1_000) return fmt(n / 1_000, n >= 10_000 ? 0 : 1) + ' тыс. подписчиков'
    const lastTwo = n % 100
    if (lastTwo >= 11 && lastTwo <= 19) return n + ' подписчиков'
    const lastDigit = n % 10
    if (lastDigit === 1) return n + ' подписчик'
    if (lastDigit >= 2 && lastDigit <= 4) return n + ' подписчика'
    return n + ' подписчиков'
  }

  function qualityLabel(f: VideoFormat | null) {
    if (!f) return ''
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

  function qualityDesc(f: VideoFormat | null) {
    if (!f) return ''
    if (f.is_audio_only) {
      const c = (f.acodec ?? '').toLowerCase()
      if (c.startsWith('mp4a'))  return 'AAC'
      if (c.startsWith('opus'))   return 'Opus'
      if (c.startsWith('mp3'))    return 'MP3'
      if (c.startsWith('flac'))   return 'FLAC'
      if (c.startsWith('ac-3') || c.startsWith('eac3')) return 'AC3'
      return f.acodec?.split('.')[0]?.toUpperCase() ?? 'AAC'
    }
    const codec = f.vcodec?.split('.')[0]?.toUpperCase() ?? ''
    if (codec === 'AV01') return 'AV1'
    if (codec === 'VP09') return 'VP9'
    if (codec.startsWith('AVC')) return 'AVC'
    return codec || 'VIDEO'
  }

  // Выбрать первый совместимый кодек по умолчанию
  function pickDefaultCodecs() {
    if (dl.format === 'audio') {
      if (!dl.audioCodec || !isAudioCodecOk(dl.audioCodec)) {
        const compat = AUDIO_COMPAT[dl.container] ?? []
        dl.audioCodec = compat.length ? compat[0] : null
      }
    } else {
      if (!dl.videoCodec || !isVideoCodecOk(dl.videoCodec)) {
        const compat = VIDEO_COMPAT[dl.container] ?? []
        dl.videoCodec = compat.length ? compat[0] : null
      }
      if (dl.format === 'video') {
        if (!dl.audioCodec || !isAudioCodecOk(dl.audioCodec)) {
          const compat = AUDIO_COMPAT[dl.container] ?? []
          dl.audioCodec = compat.length ? compat[0] : null
        }
      }
    }
  }

  // Выбрать качество по умолчанию с учётом настроек пользователя
  function pickDefaultQuality() {
    dl.bitrate = null
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
  let wasMuriko = $state(false)
  $effect(() => {
    if (isMuriko && !wasMuriko) {
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
    wasMuriko = isMuriko
  })

  // Пасхалка — груз 200
  const isGruz200 = $derived(
    /^(груз(\s*-?\s*200)?|gruz(\s*-?\s*200)?|200|g200|г200)$/i.test(dl.url.trim())
  )
  let g200video = $state<HTMLVideoElement | null>(null)
  let wasGruz200 = $state(false)
  $effect(() => {
    if (isGruz200 && !wasGruz200) {
      emitThought('...', 'muted', 2)
      setTimeout(() => g200video?.play().catch(() => {}), 50)
    }
    wasGruz200 = isGruz200
  })

  const rejected = $derived(dl.url && !urlValid && !dl.loading && !isMuriko && !isGruz200 ? classifyRejectedUrl(dl.url) : null)

  // Автозапуск анализа при изменении валидного URL
  $effect(() => {
    if (dl.url !== dl.lastFetchedUrl) dl.error = null
    if (!urlValid && dl.url) { dl.info = null; dl.quality = '' }
    if (urlValid && dl.url !== dl.lastFetchedUrl && !dl.loading) {
      fetchInfo()
    }
  })

  // ── Действия ───────────────────────────────────────────────────────────────

  async function fetchInfo() {
    if (!canFetch) return
    const prevQuality = dl.quality
    dl.loading = true; dl.error = null; dl.info = null; dl.quality = ''
    dl.lastFetchedUrl = dl.url
    try {
      dl.info = await commands.fetchInfo(dl.url)
      dl.quality = prevQuality
      pickDefaultQuality()
      pickDefaultCodecs()
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
    if (dl.queuing || dl.queued) return
    if (!dl.info || !dl.quality) {
      if (!dl.quality) shakeQuality = true
      triggerShake()
      return
    }
    dl.queuing = true
    dl.queueError = null
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
      setTimeout(() => { dl.queued = false; clearUrl() }, 1800)
    } catch (e) {
      dl.queueError = e instanceof Error ? e.message : String(e)
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
    dl.url = ''; dl.info = null; dl.error = null; dl.queueError = null; dl.quality = ''; dl.audioCodec = null; dl.videoCodec = null; dl.lastFetchedUrl = ''; lastRejectedUrl = ''
  }

  function setFormat(f: string) {
    dl.format = f
    dl.bitrate = null
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
  <div class="url-bar" class:valid={urlValid} class:loading={dl.loading}>
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
        aria-hidden="true"
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
    <div class="analyzing-badge">
      <svg class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
      Анализ ссылки...
    </div>
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
            <img src={dl.info.thumbnail} alt={dl.info.title ?? 'Превью видео'} />
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
          <button class="channel-row" onclick={() => dl.info?.uploader_url && commands.openUrl(dl.info.uploader_url)} disabled={!dl.info?.uploader_url}>
            <div class="channel-avatar">
              {#if dl.info.channel_avatar}
                <img src={dl.info.channel_avatar} alt={dl.info.channel ?? 'Аватар канала'} onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display='none'; (e.currentTarget.nextElementSibling as HTMLElement).style.display='grid' }} />
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
          </button>
        {/if}
        {#if dl.info.title}
          <p class="video-title">{dl.info.title}</p>
        {/if}
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
          <button onclick={() => commands.openUrl(dl.info!.url)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
            Открыть
          </button>
        </div>
      </div>

      <!-- Тогл под карточкой описания -->
      <div class="format-toggle">
        <button class:active={dl.format === 'video'} onclick={() => setFormat('video')}
          use:tooltip={'Видео + аудио в одном файле'}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/></svg>
          Видео со звуком
        </button>
        <button class:active={dl.format === 'video_only'} onclick={() => setFormat('video_only')}
          use:tooltip={'Только видеодорожка без аудио'}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2" ry="2"/><line x1="1" y1="22" x2="23" y2="2"/></svg>
          Только видео
        </button>
        <button class:active={dl.format === 'audio'} onclick={() => setFormat('audio')}
          use:tooltip={'Только аудиодорожка без видео'}>
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
        <div class="quality-grid" class:shake={shakeQuality}>
          {#each videoFormats as f (f.format_id)}
            <button
              class="quality-card"
              class:selected={bestFormatId(f.resolution) === dl.quality || allVideoFormats.filter(fmt => fmt.resolution === f.resolution).some(fmt => fmt.format_id === dl.quality)}
              aria-pressed={bestFormatId(f.resolution) === dl.quality || allVideoFormats.filter(fmt => fmt.resolution === f.resolution).some(fmt => fmt.format_id === dl.quality)}
              onclick={() => {
                const id = bestFormatId(f.resolution)
                const sameRes = allVideoFormats.filter(fmt => fmt.resolution === f.resolution).some(fmt => fmt.format_id === dl.quality)
                if (dl.quality === id || sameRes) { dl.quality = ''; dl.fps = null; dl.bitrate = null }
                else { dl.quality = id; dl.fps = null; dl.bitrate = null }
              }}
            >
              <span class="q-value">{qualityLabel(f)}</span>
            </button>
          {/each}
        </div>

        <!-- FPS — только если есть варианты -->
        {#if fpsOptions.length > 0}
          <div class="fps-grid">
            {#each fpsOptions as f}
              <button
                class="quality-card"
                class:selected={dl.fps === f}
                aria-pressed={dl.fps === f}
                onclick={() => {
                  if (dl.fps === f) { dl.fps = null; dl.bitrate = null }
                  else { dl.fps = f; dl.bitrate = null }
                }}
              >
                <span class="q-value">{f}</span>
                <span class="q-desc">fps</span>
              </button>
            {/each}
          </div>
        {/if}

        <!-- Контейнер -->
        {@const containerDescs: Record<string, string> = {
          mp4:  'MP4 — универсальный. H.264/H.265/AV1 + AAC. Поддерживается везде.',
          webm: 'WebM — открытый. VP9/AV1 + Opus. Нативно в браузерах.',
          mkv:  'MKV — без ограничений по кодекам. Не все SmartTV и консоли.',
          mov:  'MOV (QuickTime) — контейнер Apple. Нужен для ProRes.',
        }}
        <div class="container-grid">
          {#each ['mp4','webm','mkv','mov'] as c}
            <button
              class="quality-card"
              class:selected={dl.container === c}
              aria-pressed={dl.container === c}
              use:tooltip={containerDescs[c]}
              onclick={() => dl.container = dl.container === c ? '' : c}
            >
              <span class="q-value">{c.toUpperCase()}</span>
            </button>
          {/each}
        </div>

        <!-- Видео-кодек -->
        {#if dl.format === 'video' || dl.format === 'video_only'}
        <div class="codec-row">
          <span class="codec-label">Видео</span>
          <div class="codec-cards-grid">
          {#each VIDEO_CODECS as codec}
            {@const ok = isVideoCodecOk(codec.id)}
            <button
              class="quality-card codec-card"
              class:selected={dl.videoCodec === codec.id}
              class:incompatible={!ok}
              aria-pressed={dl.videoCodec === codec.id}
              onmouseenter={(e) => onCodecEnter(e, codec)}
              onmouseleave={onCodecLeave}
              onclick={() => {
                if (dl.videoCodec === codec.id) { dl.videoCodec = null; dl.bitrate = null; return }
                dl.videoCodec = codec.id
                dl.bitrate = null
                if (codec.id === 'prores') dl.container = 'mov'
                else if (!isVideoCodecOk(codec.id)) {
                  const compatible = Object.entries(VIDEO_COMPAT).find(([, v]) => v.includes(codec.id))
                  if (compatible) dl.container = compatible[0]
                }
              }}
            >
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
        </div>
        {/if}

        <!-- Аудио-кодек — только при видео со звуком -->
        {#if dl.format === 'video'}
        <div class="codec-row">
          <span class="codec-label">Аудио</span>
          <div class="codec-cards-grid">
          {#each AUDIO_CODECS as codec}
            {@const ok = isAudioCodecOk(codec.id)}
            <button
              class="quality-card codec-card"
              class:selected={dl.audioCodec === codec.id}
              class:incompatible={!ok}
              aria-pressed={dl.audioCodec === codec.id}
              onmouseenter={(e) => onCodecEnter(e, codec)}
              onmouseleave={onCodecLeave}
              onclick={() => {
                if (dl.audioCodec === codec.id) { dl.audioCodec = null; return }
                dl.audioCodec = codec.id
                if (!isAudioCodecOk(codec.id)) {
                  const compatible = Object.entries(AUDIO_COMPAT).find(([, v]) => v.includes(codec.id))
                  if (compatible) dl.container = compatible[0]
                }
              }}
            >
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
        </div>
        {/if}
      {:else}
        <div class="quality-grid">
          {#each audioFormats as f (f.format_id)}
            <button
              class="quality-card"
              class:selected={dl.quality === f.format_id}
              aria-pressed={dl.quality === f.format_id}
              onclick={() => {
                if (dl.quality === f.format_id) { dl.quality = ''; dl.bitrate = null }
                else { dl.quality = f.format_id; dl.bitrate = null }
              }}
            >
              <span class="q-value">{qualityLabel(f)}</span>
              <span class="q-desc">{qualityDesc(f)}</span>
            </button>
          {/each}
        </div>

        {@const audioContainerDescs: Record<string, string> = {
          mp3:  'MP3 — универсальный. Поддерживается везде включая старые устройства.',
          m4a:  'M4A — AAC в MP4-контейнере. Лучшее качество при том же размере.',
          opus: 'Opus — открытый кодек IETF. Лучший lossy при низком битрейте.',
        }}
        <div class="container-grid">
          {#each ['mp3','m4a','opus'] as c}
            <button
              class="quality-card"
              class:selected={dl.container === c}
              aria-pressed={dl.container === c}
              use:tooltip={audioContainerDescs[c]}
              onclick={() => dl.container = dl.container === c ? '' : c}
            >
              <span class="q-value">{c.toUpperCase()}</span>
            </button>
          {/each}
        </div>

        <!-- Аудио-кодек -->
        <div class="codec-cards-grid">
          {#each AUDIO_CODECS as codec}
            <button
              class="quality-card codec-card"
              class:selected={dl.audioCodec === codec.id}
              aria-pressed={dl.audioCodec === codec.id}
              onmouseenter={(e) => onCodecEnter(e, codec)}
              onmouseleave={onCodecLeave}
              onclick={() => {
                if (dl.audioCodec === codec.id) { dl.audioCodec = null }
                else { dl.audioCodec = codec.id }
              }}
            >
              <span class="q-value">{codec.label}</span>
            </button>
          {/each}
        </div>
      {/if}

      <!-- Битрейт -->
      {#if maxBitrate > 0 || dl.quality}
        {@const isDisabled = maxBitrate === 0}
        {@const pct = maxBitrate > 0 ? (currentBitrate - minBitrate) / (maxBitrate - minBitrate) * 100 : 100}
        {@const bitrateQuality = pct >= 80 ? 'высокое' : pct >= 40 ? 'среднее' : 'низкое'}
        <div class="bitrate-card" class:bitrate-disabled={isDisabled}>
          {#if !isDisabled}
            <div class="bitrate-track-wrap">
              <input
                class="bitrate-slider"
                type="range"
                min={minBitrate}
                max={maxBitrate}
                step={Math.max(1, Math.round((maxBitrate - minBitrate) / 100))}
                value={currentBitrate}
                style="--pct: {pct.toFixed(1)}%"
                oninput={(e) => {
                  const v = +(e.target as HTMLInputElement).value
                  const p = (v - minBitrate) / (maxBitrate - minBitrate) * 100
                  ;(e.target as HTMLInputElement).style.setProperty('--pct', p.toFixed(1) + '%')
                  dl.bitrate = v >= maxBitrate ? null : v
                }}
              />
              <div class="bitrate-marks">
                <span class="bm" style="left: calc(8px + 40% * ((100% - 16px) / 100%))"><span class="bm-label">среднее</span></span>
                <span class="bm" style="left: calc(8px + 80% * ((100% - 16px) / 100%))"><span class="bm-label">высокое</span></span>
              </div>
              <div class="bitrate-tooltip" style="left: calc(8px + {pct.toFixed(1)}% * ((100% - 16px) / 100%))">
                <span class="bt-value">{dl.bitrate === null ? 'Авто' : `${currentBitrate} kbps`}</span>
                {#if dl.bitrate !== null}<span class="bt-quality">{bitrateQuality}</span>{/if}
                {#if estimatedSize}<span class="bt-size">~{formatBytes(estimatedSize)}</span>{/if}
              </div>
            </div>
          {/if}
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
              <span class="size-badge" use:tooltip={resTip}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><polyline points="8 21 12 17 16 21"/></svg>
                {qualityLabel(selectedFormat)}
              </span>
              {#if dl.fps !== null}
                <span class="size-badge" use:tooltip={'Кадров в секунду'}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="20" rx="2.18"/><line x1="7" y1="2" x2="7" y2="22"/><line x1="17" y1="2" x2="17" y2="22"/><line x1="2" y1="12" x2="22" y2="12"/><line x1="2" y1="7" x2="7" y2="7"/><line x1="17" y1="7" x2="22" y2="7"/><line x1="17" y1="17" x2="22" y2="17"/><line x1="2" y1="17" x2="7" y2="7"/></svg>
                  {dl.fps} fps
                </span>
              {/if}
              {#if dl.container}
                <span class="size-badge" use:tooltip={containerTip}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>
                  {dl.container.toUpperCase()}
                </span>
              {/if}
              {#if dl.videoCodec}
                <span class="size-badge" use:tooltip={codecTip}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
                  {VIDEO_CODECS.find(c => c.id === dl.videoCodec)?.label}
                </span>
              {/if}
              {#if dl.audioCodec}
                <span class="size-badge" use:tooltip={AUDIO_CODECS.find(c => c.id === dl.audioCodec)?.lines[0].replace(/<[^>]+>/g, '') ?? 'Аудио кодек'}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
                  {AUDIO_CODECS.find(c => c.id === dl.audioCodec)?.label ?? dl.audioCodec}
                </span>
              {/if}
              {#if dl.info?.duration}
                <span class="size-badge" use:tooltip={'Длительность видео'}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a10 10 0 1 0 0 20A10 10 0 0 0 12 2"/><polyline points="12 6 12 12 16 14"/></svg>
                  {formatDuration(dl.info.duration)}
                </span>
              {/if}
            </div>
          {/if}
        </div>

        <button
          class="btn-download"
          class:shake={downloadShake}
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
        {#if dl.queueError}
          <p class="queue-error">{dl.queueError}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if codecTipData}
  {@const r = codecTipData.rect}
  <div class="codec-tip-portal tooltip-popup" style="left:{r.left + r.width/2}px; top:{r.top}px">
    {#each codecTipData.codec.lines as line, i}
      <span class:tip-main={i === 0} class:tip-sub={i > 0}>{@html line}</span>
    {/each}
  </div>
{/if}

<style>
  .page {
    padding: var(--space-8) var(--space-9);
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
    background: var(--bg-surface);
    border: 1.5px solid var(--border-default);
    border-radius: var(--radius-panel);
    padding: 0 6px 0 16px;
    transition: border-color var(--transition-default), box-shadow var(--transition-default);
  }
  .url-bar:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .url-bar.valid { border-color: var(--border-strong); }
  .url-bar.loading { opacity: 0.75; }
  .url-icon {
    flex-shrink: 0; width: 18px; height: 18px;
    display: grid; place-items: center;
    color: var(--text-muted); transition: color var(--transition-default), filter var(--transition-default); pointer-events: none;
  }
  .url-icon svg { width: 20px; height: 20px; }
  .url-icon.lit { color: var(--accent);     filter: drop-shadow(0 1px 3px color-mix(in srgb, var(--accent) 35%, transparent)) drop-shadow(0 3px 8px color-mix(in srgb, var(--accent) 15%, transparent)); }
  .url-input {
    flex: 1; height: 100%;
    background: transparent; border: none; outline: none;
    color: var(--text-primary); font-size: 13px; padding: 0 12px; text-align: center;
    min-width: 0; caret-color: var(--accent);
    letter-spacing: -0.01em;
  }
  .url-input::placeholder { color: var(--text-muted); }
  .btn-clear {
    width: 30px; height: 30px; display: grid; place-items: center;
    background: none; border: none; border-radius: 7px;
    color: var(--text-muted); cursor: pointer; flex-shrink: 0;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .btn-clear svg { width: 12px; height: 12px; }
  .btn-clear:hover { background: rgba(255,255,255,0.08); color: var(--text-secondary); }
  .btn-fetch {
    width: 38px; height: 38px; margin-left: 4px; flex-shrink: 0;
    display: grid; place-items: center;
    background: linear-gradient(135deg, var(--accent), var(--accent-warm));
    border: none; border-radius: var(--radius-card); color: var(--text-primary); cursor: pointer;
    transition: filter var(--transition-default), transform var(--transition-fast), box-shadow var(--transition-default);
  }
  .btn-fetch svg { width: 14px; height: 14px; }
  .btn-fetch:hover:not(:disabled) {
    filter: brightness(1.1);
    box-shadow: 0 4px 18px color-mix(in srgb, var(--accent) 40%, transparent);
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
    border-radius: var(--radius-lg);
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
    border-radius: var(--radius-card);
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
    animation: fadeUp 0.35s 0.26s ease both;
    box-shadow:
      inset 0 1px 0 rgba(180,180,180,0.12),
      inset 0 -1px 0 rgba(0,0,0,0.2);
  }
  .error-retry:hover {
    background: rgba(255,255,255,0.09);
    color: var(--text-primary);
  }

  /* ── Analyzing badge ── */
  .analyzing-badge {
    display: flex; align-items: center; gap: var(--space-2);
    font-size: var(--text-sm); color: var(--status-info);
    margin-top: var(--space-4); animation: fadeUp 0.3s ease both;
  }
  .analyzing-badge svg { width: 14px; height: 14px; }

  /* ── Shimmer skeleton ── */
  .shimmer {
    position: relative; overflow: hidden; background: var(--bg-overlay);
  }
  .shimmer::after {
    content: ''; position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.035) 45%, rgba(255,255,255,0.055) 50%, rgba(255,255,255,0.035) 55%, transparent 100%);
    animation: shimmer 2.2s infinite;
  }
  @keyframes shimmer { 0% { transform: translateX(-100%); } 100% { transform: translateX(100%); } }

  .skeleton-grid { margin-top: 16px; }
  .thumb-skeleton { width: 100%; aspect-ratio: 16/9; border-radius: var(--radius-panel); }
  .sk { border-radius: var(--radius-sm); }
  .sk-row { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
  .sk-avatar { width: 26px; height: 26px; border-radius: 50%; flex-shrink: 0; }
  .sk-name  { height: 12px; width: 110px; }
  .sk-title { height: 18px; width: 88%; margin-bottom: 8px; }
  .sk-meta  { height: 12px; width: 50%; }
  .options-skeleton { display: flex; flex-direction: column; gap: 10px; margin-top: 12px; }
  .sk-toggle { height: 44px; border-radius: var(--radius-card); }
  .sk-cards  { display: grid; grid-template-columns: repeat(4,1fr); gap: 8px; }
  .sk-card   { height: 88px; border-radius: var(--radius-lg); }
  .sk-action { height: 58px; border-radius: var(--radius-lg); }

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
    border-radius: var(--radius-panel); overflow: visible; position: relative;
    background: var(--bg-elevated);
    box-shadow: 0 4px 16px rgba(0,0,0,0.3);
  }
  .thumb-wrap { width: 100%; aspect-ratio: 16/9; background: var(--bg-overlay); overflow: hidden; border-radius: var(--radius-panel); }
  .thumb-card::after { content: ''; position: absolute; inset: 0; border-radius: var(--radius-panel); box-shadow: inset 0 1px 0 rgba(0,0,0,0.4), inset 0 -1px 0 rgba(0,0,0,0.4); pointer-events: none; z-index: 1; }
  .thumb-wrap img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .thumb-placeholder { width: 100%; height: 100%; background: var(--bg-overlay); }
  .duration-badge {
    position: absolute; bottom: 10px; right: 10px;
    display: inline-flex; align-items: center; gap: 4px;
    padding: 4px 9px;
    background: rgba(0,0,0,0.82);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: var(--radius-sm);
    font-size: 11px; font-weight: 600; color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    backdrop-filter: blur(6px);
    letter-spacing: 0.02em;
  }
  .duration-badge svg { width: 10px; height: 10px; opacity: 0.5; }

  /* Инфо */
  .info-card {
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-panel);
    padding: 20px 22px;
    flex: 1;
    display: flex; flex-direction: column; gap: 10px; align-items: flex-start;
    min-width: 0;
    box-shadow: 0 4px 20px rgba(0,0,0,0.2);
  }
  .video-title {
    font-size: 13px; font-weight: 600; color: var(--text-primary);
    line-height: 1.4; margin: 0;
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .channel-row { display: inline-flex; align-items: center; gap: 8px; padding: 6px 8px; margin: -6px -8px; border-radius: var(--radius-card); cursor: pointer; transition: background var(--transition-default), box-shadow var(--transition-default); background: none; border: none; color: inherit; font: inherit; text-align: left; }
  .channel-row:hover { background: rgba(0,0,0,0.35); box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 1px 0 rgba(120,120,120,0.5); }
  .channel-avatar {
    width: 36px; height: 36px; border-radius: 50%;
    background: var(--bg-overlay);
    display: grid; place-items: center; overflow: hidden;
    font-size: 13px; font-weight: 700; color: var(--text-primary); flex-shrink: 0;
  }
  .channel-avatar img { width: 100%; height: 100%; object-fit: cover; display: block; outline: none; border-radius: 50%; }
  .channel-info { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .channel-name {
    font-size: 13px; font-weight: 600; color: var(--text-secondary);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .channel-subs { font-size: 11px; color: var(--text-muted); font-weight: 500; }
  .playlist-warning {
    display: flex; align-items: flex-start; gap: 6px;
    padding: 7px 10px; border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--status-warning) 8%, transparent); border: 1px solid color-mix(in srgb, var(--status-warning) 25%, transparent);
    font-size: 11px; color: var(--status-warning); line-height: 1.4;
  }
  .playlist-warning svg { width: 12px; height: 12px; flex-shrink: 0; margin-top: 1px; }
  .video-meta { display: flex; gap: 8px; flex-wrap: wrap; }
  .meta-badge {
    display: inline-flex; align-items: center; gap: 4px;
    padding: 2px 8px; background: transparent;
    border: 1px solid var(--border-default); border-radius: var(--radius-sm);
    font-size: 12px; font-weight: 600; color: var(--text-muted);
    letter-spacing: 0.04em; text-transform: uppercase;
  }
  .meta-badge svg { width: 10px; height: 10px; }
  .info-actions { display: flex; gap: 8px; margin-top: auto; padding-top: 6px; }
  .info-actions button {
    display: inline-flex; align-items: center; gap: 5px;
    height: 28px; padding: 0 11px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: 7px; color: var(--text-secondary);
    font-size: 11px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .info-actions button svg { width: 11px; height: 11px; }
  .info-actions button:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }

  /* ── Options ── */
  .options { display: flex; flex-direction: column; gap: 14px; margin-top: 6px; flex: 1; }

  /* Format toggle */
  .format-toggle {
    display: flex;
    background: var(--bg-elevated); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-card); padding: 3px;
  }
  .format-toggle button {
    flex: 1; display: flex; align-items: center; justify-content: center; gap: 7px;
    height: 36px; background: transparent; border: none; border-radius: var(--radius-md);
    color: var(--text-muted); font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-default), color var(--transition-default);
  }
  .format-toggle button svg { width: 13px; height: 13px; opacity: 0.5; }
  .format-toggle button:hover { color: var(--text-secondary); }
  .format-toggle button.active {
    background: rgba(0,0,0,0.35); color: var(--text-primary); font-weight: 600;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03), inset 0 1px 0 rgba(120,120,120,0.5);
  }
  .format-toggle button.active svg { opacity: 1; color: var(--accent); }

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
    background: var(--bg-elevated); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg); cursor: pointer;
    transition: border-color 0.18s, background 0.18s, transform 0.12s, box-shadow 0.18s;
  }
  .quality-card:hover {
    border-color: var(--border-default); background: var(--bg-overlay);
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
  .quality-card.selected .q-value { color: var(--accent); }
  .q-value {
    font-size: 18px; font-weight: 700; letter-spacing: -0.03em;
    color: var(--text-primary); line-height: 1;
  }
  .q-desc {
    font-size: 9px; font-weight: 600; color: var(--border-strong);
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
    background: var(--bg-elevated); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg); padding: 0 16px; overflow: visible;
  }
  .size-icon {
    width: 32px; height: 32px; border-radius: var(--radius-sm);
    background: rgba(0,0,0,0.35); border: 1px solid transparent;
    box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03);
    display: grid; place-items: center; flex-shrink: 0;
  }
  .size-icon svg { width: 16px; height: 16px; color: var(--accent); opacity: 0.65; }
  /* Портальный тултип кодека — fixed, вне stacking context родителей */
  .codec-tip-portal {
    position: fixed;
    transform: translateX(-50%) translateY(calc(-100% - 8px));
    width: 220px; text-align: left; white-space: normal;
    z-index: 9999; pointer-events: none;
    display: flex; flex-direction: column; gap: 5px;
    animation: tooltip-in-up 0.15s ease forwards;
    padding: 10px 13px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    box-shadow: 0 4px 16px rgba(0,0,0,0.55);
  }
  @keyframes tooltip-in-up {
    from { opacity: 0; transform: translateX(-50%) translateY(calc(-100% - 4px)); }
    to   { opacity: 1; transform: translateX(-50%) translateY(calc(-100% - 8px)); }
  }
  @keyframes tip-in {
    from { opacity: 0; transform: translateX(-50%) translateY(calc(-100% - 4px)); }
    to   { opacity: 1; transform: translateX(-50%) translateY(calc(-100% - 8px)); }
  }
  .codec-tip-portal :global(em) { font-style: normal; color: var(--accent); }
  .codec-tip-portal :global(warn) { color: var(--thought-warning); }
  .tip-main {
    display: block;
    font-size: 11px; font-weight: 500; color: var(--text-primary);
    line-height: 1.45; letter-spacing: 0; text-transform: none;
  }
  .tip-sub {
    display: block;
    font-size: 11px; font-weight: 400; color: var(--text-primary);
    line-height: 1.45; letter-spacing: 0; text-transform: none;
    padding-top: 5px; border-top: 1px solid var(--border-subtle);
  }
  .size-badge svg { width: 11px; height: 11px; opacity: 0.6; flex-shrink: 0; }
  .size-badge-main { font-size: 16px; font-weight: 700; letter-spacing: -0.01em; color: var(--text-primary); background: var(--bg-overlay); padding: 5px 12px; box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03); }
  .free-space { font-size: 10px; color: rgba(255,255,255,0.3); font-weight: 500; letter-spacing: 0.02em; cursor: pointer; transition: color var(--transition-fast); }
  .free-space:hover { color: rgba(255,255,255,0.6); }
  .size-badges { display: flex; gap: 5px; margin-left: auto; flex-shrink: 0; }
  .size-badge { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; font-weight: 600; color: rgba(255,255,255,0.4); background: transparent; border: 1px solid transparent; border-radius: var(--radius-sm); padding: 5px 4px; letter-spacing: 0.04em; text-transform: uppercase; transition: all var(--transition-default); box-shadow: none; }
  .size-badge:hover { color: rgba(255,255,255,1); background: rgba(0,0,0,0.35); padding: 5px 12px; box-shadow: inset 0 2px 4px rgba(0,0,0,0.6), inset 0 -1px 1px rgba(255,255,255,0.03); }

  @keyframes shake {
    0%, 100% { transform: translateX(0); }
    20% { transform: translateX(-6px); }
    40% { transform: translateX(6px); }
    60% { transform: translateX(-4px); }
    80% { transform: translateX(4px); }
  }
  .shake { animation: shake 0.4s ease; }
  .quality-grid.shake button { border-color: var(--accent); }

  .btn-download {
    height: 58px; padding: 0 36px;
    display: flex; align-items: center; gap: 10px;
    background: var(--bg-overlay);
    border: 1px solid rgba(0,0,0,1); border-radius: var(--radius-lg);
    color: var(--text-primary); font-size: 14px; font-weight: 700;
    cursor: pointer; white-space: nowrap; letter-spacing: 0.02em;
    transition: filter var(--transition-default), box-shadow var(--transition-default), transform var(--transition-fast);
    position: relative;
    box-shadow: inset 0 1px 0 rgba(120,120,120,0.5), inset 0 -1px 0 rgba(80,80,80,0.15);
  }
  .btn-download svg { width: 15px; height: 15px; }
  .btn-download:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 13%, transparent);
    border-color: color-mix(in srgb, var(--accent) 80%, transparent);
    box-shadow: 0 0 20px color-mix(in srgb, var(--accent) 15%, transparent), inset 0 1px 0 color-mix(in srgb, var(--accent) 15%, transparent);
    transform: translateY(-1px);
  }
  .btn-download:active:not(:disabled) { transform: scale(0.97); }
  .btn-download:disabled { opacity: 0.25; cursor: default; box-shadow: none; }
  .queue-error {
    margin: 0; font-size: 11px; color: var(--accent); text-align: center; max-width: 240px;
  }

  /* Битрейт */
  .bitrate-card { padding: var(--space-2) 0; }
  .bitrate-disabled { opacity: 0.45; pointer-events: none; }

  .bitrate-track-wrap { position: relative; padding-bottom: 14px; }
  .bitrate-track-wrap::before,
  .bitrate-track-wrap::after { display: none; }
  .bitrate-marks { position: absolute; top: 0; left: 0; right: 0; height: 4px; pointer-events: none; }
  .bm {
    position: absolute; top: 50%; transform: translate(-50%, -50%);
    display: flex; flex-direction: column; align-items: center;
  }
  .bm::before {
    content: ''; display: block;
    width: 1px; height: 8px;
    background: var(--text-primary);
  }
  .bm-label {
    position: absolute; top: calc(100% + 5px);
    font-size: 9px; color: var(--text-muted);
    white-space: nowrap; transform: translateX(-50%); left: 50%;
  }

  /* --pct задаётся инлайн на input; псевдоэлемент трека наследует через каскад (Chromium 108+) */
  .bitrate-slider {
    -webkit-appearance: none; appearance: none;
    width: 100%; height: 4px;
    background: transparent;
    outline: none; cursor: pointer; margin: 0; display: block;
  }
  .bitrate-slider::-webkit-slider-runnable-track {
    height: 4px; border-radius: 99px;
    background: linear-gradient(to right, var(--accent) var(--pct), rgba(255,255,255,0.15) var(--pct));
  }
  .bitrate-slider::-webkit-slider-thumb {
    -webkit-appearance: none; appearance: none;
    width: 16px; height: 16px; border-radius: 50%;
    background: var(--accent); cursor: pointer;
    margin-top: -6px;
    transition: box-shadow 150ms ease-out, transform 150ms ease-out;
  }
  .bitrate-slider::-webkit-slider-thumb:hover,
  .bitrate-slider:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 25%, transparent);
    transform: scale(1.15);
  }

  .bitrate-tooltip {
    position: absolute; bottom: calc(100% + 10px);
    transform: translateX(-50%) translateY(4px);
    display: flex; flex-direction: column; gap: 2px;
    pointer-events: none; opacity: 0;
    transition: opacity 150ms ease-out, transform 150ms ease-out;
    white-space: nowrap; z-index: 9999;
    background: var(--bg-elevated); padding: 6px 10px;
    border-radius: var(--radius-sm); border: 1px solid var(--border-subtle);
    box-shadow: 0 4px 12px rgba(0,0,0,0.4);
  }
  .bitrate-slider:hover ~ .bitrate-tooltip,
  .bitrate-slider:focus-visible ~ .bitrate-tooltip { opacity: 1; transform: translateX(-50%) translateY(0); }
  .bt-value { font-size: 13px; font-weight: 700; color: var(--text-primary); font-variant-numeric: tabular-nums; }
  .bt-quality { font-size: 11px; font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em; }
  .bt-size { font-size: 10px; color: var(--text-muted); }
  /* Пикеры кодеков */
  
  
  
  
  
  
  
  
  
  
  
  
  
  

  /* Animations */
  .spin { animation: spin 0.75s linear infinite; }
  @keyframes spin    { to { transform: rotate(360deg); } }
  @keyframes pulse   { 0%,100% { opacity: 1; } 50% { opacity: 0.35; } }

  .codec-row { display: flex; align-items: center; gap: var(--space-3); }
  .codec-label { font-size: 11px; color: var(--text-muted); white-space: nowrap; width: 32px; flex-shrink: 0; }
  .codec-cards-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 8px; flex: 1; }
  .quality-card.codec-card { min-height: 34px; padding: 8px; overflow: visible; }
  .codec-card .q-value { font-size: 13px; font-weight: 700; letter-spacing: 0.03em; }
  .codec-card.incompatible { opacity: 0.35; }
  .codec-card.incompatible:hover { opacity: 1; }
  .codec-card.incompatible:hover .q-value { color: var(--text-muted); }
  
  /* focus-visible для всех кнопок */
  .quality-card:focus-visible, .format-toggle button:focus-visible,
  .btn-download:focus-visible, .btn-fetch:focus-visible,
  .btn-clear:focus-visible, .codec-card:focus-visible,
  .info-actions button:focus-visible, .error-retry:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .channel-row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-card);
  }
  .channel-row:disabled { cursor: default; pointer-events: none; }
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { animation-duration: 0.01ms !important; animation-iteration-count: 1 !important; }
}
</style>
