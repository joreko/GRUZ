export type DeleteMode = 'history_only' | 'with_file'
export type DeleteContext = 'single' | 'selection' | 'clear-all'

interface DeleteState {
  context: DeleteContext
  count: number
  hasFile: boolean
  resolve: (mode: DeleteMode | null) => void
}

export const delDialog = $state<{ current: DeleteState | null }>({ current: null })

export function showDeleteDialog(
  context: DeleteContext,
  count = 1,
  hasFile = true,
): Promise<DeleteMode | null> {
  return new Promise((resolve) => {
    delDialog.current = { context, count, hasFile, resolve }
  })
}

export function dismissDeleteDialog() {
  const d = delDialog.current
  delDialog.current = null
  if (d) d.resolve(null)
}
