import type { VideoInfo } from '$lib/bridge/types'

// Состояние DownloadPage живёт здесь — не теряется при навигации
export const dl = $state({
  url:            '',
  info:           null as VideoInfo | null,
  loading:        false,
  error:          null as string | null,
  queueError:     null as string | null,  // ошибка добавления в очередь (не ошибка загрузки инфо)
  queued:         false,
  queuing:        false,
  format:         'video',
  quality:        '',
  fps:            null as number | null,  // выбранный fps для текущего разрешения
  bitrate:        null as number | null,  // выбранный битрейт kbps (null = максимум)
  container:      'mp4',
  videoCodec:     null as string | null,  // null = auto (лучший доступный)
  audioCodec:     null as string | null,  // null = auto (лучший доступный)
  lastFetchedUrl: '',
  currentEmoji:   '',
})

export function reset() {
  dl.url = ''
  dl.info = null
  dl.loading = false
  dl.error = null
  dl.queueError = null
  dl.queued = false
  dl.queuing = false
  dl.quality = ''
  dl.fps = null
  dl.bitrate = null
  dl.videoCodec = null
  dl.audioCodec = null
  dl.lastFetchedUrl = ''
  dl.currentEmoji = ''
  dl.container = 'mp4'
  dl.format = 'video'
}
