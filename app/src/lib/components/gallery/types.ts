import type { TaskState } from '$lib/bridge/types'

export type CardAction =
  | 'open-file'
  | 'open-folder'
  | 'cancel'
  | 'remove'
  | 'retry'
  | 'redownload'
  | 'delete'
  | 'copy-link'

export interface CardModel {
  kind: 'task' | 'history'
  id: string
  url: string | null
  video_id: string | null
  title: string | null
  thumbnail: string | null
  channel: string | null
  platform: string
  duration: number | null
  isAudio: boolean
  streamType: string | null
  isPlaylist: boolean
  playlistId: string | null
  width: number | null
  height: number | null
  format: string
  quality: string
  container: string
  fps: number | null
  source_fps: number | null
  video_codec: string | null
  audio_codec: string | null
  state: TaskState | null
  progress: number
  error: string | null
  file_path: string | null
  file_size: number | null
  created_at: number
  favorite: boolean
  localThumbnail: string | null
}
