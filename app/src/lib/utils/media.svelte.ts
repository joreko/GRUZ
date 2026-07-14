import { convertFileSrc } from '@tauri-apps/api/core'

export function assetUrl(path: string | null | undefined): string | null {
  if (!path) return null
  if (path.startsWith('asset://') || path.startsWith('http://asset.localhost')) {
    return path
  }
  try {
    return convertFileSrc(path)
  } catch {
    return path
  }
}
