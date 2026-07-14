<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { CardModel } from './types'
  import { computeSelection, type Rect } from '$lib/gallery/selection.svelte'
  import { aspectStore } from '$lib/gallery/aspectStore.svelte'

  interface Props {
    groups: { label: string; items: CardModel[] }[]
    density: 'comfortable' | 'compact'
    mode?: 'grid' | 'masonry'
    card: Snippet<[CardModel, boolean, ((id: string, width: number, height: number) => void) | undefined, number]>
    onRubberBand?: (ids: string[]) => void
    onNearEnd?: () => void
    onAspect?: (id: string, width: number, height: number) => void
  }

  let {
    groups,
    density,
    mode = 'grid',
    card,
    onRubberBand,
    onNearEnd,
    onAspect,
  }: Props = $props()

  // Чистая волна сверху вниз: задержка растёт только по вертикали (y),
  // карточки одной строки (одинаковый y) появляются синхронно. Без случайности.
  function staggerDelay(y: number, _id: string): number {
    return Math.round(Math.max(0, y) * 0.3)
  }

  const HEADER_H = 30
  const BUFFER = 700

  let scrollEl = $state<HTMLDivElement | null>(null)
  let contentEl = $state<HTMLDivElement | null>(null)
  let containerWidth = $state(0)
  let viewportH = $state(0)
  let scrollTop = $state(0)
  let rafPending = false
  // Transition трансформации вкл. только ПОСЛЕ первой раскладки: иначе смена
  // transform при измерении ширины (containerWidth 0 → реальная) «провозит»
  // карточки слева направо из-за CSS-перехода на .vg-item.
  let settled = $state(false)

  interface Positioned {
    item: CardModel
    x: number
    y: number
    w: number
    h: number
    groupIndex: number
  }
  interface GroupRange {
    label: string
    top: number
    bottom: number
    groupIndex: number
  }

  // Реальное соотношение сторон карточки для мозаики.
  // 1) размеры из БД (заполняются при генерации превью — родные пропорции
  //    видео, включая поворот вертикальных), 2) реальные размеры загруженной
  //    картинки, 3) квадрат для аудио, 4) 16:9 по умолчанию.
  function aspectRatio(item: CardModel): number {
    if (item.height && item.width) return item.height / item.width
    const a = aspectStore[item.id]
    if (a && a[0] && a[1]) return a[1] / a[0]
    if (item.isAudio) return 1
    return 9 / 16
  }

  const layout = $derived.by(() => {
    const w = containerWidth || 1
    const isMasonry = mode === 'masonry'

    // Мозаика — плотнее и колонки уже (дух Pinterest), сетка — крупнее.
    const gap = isMasonry
      ? density === 'compact' ? 10 : 14
      : density === 'compact' ? 8 : 12
    const minCol = isMasonry
      ? density === 'compact' ? 150 : 208
      : density === 'compact' ? 160 : 220
    const columns = Math.max(1, Math.floor((w + gap) / (minCol + gap)))
    const colW = (w - gap * (columns - 1)) / columns

    const positioned: Positioned[] = []
    const ranges: GroupRange[] = []

    let y = 0
    groups.forEach((group, gi) => {
      const top = y
      ranges.push({ label: group.label, top, bottom: 0, groupIndex: gi })
      if (group.label) y += HEADER_H

      if (!isMasonry) {
        const rowH = colW * (9 / 16)
        const items = group.items
        const rows = Math.ceil(items.length / columns)
        for (let r = 0; r < rows; r++) {
          const rowTop = y + r * (rowH + gap)
          for (let c = 0; c < columns; c++) {
            const idx = r * columns + c
            if (idx >= items.length) break
            const item = items[idx]
            positioned.push({
              item,
              x: c * (colW + gap),
              y: rowTop,
              w: colW,
              h: rowH,
              groupIndex: gi,
            })
          }
        }
        y += rows * (rowH + gap)
      } else {
        // masonry: каждый элемент в самую короткую колонку (Pinterest-упаковка)
        const colHeights = new Array(columns).fill(y)
        for (const item of group.items) {
          const ratio = aspectRatio(item)
          // Клампим экстремальные пропорции, чтобы не было «спичек» и
          // гигантских простыней. Минимальная высота — относительная (от
          // ширины колонки), чтобы 16:9 не искажался в «компактно»
          // (иначе абсолютный пол вздувал бы узкие колонки до ~4:3).
          const minH = density === 'compact' ? colW * 0.5 : 120
          const ih = Math.min(colW * 2.4, Math.max(minH, colW * ratio))
          let ci = 0
          for (let k = 1; k < columns; k++) {
            if (colHeights[k] < colHeights[ci]) ci = k
          }
          const x = ci * (colW + gap)
          const iy = colHeights[ci]
          positioned.push({ item, x, y: iy, w: colW, h: ih, groupIndex: gi })
          colHeights[ci] = iy + ih + gap
        }
        y = Math.max(...colHeights)
      }
      y += gap
    })

    // Закрываем высоты групп
    for (let i = 0; i < ranges.length; i++) {
      ranges[i].bottom = i + 1 < ranges.length ? ranges[i + 1].top : y
    }

    return { positioned, ranges, totalHeight: y }
  })

  const visible = $derived.by(() => {
    const top = scrollTop - BUFFER
    const bottom = scrollTop + viewportH + BUFFER
    return layout.positioned.filter(
      (p) => p.y + p.h >= top && p.y <= bottom,
    )
  })

  const floatingLabel = $derived.by(() => {
    if (scrollTop <= 2) return null
    for (const r of layout.ranges) {
      if (scrollTop >= r.top && scrollTop < r.bottom) return r.label
    }
    return null
  })

  // ── Скролл / измерение ──
  function onScroll() {
    if (rafPending) return
    rafPending = true
    requestAnimationFrame(() => {
      rafPending = false
      if (!scrollEl) return
      scrollTop = scrollEl.scrollTop
      if (
        onNearEnd &&
        layout.totalHeight > 0 &&
        scrollEl.scrollTop + scrollEl.clientHeight > layout.totalHeight - 600
      ) {
        onNearEnd()
      }
    })
  }

  $effect(() => {
    const el = scrollEl
    if (!el) return
    const measure = () => {
      // Ширину меряем по внутреннему контенту, а не по скролл-контейнеру:
      // у .vg-scroll есть горизонтальный padding (отступ скроллбара от сетки),
      // иначе колонки считались бы шире реальной области.
      containerWidth = contentEl ? contentEl.clientWidth : el.clientWidth
      viewportH = el.clientHeight
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    // Включаем transition только после первого кадра с правильной раскладкой.
    requestAnimationFrame(() => { settled = true })
    return () => ro.disconnect()
  })

  // ── Rubber-band выделение ──
  let dragging = $state(false)
  let dragRect = $state<Rect | null>(null)

  function contentCoords(e: PointerEvent): { x: number; y: number } {
    const el = contentEl
    if (!el) return { x: 0, y: 0 }
    const rect = el.getBoundingClientRect()
    // Координаты относительно .vg-content (туда же позиционируются карточки),
    // без учёта padding скролл-контейнера и scrollTop — rect уже сдвинут скроллом.
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest('[data-card]')) return // клик по карточке — не резинка
    const { x, y } = contentCoords(e)
    dragging = true
    dragRect = { left: x, top: y, right: x, bottom: y }
    scrollEl?.setPointerCapture(e.pointerId)
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging || !dragRect) return
    const { x, y } = contentCoords(e)
    const rect: Rect = {
      left: Math.min(dragRect.left, x),
      top: Math.min(dragRect.top, y),
      right: Math.max(dragRect.right, x),
      bottom: Math.max(dragRect.bottom, y),
    }
    dragRect = rect
    const cardRects = layout.positioned.map((p) => ({
      id: p.item.id,
      rect: { left: p.x, top: p.y, right: p.x + p.w, bottom: p.y + p.h },
    }))
    const ids = computeSelection(rect, cardRects)
    onRubberBand?.(ids)
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragging) return
    dragging = false
    dragRect = null
    scrollEl?.releasePointerCapture(e.pointerId)
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="vg-scroll"
  bind:this={scrollEl}
  onscroll={onScroll}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
