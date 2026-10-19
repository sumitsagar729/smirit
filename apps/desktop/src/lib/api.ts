import {
  type ApiError,
  type AppInfo,
  type AssetDetail,
  commands,
  type Id,
  type IndexStatus,
  type LibraryStats,
  type Source,
  type Tile,
} from './bindings'
import { mock } from './mock'

export type { ApiError, AppInfo, AssetDetail, Id, IndexStatus, LibraryStats, Source, Tile }

/** Thrown for any failed command; `code` is stable, `message` is human-readable. */
export class CommandError extends Error {
  readonly code: string
  constructor(e: ApiError) {
    super(e.message)
    this.code = e.code
  }
}

type Res<T> = { status: 'ok'; data: T } | { status: 'error'; error: ApiError }

async function unwrap<T>(p: Promise<Res<T>>): Promise<T> {
  const r = await p
  if (r.status === 'error') throw new CommandError(r.error)
  return r.data
}

/** True inside the Tauri app; false when the UI runs in a plain browser (design previews). */
export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const real = {
  appInfo: () => commands.appInfo(),
  libraryStats: () => unwrap(commands.libraryStats()),
  listSources: () => unwrap(commands.listSources()),
  addSource: (path: string) => unwrap(commands.addSource(path)),
  removeSource: (id: Id) => unwrap(commands.removeSource(id)),
  rescan: () => commands.rescan(),
  indexStatus: () => commands.indexStatus(),
  timeline: () => unwrap(commands.timeline()),
  assetDetail: (id: Id) => unwrap(commands.assetDetail(id)),
}

export type Api = typeof real

export const api: Api = inTauri ? real : mock

/** Subscribes to indexing progress pushed by the core. */
export async function onIndexStatus(cb: (s: IndexStatus) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<IndexStatus>('index-status', (e) => cb(e.payload))
}

/** Native folder picker; in a browser preview, asks for a path instead. */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return window.prompt('Folder path', 'D:\\photos and videos')
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ directory: true, multiple: false, title: 'Add a photo folder' })
  return typeof picked === 'string' ? picked : null
}

export async function revealInExplorer(path: string) {
  if (!inTauri) return
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener')
  await revealItemInDir(path)
}

// The custom protocol is addressed differently per platform.
const isWindows = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
const base = isWindows ? 'http://smriti.localhost' : 'smriti://localhost'

export const thumbUrl = (id: Id) => `${base}/thumb/${id}`
export const mediaUrl = (id: Id) => `${base}/media/${id}`
