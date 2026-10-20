import {
  type ApiError,
  type AppInfo,
  type AssetDetail,
  type AssetFace,
  type Chip,
  commands,
  type FaceGroup,
  type FaceRef,
  type Home,
  type Id,
  type IndexStatus,
  type LibraryStats,
  type Person,
  type PersonRef,
  type PersonUpdate,
  type PlaceSummary,
  type Query,
  type Source,
  type Stop,
  type Suggestion,
  type Tile,
  type Trip,
} from './bindings'
import { mock } from './mock'

export type {
  ApiError,
  AppInfo,
  AssetDetail,
  AssetFace,
  Chip,
  FaceGroup,
  FaceRef,
  Home,
  Id,
  IndexStatus,
  LibraryStats,
  Person,
  PersonRef,
  PersonUpdate,
  PlaceSummary,
  Query,
  Source,
  Stop,
  Suggestion,
  Tile,
  Trip,
}

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
  timeline: (query: Query | null) => unwrap(commands.timeline(query)),
  assetDetail: (id: Id) => unwrap(commands.assetDetail(id)),
  people: () => unwrap(commands.people()),
  person: (id: Id) => unwrap(commands.person(id)),
  faceGroups: (limit: number) => unwrap(commands.faceGroups(limit)),
  groupFaces: (group: Id) => unwrap(commands.groupFaces(group)),
  personFaces: (person: Id, suggestions: boolean) => unwrap(commands.personFaces(person, suggestions)),
  assetFaces: (asset: Id) => unwrap(commands.assetFaces(asset)),
  nameGroup: (group: Id, to: PersonRef) => unwrap(commands.nameGroup(group, to)),
  assignFaces: (faces: Id[], to: PersonRef) => unwrap(commands.assignFaces(faces, to)),
  rejectFaces: (faces: Id[]) => unwrap(commands.rejectFaces(faces)),
  ignoreFaces: (faces: Id[]) => unwrap(commands.ignoreFaces(faces)),
  ignoreGroup: (group: Id) => unwrap(commands.ignoreGroup(group)),
  updatePerson: (id: Id, update: PersonUpdate) => unwrap(commands.updatePerson(id, update)),
  mergePeople: (from: Id, into: Id) => unwrap(commands.mergePeople(from, into)),
  deletePerson: (id: Id) => unwrap(commands.deletePerson(id)),
  setCoverFace: (person: Id, face: Id) => unwrap(commands.setCoverFace(person, face)),
  suggest: (text: string, today: string) => unwrap(commands.suggest(text, today)),
  places: () => unwrap(commands.places()),
  trips: () => unwrap(commands.trips()),
  trip: (id: Id) => unwrap(commands.trip(id)),
  renameTrip: (id: Id, title: string) => unwrap(commands.renameTrip(id, title)),
  home: () => unwrap(commands.home()),
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
export const faceUrl = (id: Id) => `${base}/face/${id}`
/** Map outlines (Natural Earth): `countries` or `states`. */
export const geoUrl = (name: 'countries' | 'states') => (inTauri ? `${base}/geo/${name}` : `/geo/${name}`)