>
  {#if floatingLabel}
    <div class="vg-sticky" style="transform: translateY({scrollTop}px); height: {HEADER_H}px;">
      <span>{floatingLabel}</span>
    </div>
  {/if}

  <div class="vg-content" bind:this={contentEl} style="height: {layout.totalHeight}px;">
    {#each layout.ranges as h (h.groupIndex)}
      {#if h.label}
        <div class="vg-group-head" style="transform: translate(0, {h.top}px); height: {HEADER_H}px;">
          {h.label}
        </div>
      {/if}
    {/each}

    {#each visible as p (p.item.id)}
      <div
        class="vg-item"
        class:settled={settled}
        style="transform: translate({p.x}px, {p.y}px); width: {p.w}px; height: {p.h}px;"
      >
        {@render card(p.item, true, onAspect, staggerDelay(p.y, p.item.id))}
      </div>
    {/each}

    {#if dragRect}
      <div
        class="vg-rubber"
        style="left: {dragRect.left}px; top: {dragRect.top}px; width: {Math.max(0, dragRect.right - dragRect.left)}px; height: {Math.max(0, dragRect.bottom - dragRect.top)}px;"
      ></div>
    {/if}
  </div>
</div>

<style>
  .vg-scroll {
    position: relative;
    height: 100%;
    width: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    /* Отступ справа — скроллбар не прижат к сетке, а висит снаружи с зазором.
       scrollbar-gutter: stable резервирует место под скроллбар всегда,
       поэтому сетка не дёргается (не сдвигается) при его появлении. */
    padding: 8px 12px 8px 0;
    scrollbar-gutter: stable;
  }
  .vg-scroll::-webkit-scrollbar { width: 7px; height: 7px; }
  .vg-scroll::-webkit-scrollbar-track { background: transparent; }
  .vg-scroll::-webkit-scrollbar-thumb {
    background: var(--border-default);
    border-radius: 9999px;
    border: 1px solid transparent;
    background-clip: content-box;
    min-height: 40px;
  }
  .vg-scroll::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
    border: 1px solid transparent;
    background-clip: content-box;
  }
  .vg-scroll::-webkit-scrollbar-corner { background: transparent; }
  .vg-content {
    position: relative;
    width: 100%;
  }
  .vg-item {
    position: absolute;
    top: 0;
    left: 0;
    will-change: transform;
  }
  .vg-item.settled {
    transition: transform 0.32s cubic-bezier(0.22, 1, 0.36, 1),
                width 0.32s cubic-bezier(0.22, 1, 0.36, 1),
                height 0.32s cubic-bezier(0.22, 1, 0.36, 1);
  }
  .vg-group-head {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    padding: 0 4px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
    pointer-events: none;
    background: linear-gradient(to bottom, var(--bg-content) 60%, transparent);
    z-index: 2;
    animation: gh-in 0.3s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  @keyframes gh-in {
    from { opacity:0; }
    to   { opacity:1; }
  }
  .vg-sticky {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    padding: 0 12px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-secondary);
    background: color-mix(in srgb, var(--bg-content) 88%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--border-subtle);
    pointer-events: none;
    z-index: 7;
  }
  .vg-rubber {
    position: absolute;
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    pointer-events: none;
    z-index: 8;
  }
</style>
