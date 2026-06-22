// settings.svelte.ts — Svelte 5: экспортировать объект, не переприсваивать переменную
import { commands } from '$lib/bridge/commands'
import type { Settings } from '$lib/bridge/types'

export const store = $state({ settings: null as Settings | null })

// Поля с нестроковыми типами — нужна десериализация после updateSetting
const numFields  = new Set<keyof Settings>(['max_concurrent'])
const boolFields = new Set<keyof Settings>(['auto_merge','embed_subtitles','minimize_to_tray','ytdlp_auto_update'])

export async function loadSettings() {
  store.settings = await commands.getSettings()
}

export async function updateSetting(key: keyof Settings, value: string) {
  await commands.updateSetting(key, value)
  if (store.settings) {
    const s = store.settings as unknown as Record<string, unknown>
    if (numFields.has(key))       s[key] = Number(value)
    else if (boolFields.has(key)) s[key] = value === 'true'
    else                          s[key] = value
  }
}
