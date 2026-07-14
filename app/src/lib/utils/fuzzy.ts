// Нечёткий поиск: подпоследовательность + лёгкая толерантность к опечаткам.
// Чистые функции, без зависимостей. Регистр не важен.

export interface FuzzyResult<T> {
  item: T
  score: number
}

/**
 * Оценка сходства query и text.
 * Возвращает число > 0 (чем больше — тем лучше совпадение) или -1,
 * если query не является подпоследовательностью text.
 *
 * Бонусы:
 *  - совпадение в начале слова (после пробела/разделителя)
 *  - подряд идущие символы (run of consecutive matches)
 *  - раннее первое совпадение
 *  - полное равенство строк
 */
export function fuzzyScore(query: string, text: string): number {
  const q = query.trim().toLowerCase()
  const t = text.toLowerCase()
  if (q.length === 0) return 0
  if (t.length === 0) return -1
  if (q === t) return 1000 + (1000 - t.length) // точное совпадение — максимум

  let score = 0
  let qi = 0
  let prevMatchIdx = -2
  let run = 0
  let firstMatch = -1

  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      if (firstMatch === -1) firstMatch = ti
      // Бонус за начало слова
      const prevChar = ti > 0 ? t[ti - 1] : ' '
      if (prevChar === ' ' || prevChar === '-' || prevChar === '_' || prevChar === '/' || prevChar === '.') {
        score += 12
      }
      // Бонус за подряд идущее совпадение
      if (ti === prevMatchIdx + 1) {
        run += 1
        score += 8 + run * 2
      } else {
        run = 0
        score += 4
      }
      prevMatchIdx = ti
      qi += 1
    }
  }

  // Не все символы query найдены — не совпало
  if (qi < q.length) return -1

  // Бонус за раннее первое совпадение (чем раньше — тем лучше)
  score += Math.max(0, 30 - firstMatch)

  // Лёгкая толерантность к опечаткам: если длины близки, даём бонус
  const lenDiff = Math.abs(t.length - q.length)
  if (lenDiff <= 2) score += 6 - lenDiff * 2

  // Нормализация: длинные тексты с редким совпадением проигрывают
  score -= t.length * 0.05

  return score
}

/**
 * Отфильтровать и отсортировать items по нечёткому совпадению query
 * с полями, перечисленными в keys. Возвращает новый массив (исходный не меняет).
 */
export function fuzzyFilter<T>(
  items: T[],
  query: string,
  keys: (keyof T | ((item: T) => string | null | undefined))[],
): T[] {
  const q = query.trim()
  if (!q) return items.slice()

  const getStr = (k: (typeof keys)[number], item: T): string => {
    if (typeof k === 'function') return (k(item) as string) ?? ''
    const v = item[k]
    return v == null ? '' : String(v)
  }

  const results: FuzzyResult<T>[] = []
  for (const item of items) {
    let best = -1
    for (const k of keys) {
      const s = fuzzyScore(q, getStr(k, item))
      if (s > best) best = s
    }
    if (best >= 0) results.push({ item, score: best })
  }

  results.sort((a, b) => b.score - a.score)
  return results.map((r) => r.item)
}
