// Определение типа YouTube URL

export type UrlType = 'video' | 'short' | 'playlist' | 'live' | 'music' | 'unknown'

export function detectUrlType(url: string): UrlType {
  if (!url) return 'unknown'
  try {
    const u = new URL(url)
    if (u.hostname.includes('music.youtube.com')) return 'music'
    if (u.pathname === '/watch' && u.searchParams.has('list') && !u.searchParams.has('v')) return 'playlist'
    if (u.pathname.startsWith('/shorts/')) return 'short'
    if (u.pathname === '/watch') return 'video'
    if (u.hostname === 'youtu.be') return 'video'
    if (u.pathname.startsWith('/playlist')) return 'playlist'
    return 'unknown'
  } catch {
    return 'unknown'
  }
}

export function isValidYoutubeUrl(url: string): boolean {
  try {
    const u = new URL(url)
    return (
      u.hostname.includes('youtube.com') ||
      u.hostname === 'youtu.be' ||
      u.hostname.includes('music.youtube.com')
    )
  } catch {
    return false
  }
}

export type RejectedUrlKind = 'nsfw' | 'social' | 'video_other' | 'not_url' | 'other_site'

export interface RejectedUrl {
  kind: RejectedUrlKind
  emoji: string
  title: string
  hint: string
}

export function classifyRejectedUrl(url: string): RejectedUrl | null {
  if (!url.trim()) return null

  // Проверяем — это вообще URL?
  let hostname = ''
  try {
    hostname = new URL(url).hostname.replace('www.', '')
  } catch {
    // Не URL совсем
    return {
      kind: 'not_url',
      emoji: '(°_°)',
      title: 'Это не ссылка',
      hint: 'Вставьте ссылку на YouTube видео.',
    }
  }

  // NSFW
  if (['pornhub.com','xvideos.com','xnxx.com','onlyfans.com','brazzers.com',
       'redtube.com','youporn.com','spankbang.com','xhamster.com'].some(d => hostname.includes(d))) {
    return {
      kind: 'nsfw',
      emoji: '( ͡° ͜ʖ ͡°)',
      title: 'Серьёзно?',
      hint: 'Я скачиваю только YouTube. Ты знаешь что делаешь.',
    }
  }

  // Другие видео-сайты
  if (['vimeo.com','twitch.tv','dailymotion.com','rutube.ru','ok.ru',
       'vk.com','tiktok.com','instagram.com','twitter.com','x.com'].some(d => hostname.includes(d))) {
    return {
      kind: 'video_other',
      emoji: '(・_・)',
      title: 'Не YouTube',
      hint: `Я умею только YouTube. ${hostname} — не моё.`,
    }
  }

  // Соцсети / всякое
  if (['facebook.com','reddit.com','telegram.org','discord.com',
       'spotify.com','soundcloud.com'].some(d => hostname.includes(d))) {
    return {
      kind: 'social',
      emoji: '(￣_￣)',
      title: 'Не то',
      hint: 'Вставьте ссылку на YouTube видео.',
    }
  }

  // Любой другой сайт
  return {
    kind: 'other_site',
    emoji: '(¬_¬)',
    title: 'Не YouTube',
    hint: `${hostname} — не поддерживается. Нужна ссылка на YouTube.`,
  }
}
