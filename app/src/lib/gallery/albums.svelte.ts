// Стор альбомов (Svelte 5 runes). Обращается к bridge-командам.
import { commands } from '$lib/bridge/commands'
import type { Album } from '$lib/bridge/types'

export const albums = $state({
  list: [] as Album[],
  currentId: null as string | null,
  loading: false,
})

export async function loadAlbums() {
  albums.loading = true
  try {
    albums.list = await commands.listAlbums()
  } finally {
    albums.loading = false
  }
}

export async function createAlbum(name: string, kind = 'user', query?: string): Promise<string> {
  const id = await commands.createAlbum(name, kind, query)
  await loadAlbums()
  albums.currentId = id
  return id
}

export async function renameCurrent(name: string) {
  if (!albums.currentId) return
  await commands.renameAlbum(albums.currentId, name)
  await loadAlbums()
}

export async function deleteCurrent() {
  if (!albums.currentId) return
  const id = albums.currentId
  albums.currentId = null
  await commands.deleteAlbum(id)
  await loadAlbums()
}

export async function addSelected(ids: string[]) {
  if (!albums.currentId || ids.length === 0) return
  await commands.addToAlbum(albums.currentId, ids)
}

/** Добавить выбранные в произвольный альбом (без переключения текущего вида). */
export async function addToAlbum(albumId: string, ids: string[]) {
  if (!albumId || ids.length === 0) return
  await commands.addToAlbum(albumId, ids)
}

export async function removeSelected(ids: string[]) {
  if (!albums.currentId || ids.length === 0) return
  await commands.removeFromAlbum(albums.currentId, ids)
}

export function setCurrent(id: string | null) {
  albums.currentId = id
}
