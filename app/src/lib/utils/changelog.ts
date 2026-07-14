// Утилиты для работы с changelog через GitHub API

export const REPO = 'joreko/GRUZ'
const API = `https://api.github.com/repos/${REPO}`

// Пользовательские типы — показываются в changelog и считаются в (+N)
const USER_TYPES = new Set(['добавлено', 'исправлено', 'улучшено', 'быстрее', 'удалено'])

// Технические scope — идут в techLines независимо от типа коммита
const TECH_SCOPES = new Set(['ci', 'deps', 'tauri', 'bridge', 'installer', 'stores', 'docs', 'test', 'refact'])

export interface CommitLine {
  type: string       // тип коммита (добавлено, исправлено, ...)
  scope: string      // область (queue, downloader, ...)
  text: string       // описание изменения
}

export interface ParsedCommit {
  counter: number        // XXXX из заголовка
  lines: CommitLine[]    // пользовательские строки (добавлено/исправлено/улучшено/быстрее)
  techLines: CommitLine[] // технические строки (рефакт/сборка/доки/тесты и прочие)
}

export interface ReleaseChangelog {
  tag: string
  name: string
  publishedAt: string
  prerelease: boolean
  description: string        // body релиза из GitHub
  commits: ParsedCommit[]
  totalUserLines: number
  latestCounter: number
}

// Версия в tauri.conf.json: "0.0.104" — patch это счётчик коммита.
// Показываем версию полностью: "0.0.104". patch несёт смысл (номер сборки),
// обрезать его нельзя — иначе все релизы выглядят одинаково ("v0.0").
// Суффикс пре-релиза ("-beta") отбрасываем — о бете говорит отдельный бейдж.
export function displayVersion(full: string): string {
  return full.replace(/-.*$/, '')
}

// patch-компонент версии = счётчик коммита. "0.0.130" → 130
export function versionCounter(full: string): number {
  const parts = full.split('.')
  return parseInt(parts[parts.length - 1], 10) || 0
}

// Сравнение semver-версий. >0 если a новее b, <0 если старее, 0 если равны.
export function compareVersions(a: string, b: string): number {
  const clean = (s: string) => s.replace(/-.*$/, '')
  const pa = clean(a).split('.').map(n => parseInt(n, 10) || 0)
  const pb = clean(b).split('.').map(n => parseInt(n, 10) || 0)
  const len = Math.max(pa.length, pb.length)
  for (let i = 0; i < len; i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (diff !== 0) return diff
  }
  return 0
}

// Парсим сообщение коммита. Возвращает counter + пользовательские строки.
// counter берётся из заголовка даже если пользовательских строк нет —
// это важно для точного сравнения версий по счётчику.
function parseCommitMessage(message: string): { counter: number; lines: CommitLine[]; techLines: CommitLine[] } | null {
  const lines = message.trim().split('\n').map(l => l.trim()).filter(Boolean)
  if (lines.length === 0) return null

  const counter = parseInt(lines[0], 10)
  if (isNaN(counter)) return null

  const userLines: CommitLine[] = []
  const techLines: CommitLine[] = []
  for (const line of lines.slice(1)) {
    const m = line.match(/^([а-яёА-ЯЁ]+)(?:\(([^)]+)\))?:\s*(.+)$/)
    if (!m) continue
    const [, type, scope = '', text] = m
    if (USER_TYPES.has(type) && !TECH_SCOPES.has(scope)) userLines.push({ type, scope, text })
    else if (type !== 'описание') techLines.push({ type, scope, text })
  }

  return { counter, lines: userLines, techLines }
}

interface GithubRelease {
  tag_name: string
  name: string
  body: string
  published_at: string
  prerelease: boolean
}

interface GithubCommit {
  sha: string
  commit: { message: string }
}

async function fetchReleases(): Promise<GithubRelease[]> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch(`${API}/releases?per_page=30`, { signal: ctrl.signal })
    if (!r.ok) throw new Error(`${r.status}`)
    return r.json()
  } finally { clearTimeout(t) }
}

// Коммиты между двумя тегами через compare API — точный SHA-диапазон
async function fetchCommitsBetween(base: string, head: string): Promise<GithubCommit[]> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch(`${API}/compare/${base}...${head}`, { signal: ctrl.signal })
    if (!r.ok) throw new Error(`${r.status}`)
    const data = await r.json()
    return data.commits ?? []
  } finally { clearTimeout(t) }
}

// Для самого первого релиза — берём все коммиты до тега
async function fetchCommitsUpTo(tag: string): Promise<GithubCommit[]> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch(`${API}/commits?sha=${tag}&per_page=100`, { signal: ctrl.signal })
    if (!r.ok) throw new Error(`${r.status}`)
    return r.json()
  } finally { clearTimeout(t) }
}

// Кэш changelog — GitHub API без авторизации даёт лишь 60 запросов/час,
// а каждый fetchChangelog тратит ~1+N. TitleBar и UpdatesPage зовут его
// независимо, плюс повторные заходы — без кэша лимит быстро исчерпывается.
const CACHE_TTL_MS = 5 * 60 * 1000
let memoryCache: { at: number; data: ReleaseChangelog[] } | null = null
let inflight: Promise<ReleaseChangelog[]> | null = null

