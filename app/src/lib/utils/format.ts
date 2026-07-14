// Утилиты форматирования — нет зависимостей, чистые функции

export function formatBytes(bytes: number | null): string {
  if (bytes == null) return '—'
  const fmt = (v: number) => v.toLocaleString('ru-RU', { maximumFractionDigits: 1, minimumFractionDigits: 1 })
  if (bytes < 1024) return `${bytes} Б`
  if (bytes < 1024 ** 2) return `${fmt(bytes / 1024)} КБ`
  if (bytes < 1024 ** 3) return `${fmt(bytes / 1024 ** 2)} МБ`
  return `${fmt(bytes / 1024 ** 3)} ГБ`
}

export function formatDuration(seconds: number | null): string {
  if (seconds == null || seconds < 0) return '—'
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  return `${m}:${String(s).padStart(2, '0')}`
}

export function formatDate(value: string | number): string {
  if (value == null) return '—'
  const ms = typeof value === 'number'
    ? value * 1000
    : /^\d+$/.test(value) ? Number(value) * 1000 : value
  const d = new Date(ms)
  if (isNaN(d.getTime())) return '—'
  return new Intl.DateTimeFormat('ru', {
    day: '2-digit', month: '2-digit', year: 'numeric',
    hour: '2-digit', minute: '2-digit',
  }).format(d)
}

/** Читаемое описание параметров скачивания из элемента истории */
export function formatParams(item: {
  format: string, quality: string, container: string,
  fps?: number | null, source_fps?: number | null,
  video_codec?: string | null, audio_codec?: string | null,
  file_size?: number | null
}): string {
  const parts: string[] = []
  // Разрешение
  const h = item.quality.match(/height=(\d+)/)?.[1]
  if (h) parts.push(`${h}p`)
  else if (item.format === 'audio') parts.push('аудио')
  // FPS — показываем реальный fps источника, если он известен;
  // иначе падаем на выбранный лимит (для обратной совместимости)
  const displayFps = item.source_fps ?? item.fps
  if (displayFps && displayFps > 0) parts.push(`${displayFps}fps`)
  // Контейнер
  parts.push(item.container.toUpperCase())
  // Кодек (кратко)
  const vc = item.video_codec
  if (vc && vc !== 'copy') parts.push(vc.toUpperCase())
  // Размер
  if (item.file_size) parts.push(formatBytes(item.file_size))
  return parts.join(' · ')
}
