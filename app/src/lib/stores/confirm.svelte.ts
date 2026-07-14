export type ConfirmDialog =
  | { type: 'confirm'; title: string; message: string; resolve: (v: boolean) => void }
  | { type: 'prompt'; title: string; message: string; defaultValue: string; resolve: (v: string | null) => void }

export const dialog = $state<{ current: ConfirmDialog | null }>({ current: null })

export function showConfirm(message: string, title = 'Подтверждение'): Promise<boolean> {
  return new Promise((resolve) => {
    dialog.current = { type: 'confirm', title, message, resolve }
  })
}

export function showPrompt(message: string, defaultValue = '', title = 'Ввод'): Promise<string | null> {
  return new Promise((resolve) => {
    dialog.current = { type: 'prompt', title, message, defaultValue, resolve }
  })
}

export function dismissDialog() {
  const d = dialog.current
  dialog.current = null
  if (d) {
    if (d.type === 'confirm') d.resolve(false)
    else d.resolve(null)
  }
}
