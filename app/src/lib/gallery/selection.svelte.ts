// Стор выделения галереи (Svelte 5 runes).
// Чистые хелперы + реактивное состояние. Framework-agnostic по сути:
// никаких DOM-вызовов, только данные.

import { SvelteSet } from 'svelte/reactivity'

export interface Rect {
  left: number
  top: number
  right: number
  bottom: number
}

export const selection = $state({
  ids: new SvelteSet<string>(),
  lastId: null as string | null,
})

/** Переключить один элемент. Обновляет lastId для shift-range. */
export function toggle(id: string) {
  if (selection.ids.has(id)) selection.ids.delete(id)
  else selection.ids.add(id)
  selection.lastId = id
}

/** Выбрать только один элемент (сброс остальных). */
export function selectOnly(id: string) {
  selection.ids.clear()
  selection.ids.add(id)
  selection.lastId = id
}

/** Выбрать диапазон от lastId до id по упорядоченному списку. */
export function selectRange(fromId: string, toId: string, orderedIds: string[]) {
  const a = orderedIds.indexOf(fromId)
  const b = orderedIds.indexOf(toId)
  if (a === -1 || b === -1) {
    selectOnly(toId)
    return
  }
  const [lo, hi] = a < b ? [a, b] : [b, a]
  selection.ids.clear()
  for (let i = lo; i <= hi; i++) selection.ids.add(orderedIds[i])
  selection.lastId = toId
}

/** Добавить все перечисленные в текущее выделение. */
export function selectAll(ids: string[]) {
  for (const id of ids) selection.ids.add(id)
}

/** Жёстко заменить выделение списком (для rubber-band). */
export function setSelection(ids: string[]) {
  selection.ids.clear()
  for (const id of ids) selection.ids.add(id)
  selection.lastId = ids.length ? ids[ids.length - 1] : null
}

export function clear() {
  selection.ids.clear()
  selection.lastId = null
}

export function has(id: string): boolean {
  return selection.ids.has(id)
}

export function count(): number {
  return selection.ids.size
}

/**
 * Вернуть id карточек, пересекающихся с прямоугольником выделения.
 * rect и cardRects — в одной системе координат (контент скролла).
 */
export function computeSelection(rect: Rect, cardRects: { id: string; rect: Rect }[]): string[] {
  const out: string[] = []
  for (const c of cardRects) {
    if (
      rect.left <= c.rect.right &&
      rect.right >= c.rect.left &&
      rect.top <= c.rect.bottom &&
      rect.bottom >= c.rect.top
    ) {
      out.push(c.id)
    }
  }
  return out
}
