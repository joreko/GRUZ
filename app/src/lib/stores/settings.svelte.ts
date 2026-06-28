// settings.svelte.ts — Svelte 5: экспортировать объект, не переприсваивать переменную
import { commands } from '$lib/bridge/commands'
import type { Settings } from '$lib/bridge/types'

export const store = $state({ settings: null as Settings | null, error: null as string | null })

// Поля с нестроковыми типами — нужна десериализация после updateSetting
const numFields  = new Set<keyof Settings>(['max_concurrent', 'default_fps', 'default_bitrate'])
const boolFields = new Set<keyof Settings>(['auto_merge','embed_subtitles','minimize_to_tray'])

export async function loadSettings() {
  store.error = null
  try {
    store.settings = await commands.getSettings()
  } catch (e) {
    store.error = e instanceof Error ? e.message : String(e)
  }
}

export async function updateSetting(key: keyof Settings, value: string) {
  await commands.updateSetting(key, value)
  if (store.settings) {
    const s = store.settings as unknown as Record<string, unknown>
    if (numFields.has(key))       s[key] = value === '' ? null : Number(value)
    else if (boolFields.has(key)) s[key] = value === 'true'
    else                          s[key] = value
  }
}
