import { commands } from '$lib/bridge/commands'

export interface CrashReport {
  id: number
  type: string
  text: string
  time: number
}

interface CrashState {
  report: CrashReport | null
}

// Единый источник фатальных/необработанных ошибок.
// Заполняется из App.svelte (window.onerror, unhandledrejection, провал инициализации)
// и отображается ErrorModal как модалка поверх всего приложения.
export const crash = $state<CrashState>({ report: null })

export function reportCrash(type: string, err: unknown) {
  const text = err instanceof Error ? (err.stack || err.message) : String(err)
  crash.report = { id: Date.now(), type, text, time: Date.now() }
  commands.logFrontend(`[${type}] ${text}`, 'error').catch(() => {})
}

export function dismissCrash() {
  crash.report = null
}

function formatReport(r: CrashReport): string {
  const ts = new Date(r.time).toLocaleString()
  return `ГРУЗ — отчёт об ошибке\nВремя: ${ts}\nТип: ${r.type}\n\n${r.text}`
}

export async function copyCrashReport(): Promise<boolean> {
  const r = crash.report
  if (!r) return false
  try {
    await commands.writeText(formatReport(r))
    return true
  } catch {
    return false
  }
}
