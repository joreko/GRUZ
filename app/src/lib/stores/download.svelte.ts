import type { VideoInfo } from '$lib/bridge/types'

// Состояние DownloadPage живёт здесь — не теряется при навигации
export const dl = $state({
  url:            '',
  info:           null as VideoInfo | null,
  loading:        false,
  error:          null as string | null,
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
