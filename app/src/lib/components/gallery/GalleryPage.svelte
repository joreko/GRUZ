<script module lang="ts">
  // Фоновый пересчёт превью запускаем один раз за сессию (общий для всех
  // экземпляров GalleryPage).
  let backfillStarted = false
</script>

<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { slide, fade, fly } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import { queue, refresh } from '$lib/stores/queue.svelte'
  import { commands } from '$lib/bridge/commands'
  import { onHistoryUpdated } from '$lib/bridge/events'
  import { tooltip } from '$lib/utils/tooltip'
  import { dl } from '$lib/stores/download.svelte'
  import { pushToast, dismissToast } from '$lib/stores/toast.svelte'
  import { showPrompt } from '$lib/stores/confirm.svelte'
  import { showDeleteDialog } from '$lib/stores/delete.svelte'
 import { selection, toggle as toggleSel, selectRange, selectAll, setSelection, clear as clearSel } from '$lib/gallery/selection.svelte'
 import { albums, loadAlbums, createAlbum, addToAlbum, setCurrent } from '$lib/gallery/albums.svelte'
  import type { HistoryItem, Route, DownloadTask } from '$lib/bridge/types'
  import GalleryCard from './GalleryCard.svelte'
  import GalleryLightbox from './GalleryLightbox.svelte'
  import VirtualGrid from './VirtualGrid.svelte'
  import type { CardModel, CardAction } from './types'

  let { route = $bindable<Route>('gallery') } = $props()

  let history = $state<HistoryItem[]>([])
  let loadingHistory = $state(false)
  let loadingMore = $state(false)
  let error = $state<string | null>(null)
  let hasMore = $state(true)
  let nextBeforeId = $state<string | null>(null)

  // Фильтры / поиск / сортировка / плотность / группировка
  let query = $state('')
  let debouncedQuery = $state('')
  let typeFilter = $state<'all' | 'video' | 'audio' | 'playlist'>('all')
  let platformFilter = $state<string>('all')
  let sortBy = $state<'new' | 'old' | 'size' | 'duration' | 'title'>('new')
  let density = $state<'comfortable' | 'compact'>('comfortable')
  let gridMode = $state<'grid' | 'masonry'>('grid')
  let groupBy = $state<'date' | 'year' | 'channel' | 'playlist' | 'none'>('none')
  let favoriteOnly = $state(false)

  // Запоминание режимов отображения галереи (сетка/мозаика, группировка,
  // плотность, «только избранное»). По умолчанию все выключены — значения
  // перезаписываются только если пользователь что-то менял и оно сохранено.
  type GalleryView = {
    gridMode?: 'grid' | 'masonry'
    groupBy?: 'date' | 'year' | 'channel' | 'playlist' | 'none'
    density?: 'comfortable' | 'compact'
    favoriteOnly?: boolean
  }
  const GALLERY_VIEW_KEY = 'gallery_view'
  let viewLoaded = $state(false)

  // Кастомные выпадающие списки (единый вид, JS-состояние для анимации)
  let platformOpen = $state(false)
  let sortOpen = $state(false)
  let copyOpen = $state(false)
  let albumOpen = $state(false)
  function openDD(name: 'platform' | 'sort' | 'copy' | 'album') {
    const cur =
      name === 'platform' ? platformOpen :
      name === 'sort' ? sortOpen :
      name === 'copy' ? copyOpen : albumOpen
    const next = !cur
    platformOpen = name === 'platform' && next
    sortOpen = name === 'sort' && next
    copyOpen = name === 'copy' && next
    albumOpen = name === 'album' && next
  }
  function closeDD() {
    platformOpen = sortOpen = copyOpen = albumOpen = false
  }
  function onDocPointerDown(e: PointerEvent) {
    if (!(e.target as HTMLElement).closest('.dd')) closeDD()
  }
  const sortLabels: Record<string, string> = {
    new: 'Сначала новые',
    old: 'Сначала старые',
    size: 'По размеру',
    duration: 'По длительности',
    title: 'По названию',
  }

  // Выбор
  let savedSearches = $state<string[]>([])

  // Лайтбокс
  let lightboxId = $state<string | null>(null)
  let lightboxOriginRect = $state<DOMRect | null>(null)
  let lightboxReturnEl = $state<HTMLElement | null>(null)

  // Таймеры
  let reloadTimer: ReturnType<typeof setTimeout> | null = null

  // Доступность
  let liveMsg = $state('')

  const albumId = $derived(albums.currentId)

  function taskToModel(t: DownloadTask): CardModel {
    return {
      kind: 'task',
      id: t.id,
      url: t.url,
      video_id: t.video_id,
      title: t.title,
      thumbnail: t.thumbnail,
      channel: t.channel,
      platform: t.platform,
      duration: t.duration,
      isAudio: t.format === 'audio',
      streamType: t.stream_type,
      isPlaylist: t.is_playlist,
      playlistId: null,
      width: null,
      height: null,
      format: t.format,
      quality: t.quality,
      container: t.container,
      fps: t.fps,
      source_fps: t.source_fps,
      video_codec: t.video_codec,
      audio_codec: t.audio_codec,
      state: t.state,
      progress: t.progress,
      error: t.error,
      file_path: t.file_path,
      file_size: t.file_size,
      created_at: Math.floor(new Date(t.created_at).getTime() / 1000),
      favorite: false,
      localThumbnail: null,
    }
  }

  function historyToModel(h: HistoryItem): CardModel {
    return {
      kind: 'history',
      id: h.id,
      url: h.url,
      video_id: h.video_id,
      title: h.title,
      thumbnail: h.thumbnail,
      channel: h.channel,
      platform: h.platform,
      duration: h.duration_real ?? h.duration,
      isAudio: h.format === 'audio',
      streamType: null,
      isPlaylist: h.playlist_id != null,
      playlistId: h.playlist_id,
      width: h.width,
      height: h.height,
      format: h.format,
      quality: h.quality,
      container: h.container,
      fps: h.fps,
      source_fps: h.source_fps,
      video_codec: h.video_codec,
      audio_codec: h.audio_codec,
      state: null,
      progress: 0,
      error: null,
      file_path: h.file_path,
      file_size: h.file_size,
      created_at: h.created_at,
      favorite: h.favorite,
      localThumbnail: h.local_thumbnail,
    }
  }

  const activeModels = $derived(
    queue.tasks
      .filter((t) => t.state !== 'completed' && t.state !== 'cancelled')
      .map(taskToModel),
  )

  const historyModels = $derived(history.map(historyToModel))

  const filteredModels = $derived.by(() => {
    let items = historyModels
    if (typeFilter === 'audio') items = items.filter((i) => i.isAudio)
    else if (typeFilter === 'video') items = items.filter((i) => !i.isAudio)
    else if (typeFilter === 'playlist') items = items.filter((i) => i.isPlaylist)
    if (platformFilter !== 'all') items = items.filter((i) => i.platform === platformFilter)
    if (favoriteOnly) items = items.filter((i) => i.favorite)

    // Поиск выполняется на стороне бэкенда (getHistory query) — здесь не дублируем.
    const sorted = [...items]
    switch (sortBy) {
      case 'old': sorted.sort((a, b) => a.created_at - b.created_at); break
      case 'size': sorted.sort((a, b) => (b.file_size ?? 0) - (a.file_size ?? 0)); break
      case 'duration': sorted.sort((a, b) => (b.duration ?? 0) - (a.duration ?? 0)); break
      case 'title': sorted.sort((a, b) => (a.title ?? '').localeCompare(b.title ?? '')); break
      default: sorted.sort((a, b) => b.created_at - a.created_at)
    }
    return sorted
  })

  // Параметры показываем только если одно видео скачано несколько раз
  const showParamsIds = $derived.by(() => {
    const byVideo = new Map<string, CardModel[]>()
    for (const m of filteredModels) {
      if (!m.video_id) continue
      const arr = byVideo.get(m.video_id) ?? []
      arr.push(m)
      byVideo.set(m.video_id, arr)
    }
    const show = new Set<string>()
    for (const arr of byVideo.values()) {
      if (arr.length < 2) continue
      const sig = (m: CardModel) => `${m.format}|${m.quality}|${m.container}|${m.fps}|${m.video_codec}|${m.audio_codec}`
      const sigs = new Set(arr.map(sig))
      if (sigs.size > 1) for (const m of arr) show.add(m.id)
    }
    return show
  })

  const platforms = $derived(
    Array.from(new Set(history.map((h) => h.platform).filter(Boolean))).sort(),
  )

  const groupedLibrary = $derived.by(() => groupModels(filteredModels, groupBy))

  const lightboxOrder = $derived([...activeModels, ...filteredModels])
  const orderedIds = $derived(lightboxOrder.map((m) => m.id))
  const lightboxIndex = $derived(lightboxOrder.findIndex((m) => m.id === lightboxId))
  const lightboxEntry = $derived(lightboxIndex >= 0 ? lightboxOrder[lightboxIndex] : null)

  // ── Группировка ──
  function groupModels(models: CardModel[], mode: string): { label: string; items: CardModel[] }[] {
    if (mode === 'year') {
      const m = new Map<number, CardModel[]>()
      for (const x of models) {
        const y = new Date(x.created_at * 1000).getFullYear()
        const arr = m.get(y) ?? []
        arr.push(x)
        m.set(y, arr)
      }
      return [...m.entries()]
        .sort((a, b) => b[0] - a[0])
        .map(([y, items]) => ({ label: String(y), items }))
    }
    if (mode === 'channel') {
      const m = new Map<string, CardModel[]>()
      for (const x of models) {
        const k = x.channel ?? 'Без канала'
        const arr = m.get(k) ?? []
        arr.push(x)
        m.set(k, arr)
      }
      return [...m.entries()]
        .sort((a, b) => {
          if (a[0] === 'Без канала') return 1
          if (b[0] === 'Без канала') return -1
          return a[0].localeCompare(b[0])
        })
        .map(([k, items]) => ({ label: k, items }))
    }
    if (mode === 'playlist') {
      const m = new Map<string, CardModel[]>()
      for (const x of models) {
        const k = x.isPlaylist ? (x.playlistId ?? 'Плейлист') : 'Без плейлиста'
        const arr = m.get(k) ?? []
        arr.push(x)
        m.set(k, arr)
      }
      return [...m.entries()]
        .sort((a, b) => {
          if (a[0] === 'Без плейлиста') return 1
          if (b[0] === 'Без плейлиста') return -1
          return a[0].localeCompare(b[0])
        })
        .map(([k, items]) => ({ label: k, items }))
    }
    if (mode === 'none') {
      return [{ label: '', items: models }]
    }
    // date (по умолчанию)
    const buckets: [string, CardModel[]][] = [
      ['Сегодня', []],
      ['Вчера', []],
      ['Эта неделя', []],
      ['Этот месяц', []],
      ['Ранее', []],
    ]
    const now = new Date()
    const today0 = new Date(now.getFullYear(), now.getMonth(), now.getDate())
    const todayS = today0.getTime() / 1000
    const yestS = todayS - 86400
    const weekS = todayS - 6 * 86400
    const monthS = new Date(now.getFullYear(), now.getMonth(), 1).getTime() / 1000
    for (const m of models) {
      const t = m.created_at
      if (t >= todayS) buckets[0][1].push(m)
      else if (t >= yestS) buckets[1][1].push(m)
      else if (t >= weekS) buckets[2][1].push(m)
      else if (t >= monthS) buckets[3][1].push(m)
      else buckets[4][1].push(m)
    }
    return buckets.filter(([, items]) => items.length).map(([label, items]) => ({ label, items }))
  }

  // ── Загрузка (курсорная пагинация) ──
  async function loadInitial(a: string | null, f: boolean, q: string | null = null) {
    loadingHistory = true
    error = null
    hasMore = true
    nextBeforeId = null
    try {
      const items = await commands.getHistory(null, 60, q || null, a, f ? true : null)
      history = items
      nextBeforeId = items.length ? items[items.length - 1].id : null
      hasMore = items.length >= 60
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      loadingHistory = false
    }
  }

  async function loadMore() {
    if (!hasMore || loadingMore || nextBeforeId == null) return
    loadingMore = true
    try {
      const items = await commands.getHistory(nextBeforeId, 60, debouncedQuery || null, albumId, favoriteOnly ? true : null)
      if (items.length) {
        history = [...history, ...items]
        nextBeforeId = items[items.length - 1].id
        hasMore = items.length >= 60
      } else {
        hasMore = false
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    } finally {
      loadingMore = false
    }
  }

  // Инкрементальное обновление без сброса списка и прыжка scrollTop.
  // Заменяем изменившиеся записи по id и дописываем новые (сверху),
  // сохраняя остальную часть списка и текущую прокрутку.
  async function upsertHistory() {
    try {
      const items = await commands.getHistory(null, 60, debouncedQuery || null, albumId, favoriteOnly ? true : null)
      const seen = new Set<string>()
      const merged: HistoryItem[] = []
      for (const it of items) {
        seen.add(it.id)
        merged.push(it)
      }
      for (const h of history) {
        if (!seen.has(h.id)) merged.push(h)
      }
      history = merged
    } catch {
      // тихо игнорируем — следующее событие поправит
    }
  }

  // Перезагрузка при смене альбома / фильтра избранного / поиска
  $effect(() => {
    const a = albumId
    const f = favoriteOnly
    const q = debouncedQuery
    loadInitial(a, f, q)
  })

  // Дебаунс поиска
  $effect(() => {
    const q = query
    const t = setTimeout(() => { debouncedQuery = q }, 200)
    return () => clearTimeout(t)
  })

  // Сохраняем режимы отображения галереи при любом их изменении.
  // viewLoaded гарантирует, что восстановление из хранилища не перезапишет
  // сохранённое значение дефолтными (по умолчанию все режимы выключены).
  $effect(() => {
    const snap = JSON.stringify({ gridMode, groupBy, density, favoriteOnly })
    if (!viewLoaded) return
    commands.updateSetting(GALLERY_VIEW_KEY, snap).catch(() => {})
  })

  // Объявление выделения для скринридеров
  $effect(() => {
    const n = selection.ids.size
    liveMsg = n > 0 ? `Выбрано ${n}` : ''
  })

  function announce(msg: string) {
    liveMsg = msg
  }

  let unlistenHistory: Promise<() => void> | null = null

  onMount(() => {
    const saved = localStorage.getItem('gruz.savedSearches')
    if (saved) {
      try { savedSearches = JSON.parse(saved) } catch { savedSearches = [] }
    }
    // Восстанавливаем ранее включённые режимы отображения галереи.
    commands.getSetting(GALLERY_VIEW_KEY).then((raw) => {
      if (raw) {
        try {
          const v = JSON.parse(raw) as GalleryView
          if (v.gridMode) gridMode = v.gridMode
          if (v.groupBy) groupBy = v.groupBy
          if (v.density) density = v.density
          if (typeof v.favoriteOnly === 'boolean') favoriteOnly = v.favoriteOnly
        } catch { /* битое значение — оставляем дефолты */ }
      }
      viewLoaded = true
    }).catch(() => { viewLoaded = true })
    loadAlbums()
    // Однократный фоновый пересчёт превью: вертикальные видео (rotation)
    // получают портретные превью вместо ландшафтных.
    if (!backfillStarted) {
      backfillStarted = true
      commands.backfillThumbnails().catch(() => {})
    }
    // Активные задачи уже реактивны через queue.tasks — полная перезагрузка
    // истории на каждый тик прогресса не нужна (иначе прыгает прокрутка).
    unlistenHistory = onHistoryUpdated(upsertHistory)
    document.addEventListener('pointerdown', onDocPointerDown)
  })

  onDestroy(async () => {
    if (reloadTimer) clearTimeout(reloadTimer)
    document.removeEventListener('pointerdown', onDocPointerDown)
    if (unlistenHistory) (await unlistenHistory)()
  })

  // ── Действия ──
  async function handleAction(action: CardAction, entry: CardModel) {
    try {
      switch (action) {
        case 'open-file':
          if (entry.file_path) await commands.openFile(entry.file_path)
          else error = 'Файл не найден.'
          break
        case 'open-folder':
          if (entry.file_path) await commands.openFolder(entry.file_path)
          else error = 'Папка не найдена.'
          break
        case 'cancel':
          await commands.cancelDownload(entry.id)
          await refresh()
          break
        case 'remove':
          await commands.removeTask(entry.id)
          await refresh()
          break
        case 'retry':
          await commands.removeTask(entry.id)
          await refresh()
          dl.url = entry.url ?? ''
          route = 'download'
          lightboxId = null
          break
        case 'redownload':
          dl.url = entry.url ?? ''
          route = 'download'
          lightboxId = null
          break
        case 'copy-link':
          if (entry.url) await commands.writeText(entry.url)
          break
        case 'delete':
          const mode = await showDeleteDialog('single', 1, !!entry.file_path)
          if (mode) await deleteEntries([entry.id], mode)
          break
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  async function deleteEntries(ids: string[], mode: 'history_only' | 'with_file' = 'history_only') {
    if (!ids.length) return
    const histIds = ids.filter((id) => history.some((h) => h.id === id))
    const taskIds = ids.filter((id) => queue.tasks.some((t) => t.id === id))
    const restored = history.filter((i) => histIds.includes(i.id))

    history = history.filter((i) => !histIds.includes(i.id))
    if (lightboxId && ids.includes(lightboxId)) lightboxId = null
    for (const id of ids) selection.ids.delete(id)

    try {
      if (histIds.length) {
        if (mode === 'with_file') {
          for (const id of histIds) await commands.deleteHistoryItemWithFile(id)
        } else {
          await commands.deleteHistoryItems(histIds)
        }
      }
      for (const id of taskIds) await commands.removeTask(id)
      if (taskIds.length) await refresh()
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }

    const n = ids.length
    pushToast({
      message: `Удалено: ${n}`,
      actionLabel: 'Отменить',
      onAction: async () => {
        for (const id of histIds) await commands.restoreHistoryItem(id)
        history = [...restored, ...history]
        announce('Восстановлено')
      },
    })
    announce(`Удалено ${n}`)
  }

  async function toggleFavorite(id: string) {
    const item = history.find((i) => i.id === id)
    if (!item) return
    const next = !item.favorite
    item.favorite = next // оптимистично (history — $state, мутация реактивна)
    try {
      await commands.setFavorite([id], next)
    } catch (e) {
      item.favorite = !next // откат
      error = e instanceof Error ? e.message : String(e)
    }
  }

  // ── Выбор ──
  function toggleSelect(id: string, shift: boolean) {
    if (shift && selection.lastId) selectRange(selection.lastId, id, orderedIds)
    else toggleSel(id)
  }

  function onRubberBand(ids: string[]) {
    setSelection(ids)
  }

  function clearSelection() {
    clearSel()
  }

  function selectAllVisible() {
    selectAll(orderedIds)
  }

  async function deleteSelected() {
    const mode = await showDeleteDialog('selection', selection.ids.size)
    if (mode) await deleteEntries([...selection.ids], mode)
  }

  // ── Очистка всего ──
  async function clearAll() {
    const mode = await showDeleteDialog('clear-all', history.length + queue.tasks.length)
    if (!mode) return
    try {
      if (mode === 'with_file') {
        const histIds = history.map((h) => h.id)
        for (const id of histIds) await commands.deleteHistoryItemWithFile(id)
      } else {
        await commands.clearHistory()
      }
      await commands.clearQueue()
      history = []
      await refresh()
      error = null
    } catch (e) {
      error = e instanceof Error ? e.message : String(e)
    }
  }

  function openLightbox(id: string, origin?: HTMLElement) {
    lightboxId = id
    lightboxReturnEl = origin ?? null
    lightboxOriginRect = origin ? origin.getBoundingClientRect() : null
  }

  // Мозаика: real thumbnail ratio → переписываем history целиком,
  // чтобы $derived цепочка схлопнулась и сетка пересчиталась.
  //
  // ВАЖНО: не затираем уже корректные размеры из БД. БД-поля width/height
  // вычислены ffmpeg из самого видео (верны, включая поворот вертикальных).
  // Удалённая обложка (YouTube) всегда 16:9 — если бы мы писали её размеры,
  // вертикальное 9:16 видео рисовалось бы ландшафтом, пока не нажмёшь
  // «Обновить». Перезаписываем ТОЛЬКО когда метаданных ещё нет — как
  // fallback по размерам самой картинки.
  function onAspect(id: string, width: number, height: number) {
    const idx = history.findIndex((it) => it.id === id)
    if (idx >= 0) {
      const prev = history[idx]
      if (!prev.width || !prev.height) {
        history = history.map((it) =>
          it.id === id ? { ...it, width, height } : it
        ) as HistoryItem[]
      }
    }
  }

  function navigateLightbox(dir: -1 | 1) {
    if (lightboxIndex < 0) return
    const next = lightboxIndex + dir
    if (next < 0 || next >= lightboxOrder.length) return
    lightboxId = lightboxOrder[next].id
    lightboxOriginRect = null
  }

  // ── Сохранённые поиски ──
  function saveQuery(q: string) {
    q = q.trim()
    if (!q) return
    savedSearches = [q, ...savedSearches.filter((x) => x !== q)].slice(0, 8)
    localStorage.setItem('gruz.savedSearches', JSON.stringify(savedSearches))
  }

  // ── Поделиться / копировать как ──
  function selectedModels(): CardModel[] {
    return filteredModels.filter((m) => selection.ids.has(m.id))
  }

  async function shareSelected() {
    const ms = selectedModels()
    const urls = ms.map((m) => m.url).filter(Boolean) as string[]
    const text = urls.join('\n')
    const nav = navigator as Navigator & { share?: (data: ShareData) => Promise<void> }
    if (typeof nav.share === 'function') {
      try { await nav.share({ title: 'ГРУЗ', text }) } catch { /* отмена */ }
    } else if (text) {
      await commands.writeText(text)
      pushToast({ message: 'Ссылки скопированы' })
    }
  }

  async function copyAs(format: 'md' | 'html' | 'bb' | 'path') {
    const ms = selectedModels()
    let text = ''
    if (format === 'path') {
      text = ms.map((m) => m.file_path).filter(Boolean).join('\n')
    } else if (format === 'md') {
      text = ms.map((m) => `[${m.title ?? m.url}](${m.url})`).join('\n')
    } else if (format === 'html') {
      text = ms.map((m) => `<a href="${m.url}">${m.title ?? m.url}</a>`).join('\n')
    } else if (format === 'bb') {
      text = ms.map((m) => `[url=${m.url}]${m.title ?? m.url}[/url]`).join('\n')
    }
    if (text) {
      await commands.writeText(text)
      pushToast({ message: 'Скопировано' })
    }
  }

  async function createAlbumPrompt() {
    const name = await showPrompt('Название альбома:', '', 'Новый альбом')
    if (!name || !name.trim()) return
    await createAlbum(name.trim(), 'user')
    pushToast({ message: 'Альбом создан' })
  }

  // ── Пересоздание превью для старых загрузок (fix: офлайн-превью) ──
  async function regenerateThumbs(ids: string[]) {
    if (!ids.length) return
    const toastId = pushToast({ message: `Пересоздание превью: ${ids.length}…`, duration: 0 })
    let ok = 0
    for (const id of ids) {
      try {
        await commands.generateThumbnail(id)
        ok++
      } catch {
        /* пропускаем неудачные */
      }
    }
    dismissToast(toastId)
    pushToast({ message: `Превью обновлены: ${ok}/${ids.length}` })
  }

  // ── Добавление в произвольный альбом из любого вида ──
  async function addToAlbumFromToolbar(albumId: string) {
    const ids = [...selection.ids]
    if (!ids.length) return
    await addToAlbum(albumId, ids)
    pushToast({ message: `Добавлено в альбом: ${ids.length}` })
  }

  async function createAlbumFromToolbar() {
    const name = await showPrompt('Название альбома:', '', 'Новый альбом')
    if (!name || !name.trim()) return
    const id = await createAlbum(name.trim(), 'user')
    await addToAlbum(id, [...selection.ids])
    pushToast({ message: 'Альбом создан и применён' })
  }

  // ── Клавиатура ──
  function onGridKey(e: KeyboardEvent) {
    if (lightboxId) return // клавиатуру полностью контролирует лайтбокс
    const ae = document.activeElement as HTMLElement | null
    const tag = (ae?.tagName || '').toLowerCase()
    const inField = tag === 'input' || tag === 'textarea' || !!ae?.isContentEditable

    if (!inField && ['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown'].includes(e.key)) {
      const cards = Array.from(document.querySelectorAll<HTMLElement>('[data-card]'))
      const idx = cards.indexOf(ae as HTMLElement)
      if (idx === -1) return
      e.preventDefault()
      const step = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : -1
      cards[(idx + step + cards.length) % cards.length]?.focus()
      return
    }

    if (inField) return

    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault()
      selectAllVisible()
      return
    }
    if (e.key === 'Delete' && selection.ids.size > 0) {
      e.preventDefault()
      deleteSelected()
      return
    }
    if (e.key === 'Escape') {
      closeDD()
      if (selection.ids.size > 0) clearSelection()
      else if (lightboxId) lightboxId = null
    }
  }

  const isEmpty = $derived(
    activeModels.length === 0 &&
    filteredModels.length === 0 &&
    !loadingHistory &&
    !query &&
    !debouncedQuery &&
    typeFilter === 'all' &&
    platformFilter === 'all' &&
    !favoriteOnly &&
    !albumId,
  )
  const hasContent = $derived(activeModels.length > 0 || history.length > 0)
</script>

<svelte:window onkeydown={onGridKey} />

<GalleryLightbox
  entry={lightboxEntry}
  hasPrev={lightboxIndex > 0}
  hasNext={lightboxIndex >= 0 && lightboxIndex < lightboxOrder.length - 1}
  originRect={lightboxOriginRect}
  returnFocusEl={lightboxReturnEl}
  onClose={() => lightboxId = null}
  onNavigate={navigateLightbox}
  onAction={handleAction}
/>

<div class="page">
  <div class="filter-row">
    <div class="seg" role="group" aria-label="Тип">
      <button class:active={typeFilter === 'all'} onclick={() => typeFilter = 'all'}>Все</button>
      <button class:active={typeFilter === 'video'} onclick={() => typeFilter = 'video'}>Видео</button>
      <button class:active={typeFilter === 'audio'} onclick={() => typeFilter = 'audio'}>Аудио</button>
      <button class:active={typeFilter === 'playlist'} onclick={() => typeFilter = 'playlist'}>Плейлисты</button>
    </div>

    <div class="dd dd-filter" class:open={platformOpen}>
      <button class="dd-trigger" aria-label="Платформа" title="Платформа" aria-expanded={platformOpen} onclick={() => openDD('platform')}>
        <span class="dd-filter-value">{platformFilter === 'all' ? 'Все платформы' : platformFilter}</span>
        <svg class="dd-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if platformOpen}
        <div class="dd-menu" transition:fly={{ y: -6, duration: 160 }}>
          <button class:active={platformFilter === 'all'} onclick={() => { platformFilter = 'all'; closeDD() }}>Все платформы</button>
          {#each platforms as p}
            <button class:active={platformFilter === p} onclick={() => { platformFilter = p; closeDD() }}>{p}</button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="dd dd-filter" class:open={sortOpen}>
      <button class="dd-trigger" aria-label="Сортировка" title="Сортировка" aria-expanded={sortOpen} onclick={() => openDD('sort')}>
        <span class="dd-filter-value">{sortLabels[sortBy]}</span>
        <svg class="dd-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
      </button>
      {#if sortOpen}
        <div class="dd-menu" transition:fly={{ y: -6, duration: 160 }}>
          <button class:active={sortBy === 'new'} onclick={() => { sortBy = 'new'; closeDD() }}>Сначала новые</button>
          <button class:active={sortBy === 'old'} onclick={() => { sortBy = 'old'; closeDD() }}>Сначала старые</button>
          <button class:active={sortBy === 'size'} onclick={() => { sortBy = 'size'; closeDD() }}>По размеру</button>
          <button class:active={sortBy === 'duration'} onclick={() => { sortBy = 'duration'; closeDD() }}>По длительности</button>
          <button class:active={sortBy === 'title'} onclick={() => { sortBy = 'title'; closeDD() }}>По названию</button>
        </div>
      {/if}
    </div>

    <div class="search">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
      <input
        type="text"
        placeholder="Поиск по названию, каналу, платформе…"
        bind:value={query}
        onkeydown={(e) => { if (e.key === 'Enter') saveQuery(query) }}
        aria-label="Поиск"
      />
      {#if query}
        <button class="search-clear" aria-label="Очистить" onclick={() => query = ''}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      {/if}
    </div>
  </div>

  <div class="action-row">
    <div class="action-left">
      <button class="btn-toggle" class:active={favoriteOnly} onclick={() => favoriteOnly = !favoriteOnly} aria-label="Только избранное" aria-pressed={favoriteOnly} use:tooltip={'Только избранное'}>
        <svg viewBox="0 0 24 24" fill={favoriteOnly ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
      </button>
      <button class="btn-toggle" class:active={groupBy !== 'none'} onclick={() => groupBy = groupBy === 'none' ? 'date' : 'none'} aria-label="По дате загрузки" aria-pressed={groupBy !== 'none'} use:tooltip={groupBy === 'none' ? 'Группировать по дате' : 'Без группировки'}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/><line x1="8" y1="14" x2="8" y2="14"/><line x1="12" y1="14" x2="12" y2="14"/><line x1="16" y1="14" x2="16" y2="14"/><line x1="8" y1="18" x2="8" y2="18"/><line x1="12" y1="18" x2="12" y2="18"/><line x1="16" y1="18" x2="16" y2="18"/></svg>
      </button>
      <button class="btn-toggle" class:active={gridMode === 'masonry'} onclick={() => gridMode = gridMode === 'masonry' ? 'grid' : 'masonry'} aria-label="Режим сетки" aria-pressed={gridMode === 'masonry'} use:tooltip={gridMode === 'masonry' ? 'Сетка' : 'Мозаика'}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/></svg>
      </button>
      <button class="btn-toggle" class:active={density === 'compact'} onclick={() => density = density === 'compact' ? 'comfortable' : 'compact'} aria-label="Плотность" use:tooltip={density === 'comfortable' ? 'Компактно' : 'Свободно'}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="4" y1="6" x2="20" y2="6"/><line x1="4" y1="12" x2="20" y2="12"/><line x1="4" y1="18" x2="20" y2="18"/></svg>
      </button>
      <button class="btn-toggle" onclick={() => loadInitial(albumId, favoriteOnly)} aria-label="Обновить" use:tooltip={'Обновить'} class:spin={loadingHistory}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/></svg>
      </button>
      <button class="btn-toggle" onclick={clearAll} aria-label="Очистить всё" use:tooltip={'Очистить всё'}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/></svg>
      </button>
    </div>
    <div class="action-right" class:has-selection={selection.ids.size > 0}>
      {#if selection.ids.size > 0}
        <div class="sel-tools-inner">
          <span class="sel-count">Выбрано: {selection.ids.size}</span>
          <button class="st-btn" onclick={selectAllVisible} use:tooltip={'Выбрать всё'}>Всё</button>
          <button class="st-btn" onclick={clearSelection}>Сброс</button>
          <button class="st-btn" onclick={shareSelected}>Поделиться</button>
          <div class="dd" class:open={copyOpen}>
            <button class="st-btn dd-trigger" aria-expanded={copyOpen} onclick={() => openDD('copy')}>Копировать как</button>
            {#if copyOpen}
              <div class="dd-menu" transition:fly={{ y: -6, duration: 160 }}>
                <button onclick={() => { copyAs('md'); closeDD() }}>Markdown</button>
                <button onclick={() => { copyAs('html'); closeDD() }}>HTML</button>
                <button onclick={() => { copyAs('bb'); closeDD() }}>BBCode</button>
                <button onclick={() => { copyAs('path'); closeDD() }}>Путь файла</button>
              </div>
            {/if}
          </div>
          <div class="dd" class:open={albumOpen}>
            <button class="st-btn dd-trigger" aria-expanded={albumOpen} onclick={() => openDD('album')}>Добавить в альбом ▾</button>
            {#if albumOpen}
              <div class="dd-menu dd-menu-wide" transition:fly={{ y: -6, duration: 160 }}>
                {#each albums.list as a (a.id)}
                  <button onclick={() => { addToAlbumFromToolbar(a.id); closeDD() }}>{a.name}</button>
                {/each}
                <button class="dd-create" onclick={() => { createAlbumFromToolbar(); closeDD() }}>＋ Создать альбом…</button>
              </div>
            {/if}
          </div>
          <button class="st-btn" onclick={() => regenerateThumbs([...selection.ids])} use:tooltip={'Пересоздать превью для выбранных'}>Пересоздать превью</button>
          <button class="st-btn danger" onclick={deleteSelected}>Удалить</button>
        </div>
      {/if}
    </div>
  </div>

  <!-- Альбомы -->
  <div class="albums-bar">
    <span class="ab-label">Альбомы</span>
    <button class="ab-chip" class:active={!albumId} onclick={() => setCurrent(null)}>Все</button>
    {#each albums.list as a (a.id)}
      <button class="ab-chip" class:active={albumId === a.id} onclick={() => setCurrent(a.id)}>{a.name}</button>
    {/each}
    <button class="ab-add" aria-label="Создать альбом" onclick={createAlbumPrompt} use:tooltip={'Создать альбом'}>+</button>
  </div>

  <!-- Сохранённые поиски -->
  {#if savedSearches.length}
    <div class="saved-row">
      <span class="saved-label">Быстрый поиск:</span>
      {#each savedSearches as s (s)}
        <button class="chip" onclick={() => query = s}>{s}</button>
      {/each}
    </div>
  {/if}



  {#if error}
    <div class="error-banner">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
      <span>{error}</span>
      <button class="btn-retry" onclick={() => loadInitial(albumId, favoriteOnly)}>Повторить</button>
    </div>
  {:else if loadingHistory && history.length === 0 && activeModels.length === 0}
    <div class="grid skeleton-grid" class:compact={density === 'compact'} in:fade={{ duration: 160 }}>
      {#each Array(6) as _}
        <div class="card-skel shimmer"></div>
      {/each}
    </div>
  {:else if isEmpty}
    <div class="empty-state" in:fade={{ duration: 160 }}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
        <polyline points="7 10 12 15 17 10"/>
        <line x1="12" y1="15" x2="12" y2="3"/>
      </svg>
      <p class="empty-title">Ничего нет</p>
      <p class="empty-hint">Скачайте видео — оно появится здесь</p>
    </div>
  {:else if filteredModels.length === 0 && activeModels.length === 0 && (debouncedQuery || typeFilter !== 'all' || platformFilter !== 'all' || favoriteOnly || albumId)}
    <div class="empty-state" in:fade={{ duration: 160 }}>
      <p class="empty-title">Ничего не найдено</p>
      <p class="empty-hint">Измените фильтры или поиск</p>
    </div>
  {:else}
    {#if activeModels.length > 0}
      <section class="active-section" transition:slide={{ duration: 240, easing: cubicOut }}>
        <h3 class="section-title">В процессе <span class="section-count">{activeModels.length}</span></h3>
        <div class="grid" class:compact={density === 'compact'}>
          {#each activeModels as m (m.id)}
            <GalleryCard
              entry={m}
              selected={selection.ids.has(m.id)}
              showParams={false}
               favorite={false}
              localThumbnail={null}
              onToggleSelect={toggleSelect}
              onOpen={openLightbox}
              onToggleFavorite={toggleFavorite}
            />
          {/each}
        </div>
      </section>
    {/if}

    <div class="grid-wrap" class:grid-fading={loadingHistory} in:fade={{ duration: 160 }}>
      <VirtualGrid
        groups={groupedLibrary}
        {density}
        mode={gridMode}
        onRubberBand={onRubberBand}
        onNearEnd={loadMore}
        {onAspect}
      >
        {#snippet card(item: CardModel, fill: boolean, onAspect, delay: number)}
          <GalleryCard
            entry={item}
            selected={selection.ids.has(item.id)}
            showParams={showParamsIds.has(item.id)}
            favorite={item.favorite}
            localThumbnail={item.localThumbnail}
            {fill}
            {delay}
            {onAspect}
            onToggleSelect={toggleSelect}
            onOpen={openLightbox}
            onToggleFavorite={toggleFavorite}
          />
        {/snippet}
      </VirtualGrid>
      {#if loadingMore}
        <div class="load-more">Загрузка ещё…</div>
      {/if}
    </div>
  {/if}
</div>

<div class="sr-only" aria-live="polite" role="status">{liveMsg}</div>

<style>
  .page {
    padding: var(--space-8) var(--space-9);
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    overflow: hidden;
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .action-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .action-left {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .action-right {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-inline-start: auto;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .sel-count { font-size: 11px; color: var(--text-secondary); font-weight: 500; white-space: nowrap; }

  .btn-toggle {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: 7px; color: var(--text-secondary); cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast), transform var(--transition-fast);
  }
  .btn-toggle svg { width: 14px; height: 14px; }
  .btn-toggle:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .btn-toggle:active { transform: scale(0.94); }
  .btn-toggle:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .btn-toggle.active { color: var(--accent-warm); border-color: color-mix(in srgb, var(--accent-warm) 45%, transparent); }
  .btn-toggle.spin svg { animation: spin-icon 0.7s linear infinite; }
  @keyframes spin-icon { to { transform: rotate(360deg); } }
  .search {
    display: flex; align-items: center; gap: 7px;
    flex: 1 1 auto;
    min-width: 160px;
    height: 32px; padding: 0 10px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    transition: border-color var(--transition-fast);
  }
  .search:focus-within { border-color: var(--border-strong); }
  .search svg { width: 14px; height: 14px; flex-shrink: 0; }
  .search input {
    flex: 1;
    background: transparent; border: none; outline: none;
    color: var(--text-primary);
    font-size: 13px;
    min-width: 0;
  }
  .search input::placeholder { color: var(--text-muted); }
  .search-clear {
    display: flex; align-items: center; justify-content: center;
    width: 18px; height: 18px; padding: 0;
    background: transparent; border: none; color: var(--text-muted); cursor: pointer;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
  }
  .search-clear:hover { color: var(--text-primary); }
  .search-clear svg { width: 13px; height: 13px; }

  .seg {
    display: flex;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    padding: 2px;
    flex-shrink: 0;
  }
  .seg button {
    height: 26px; padding: 0 10px;
    background: transparent; border: none; border-radius: 6px;
    color: var(--text-muted); font-size: 11px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .seg button:hover { color: var(--text-secondary); }
  .seg button.active { background: color-mix(in srgb, var(--scrim) 35%, transparent); color: var(--text-primary); font-weight: 600; }

  /* Единый вид выпадающих списков (фильтры платформы / сортировки) */
  .dd-filter {
    position: relative;
    flex-shrink: 0;
  }
  .dd-filter > .dd-trigger {
    list-style: none;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 10px;
    font-family: inherit;
    font-size: 12px;
    font-weight: 500;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .dd-filter > .dd-trigger:hover {
    background: var(--border-subtle);
    color: var(--text-primary);
    border-color: var(--border-strong);
  }
  .dd-filter > .dd-trigger:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .dd-filter.open > .dd-trigger {
    color: var(--text-primary);
    border-color: var(--accent);
  }
  .dd-filter-value { color: var(--text-primary); font-weight: 600; white-space: nowrap; }
  .dd-chevron {
    width: 13px; height: 13px; flex-shrink: 0;
    color: var(--text-muted);
    transition: transform var(--transition-fast), color var(--transition-fast);
  }
  .dd-filter.open .dd-chevron { transform: rotate(180deg); color: var(--text-secondary); }
  .dd-filter .dd-menu { min-width: 100%; }

  /* Альбомы */
  .albums-bar {
    display: flex; align-items: center; gap: 6px; flex-wrap: wrap;
    flex-shrink: 0;
  }
  .ab-label { font-size: 11px; font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.04em; }
  .ab-chip {
    height: 26px; padding: 0 11px;
    background: var(--bg-elevated); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-full); color: var(--text-secondary);
    font-size: 12px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .ab-chip:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .ab-chip:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .ab-chip.active { color: var(--text-primary); border-color: var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); }
  .ab-add {
    width: 26px; height: 26px; padding: 0;
    background: var(--bg-elevated); border: 1px solid var(--border-subtle);
    border-radius: var(--radius-full); color: var(--text-secondary);
    font-size: 16px; line-height: 1; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .ab-add:hover { background: var(--border-subtle); color: var(--text-primary); border-color: var(--border-strong); }
  .ab-add:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  /* Сохранённые поиски */
  .saved-row { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; flex-shrink: 0; }
  .saved-label { font-size: 11px; color: var(--text-muted); }
  .chip {
    height: 22px; padding: 0 9px;
    background: transparent; border: 1px solid var(--border-subtle);
    border-radius: var(--radius-full); color: var(--text-secondary);
    font-size: 11px; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .chip:hover { background: var(--bg-overlay); color: var(--text-primary); border-color: var(--border-strong); }
  .chip:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .sel-tools-inner {
    display: flex; align-items: center; gap: 6px;
    height: 28px; padding: 0 8px;
    background: color-mix(in srgb, var(--accent) 8%, var(--bg-elevated));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: var(--radius-md);
  }
  .st-btn {
    height: 26px; padding: 0 10px;
    background: var(--bg-overlay); border: 1px solid var(--border-default);
    border-radius: var(--radius-sm); color: var(--text-primary);
    font-size: 11px; font-weight: 500; cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .st-btn:hover { background: var(--border-subtle); border-color: var(--border-strong); }
  .st-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .st-btn.danger { color: var(--status-error); border-color: color-mix(in srgb, var(--status-error) 40%, transparent); }
  .st-btn.danger:hover { background: color-mix(in srgb, var(--status-error) 15%, transparent); border-color: var(--status-error); }
  .dd { position: relative; }
  .dd-menu {
    position: absolute; top: calc(100% + 5px); left: 0; z-index: 30;
    display: flex; flex-direction: column; gap: 3px;
    padding: 6px; min-width: 172px;
    background: var(--bg-elevated); border: 1px solid var(--border-default);
    border-radius: var(--radius-md); box-shadow: var(--shadow-menu);
    transform-origin: top center;
  }
  .dd-menu button {
    position: relative;
    height: 32px; padding: 0 12px; text-align: left;
    background: transparent; border: none; border-radius: var(--radius-sm);
    color: var(--text-secondary); font-size: 12px; font-weight: 500; cursor: pointer;
    display: flex; align-items: center; gap: 8px;
    white-space: nowrap;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .dd-menu button:hover { background: var(--bg-overlay); color: var(--text-primary); }
  .dd-menu button.active {
    color: var(--accent);
    background: var(--bg-selected);
    font-weight: 600;
  }
  .dd-menu-wide { min-width: 200px; max-height: 280px; overflow-y: auto; }
  .dd-create { color: var(--accent) !important; font-weight: 600; }

  /* Контент */
  .active-section { display: flex; flex-direction: column; gap: 8px; flex-shrink: 0; }
  .section-title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    display: flex; align-items: center; gap: 8px;
  }
  .section-count {
    font-size: 11px; font-weight: 600;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: var(--radius-full);
    padding: 1px 8px;
  }

  .grid-wrap {
    flex: 1 1 auto;
    min-height: 0;
    position: relative;
    transition: opacity 0.25s ease, filter 0.25s ease;
  }
  .grid-wrap.grid-fading {
    opacity: 0.5;
    filter: blur(1px);
    pointer-events: none;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }
  .grid.compact {
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 8px;
  }

  .card-skel {
    aspect-ratio: 16 / 9;
    border-radius: var(--radius-panel);
    background: var(--bg-overlay);
  }
  .skeleton-grid { pointer-events: none; }
  .shimmer { position: relative; overflow: hidden; }
  .shimmer::after {
    content: '';
    position: absolute; inset: 0;
    background: linear-gradient(90deg, transparent 0%, color-mix(in srgb, var(--on-scrim) 4%, transparent) 50%, transparent 100%);
    animation: shimmer-sweep 1.6s ease-in-out infinite;
  }
  @keyframes shimmer-sweep { 0% { transform: translateX(-100%); } 100% { transform: translateX(100%); } }

  .load-more {
    position: absolute; bottom: 8px; left: 50%; transform: translateX(-50%);
    padding: 4px 12px; font-size: 11px; color: var(--text-secondary);
    background: var(--bg-elevated); border: 1px solid var(--border-default);
    border-radius: var(--radius-full); pointer-events: none;
  }

  .empty-state {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 12px; padding: 60px 0; color: var(--text-muted);
  }
  .empty-state svg { width: 36px; height: 36px; opacity: 0.35; }
  .empty-title { margin: 0; font-size: 15px; font-weight: 500; color: var(--text-secondary); }
  .empty-hint { margin: 0; font-size: 13px; color: var(--text-muted); }

  .error-banner {
    display: flex; align-items: center; gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: color-mix(in srgb, var(--status-error) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-error) 30%, transparent);
    border-radius: var(--radius-md); color: var(--status-error);
    font-size: var(--text-sm);
    flex-shrink: 0;
  }
  .error-banner svg { width: 16px; height: 16px; flex-shrink: 0; }
  .btn-retry {
    margin-left: auto; padding: var(--space-1) var(--space-3);
    background: color-mix(in srgb, var(--status-error) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-error) 30%, transparent);
    border-radius: var(--radius-sm); color: var(--status-error);
    font-size: var(--text-xs); cursor: pointer;
    transition: background var(--transition-fast);
  }
  .btn-retry:hover { background: color-mix(in srgb, var(--status-error) 25%, transparent); }

  .sr-only {
    position: absolute; width: 1px; height: 1px;
    padding: 0; margin: -1px; overflow: hidden;
    clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0;
  }
</style>
