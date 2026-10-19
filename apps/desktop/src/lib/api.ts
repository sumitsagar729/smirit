import { type ApiError, type AppInfo, commands, type Id, type LibraryStats, type Source } from './bindings'

export type { ApiError, AppInfo, Id, LibraryStats, Source }

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
}

// In-memory stand-in so every screen can be developed and reviewed in a browser.
const mockSources: Source[] = []
const mock: typeof real = {
  appInfo: async () => ({ version: '0.1.0-preview', os: 'browser', arch: '' }),
  libraryStats: async () => ({
    root: 'C:\\Users\\you\\AppData\\Local\\Smriti',
    schemaVersion: 1,
    sources: mockSources.length,
    photos: 0,
    videos: 0,
    people: 0,
    pendingJobs: 0,
  }),
  listSources: async () => [...mockSources],
  addSource: async (path) => {
    if (mockSources.some((s) => s.path === path)) {
      throw new CommandError({ code: 'source_already_added', message: `${path} is already part of the library` })
    }
    const s: Source = {
      id: mockSources.length + 1,
      path,
      isOnline: true,
      addedAt: new Date().toISOString(),
      lastScanAt: null,
      photos: 0,
      videos: 0,
    }
    mockSources.push(s)
    return s
  },
  removeSource: async (id) => {
    const i = mockSources.findIndex((s) => s.id === id)
    if (i >= 0) mockSources.splice(i, 1)
    return null
  },
}

export const api = inTauri ? real : mock

/** Native folder picker; in a browser preview, asks for a path instead. */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return window.prompt('Folder path', 'D:\\photos and videos')
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ directory: true, multiple: false, title: 'Add a photo folder' })
  return typeof picked === 'string' ? picked : null
}
