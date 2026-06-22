// Утилиты для работы с changelog через GitHub API

const REPO = 'joreko/GRUZ'
const API = `https://api.github.com/repos/${REPO}`

// Пользовательские типы — показываются в changelog и считаются в (+N)
const USER_TYPES = new Set(['добавлено', 'исправлено', 'улучшено', 'быстрее'])

export interface CommitLine {
  type: string       // тип коммита (добавлено, исправлено, ...)
  scope: string      // область (queue, downloader, ...)
  text: string       // описание изменения
}

export interface ParsedCommit {
  counter: number      // XXXX из заголовка
  lines: CommitLine[]  // отфильтрованные пользовательские строки
}

export interface ReleaseChangelog {
  tag: string
  name: string
  publishedAt: string
  commits: ParsedCommit[]
  totalUserLines: number  // сумма пользовательских строк — для (+N)
  latestCounter: number   // максимальный счётчик в релизе (включая технические коммиты)
}

// Версия в tauri.conf.json: "0.0.104" — patch это счётчик.
// В UI показываем только major.minor: "v0.0"
// При минор-релизе: "0.1.0" → "v0.1"
export function displayVersion(full: string): string {
  const [major, minor] = full.split('.')
  return `${major}.${minor}`
}

// Парсим сообщение коммита. Возвращает counter + пользовательские строки.
// counter берётся из заголовка даже если пользовательских строк нет —
// это важно для точного baseline.
function parseCommitMessage(message: string): { counter: number; lines: CommitLine[] } | null {
  const lines = message.trim().split('\n').map(l => l.trim()).filter(Boolean)
  if (lines.length === 0) return null

  const counter = parseInt(lines[0], 10)
  if (isNaN(counter)) return null

  const userLines: CommitLine[] = []
  for (const line of lines.slice(1)) {
    const m = line.match(/^([а-яёА-ЯЁ]+)(?:\(([^)]+)\))?:\s*(.+)$/)
    if (!m) continue
    const [, type, scope = '', text] = m
    if (USER_TYPES.has(type)) userLines.push({ type, scope, text })
  }

  return { counter, lines: userLines }
}

interface GithubRelease {
  tag_name: string
  name: string
  published_at: string
}

interface GithubCommit {
  sha: string
  commit: { message: string }
}

async function fetchReleases(): Promise<GithubRelease[]> {
  const r = await fetch(`${API}/releases?per_page=30`)
  if (!r.ok) throw new Error(`${r.status}`)
  return r.json()
}

// Коммиты между двумя тегами через compare API — точный SHA-диапазон
async function fetchCommitsBetween(base: string, head: string): Promise<GithubCommit[]> {
  const r = await fetch(`${API}/compare/${base}...${head}`)
  if (!r.ok) throw new Error(`${r.status}`)
  const data = await r.json()
  return data.commits ?? []
}

// Для самого первого релиза — берём все коммиты до тега
async function fetchCommitsUpTo(tag: string): Promise<GithubCommit[]> {
  const r = await fetch(`${API}/commits?sha=${tag}&per_page=100`)
  if (!r.ok) throw new Error(`${r.status}`)
  return r.json()
}

// Основная функция — загружает полный changelog по релизам параллельно
export async function fetchChangelog(): Promise<ReleaseChangelog[]> {
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
      if (parsed.lines.length > 0) commits.push({ counter: parsed.counter, lines: parsed.lines })
    }

    return {
      tag: rel.tag_name,
      name: rel.name || rel.tag_name,
      publishedAt: rel.published_at,
      commits,
      totalUserLines: commits.reduce((s, c) => s + c.lines.length, 0),
      latestCounter,
    }
  })
}

// Считает (+N) — пользовательских изменений после baseline-счётчика
export function countNewChanges(changelogs: ReleaseChangelog[], baselineCounter: number): number {
  let total = 0
  for (const rel of changelogs) {
    for (const commit of rel.commits) {
      if (commit.counter > baselineCounter) total += commit.lines.length
    }
  }
  return total
}

// Максимальный счётчик по всем коммитам (включая технические)
export function getLatestCounter(changelogs: ReleaseChangelog[]): number {
  return changelogs.reduce((max, rel) => Math.max(max, rel.latestCounter), 0)
}

// Baseline-счётчик — номер последнего известного коммита на момент установки
const BASELINE_KEY = 'gruz_baseline_counter'

export function getBaselineCounter(): number | null {
  const v = localStorage.getItem(BASELINE_KEY)
  return v !== null ? parseInt(v, 10) : null
}

export function setBaselineCounter(counter: number): void {
  localStorage.setItem(BASELINE_KEY, String(counter))
}