// Основная функция — загружает полный changelog по релизам параллельно.
// Результат кэшируется на 5 минут; параллельные вызовы дедуплицируются.
// force=true принудительно обходит кэш и инфлайт (ручная «Проверить обновления»).
export async function fetchChangelog(force = false): Promise<ReleaseChangelog[]> {
  if (!force) {
    if (memoryCache && Date.now() - memoryCache.at < CACHE_TTL_MS) {
      return memoryCache.data
    }
    if (inflight) return inflight
  }

  inflight = fetchChangelogUncached()
    .then(data => {
      memoryCache = { at: Date.now(), data }
      return data
    })
    .finally(() => { inflight = null })

  return inflight
}

// Сколько запросов к GitHub API ещё доступно в текущем часовом окне.
// Эндпоинт rate_limit сам НЕ расходует лимит — можно звать свободно.
// Возвращает null при сбое сети/API (не блокирует основной сценарий).
export async function fetchRateLimitRemaining(): Promise<number | null> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch('https://api.github.com/rate_limit', { signal: ctrl.signal })
    if (!r.ok) return null
    const data = await r.json()
    return data?.resources?.core?.remaining ?? null
  } catch {
    return null
  } finally {
    clearTimeout(t)
  }
}

interface GithubAsset {
  name: string
  browser_download_url: string
}

// Самый лёгкий способ узнать, появился ли новый релиз: один запрос к
// последнему релизу. Для авто-проверки раз в минуту — чтобы не жечь лимит
// GitHub (60/час): полный fetchChangelog стоит ~1+N запросов. Возвращает
// tag_name или null при сбое/отсутствии.
export async function fetchLatestTag(): Promise<string | null> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch(`${API}/releases?per_page=1`, { signal: ctrl.signal })
    if (!r.ok) return null
    const data = await r.json()
    const first = Array.isArray(data) ? data[0] : null
    return first?.tag_name ?? null
  } catch {
    return null
  } finally {
    clearTimeout(t)
  }
}

// Реальный URL установщика для тега. Имя ассета берём из релиза на GitHub,
// а не хардкодим GRUZ_<ver>_Setup.exe — иначе беты/переименования в CI
// или отсутствие сборки для тега уходят в 404 при скачивании.
// Возвращает null, если ассета-установщика нет (релиз без exe) или сеть упала.
export async function resolveSetupAsset(tag: string): Promise<string | null> {
  const ctrl = new AbortController()
  const t = setTimeout(() => ctrl.abort(), 10_000)
  try {
    const r = await fetch(`${API}/releases/tags/${tag}`, { signal: ctrl.signal })
    if (!r.ok) return null
    const data = await r.json()
    const assets: GithubAsset[] = data?.assets ?? []
    const pick = (re: RegExp) => assets.find(a => re.test(a.name))
    const found =
      pick(/^GRUZ_.*[_-]Setup\.exe$/i) ??
      pick(/_Setup\.exe$/i) ??
      pick(/[._-]setup\.exe$/i) ??
      pick(/\.exe$/i)
    return found ? found.browser_download_url : null
  } catch {
    return null
  } finally {
    clearTimeout(t)
  }
}

async function fetchChangelogUncached(): Promise<ReleaseChangelog[]> {
  const releases = await fetchReleases()
  if (releases.length === 0) return []

  // Сортируем от новых к старым
  const sorted = [...releases].sort(
    (a, b) => new Date(b.published_at).getTime() - new Date(a.published_at).getTime()
  )

  // Параллельно запрашиваем коммиты для каждого релиза
  const commitResults = await Promise.all(
    sorted.map((rel, i) => {
      const prev = sorted[i + 1]
      return (prev
        ? fetchCommitsBetween(prev.tag_name, rel.tag_name)
        : fetchCommitsUpTo(rel.tag_name)
      ).catch(() => [] as GithubCommit[])
    })
  )

  return sorted.map((rel, i) => {
    const rawCommits = commitResults[i]
    const commits: ParsedCommit[] = []
    let latestCounter = 0

    for (const c of rawCommits) {
      const parsed = parseCommitMessage(c.commit.message)
      if (!parsed) continue
      if (parsed.counter > latestCounter) latestCounter = parsed.counter
      if (parsed.lines.length > 0 || parsed.techLines.length > 0)
        commits.push({ counter: parsed.counter, lines: parsed.lines, techLines: parsed.techLines })
    }

    return {
      tag: rel.tag_name,
      name: rel.name || rel.tag_name,
      publishedAt: rel.published_at,
      prerelease: rel.prerelease,
      description: (rel.body || '').split(/\r?\n\r?\n/)[0].trim(),
      commits,
      totalUserLines: commits.reduce((s, c) => s + c.lines.length + c.techLines.length, 0),
      latestCounter,
    }
  })
}

// Считает (+N) — пользовательских изменений в релизах НОВЕЕ установленной версии.
// currentCounter = patch текущего бинарника (= счётчик коммита).
// Старые релизы (counter <= current) не считаются — это не «доступные обновления».
export function countNewChanges(changelogs: ReleaseChangelog[], currentCounter: number): number {
  let total = 0
  for (const rel of changelogs) {
    for (const commit of rel.commits) {
      if (commit.counter > currentCounter) total += commit.lines.length + commit.techLines.length
    }
  }
  return total
}

// Максимальный счётчик по всем коммитам (включая технические)
export function getLatestCounter(changelogs: ReleaseChangelog[]): number {
  return changelogs.reduce((max, rel) => Math.max(max, rel.latestCounter), 0)
}
