// Единственный источник мыслей для TitleBar
// Используется как фронтовым кодом (навигация), так и Tauri-событиями

import type { OrchestratorThought } from '$lib/bridge/events'

type ThoughtCallback = (t: OrchestratorThought) => void

let _cb: ThoughtCallback | null = null

export function registerThoughtCallback(cb: ThoughtCallback) {
  _cb = cb
}

export function emitThought(text: string, color: string, priority: number) {
  _cb?.({ text, color, priority })
}
