// In-memory stand-in for the Rust core so every screen can be developed and
// reviewed in a plain browser. Never used inside the desktop app.
import type { Api } from './api'
import type { ApiError, FaceGroup, FaceRef, Person, Source, Tile } from './bindings'

class MockError extends Error {
  readonly code: string
  constructor(e: ApiError) {
    super(e.message)
    this.code = e.code
  }
}

const sources: Source[] = []

// A deterministic fake library: bursts of photos on "event" days over 6 years.
function fakeTimeline(): Tile[] {
  let seed = 7
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647)
  const aspects = [4 / 3, 3 / 4, 16 / 9, 9 / 16, 1, 3 / 2, 2 / 3]
  const tiles: Tile[] = []
  let id = 1
  let t = new Date('2026-09-20T18:00:00').getTime()
  const end = new Date('2020-01-01').getTime()
  while (t > end && tiles.length < 1500) {
    const n = 1 + Math.floor(rnd() * 14)
    for (let i = 0; i < n; i++) {
      const video = rnd() < 0.08
      tiles.push({
        id: id++,
        aspect: aspects[Math.floor(rnd() * aspects.length)],
        takenAt: new Date(t - i * 60_000).toISOString().slice(0, 19),
        kind: video ? 'video' : 'photo',
        duration: video ? 5 + Math.floor(rnd() * 120) : null,
        hasThumb: false,
      })
    }
    t -= (1 + rnd() * 9) * 86_400_000
  }
  return tiles
}
const tiles = fakeTimeline()

// People: a few named, a few groups waiting for names. Face ids are fake, so
// avatars show initials in the browser preview.
let nextId = 1000
const people: Person[] = [
  { id: 1, name: 'Malika', birthDate: null, isFamily: true, coverFaceId: null, faceCount: 4529, photoCount: 3810 },
  { id: 2, name: 'Divik', birthDate: '2020-10-03', isFamily: true, coverFaceId: null, faceCount: 2554, photoCount: 2301 },
  { id: 3, name: 'Sumit', birthDate: null, isFamily: true, coverFaceId: null, faceCount: 2144, photoCount: 1987 },
]
let groups: FaceGroup[] = [412, 128, 64, 23, 9, 5].map((size, i) => ({
  id: 500 + i,
  size,
  faceIds: Array.from({ length: Math.min(size, 12) }, (_, k) => 5000 + i * 20 + k),
}))
const faceRefs = (n: number, kind: string | null): FaceRef[] =>
  tiles.slice(0, n).map((t, i) => ({ id: 9000 + i, assetId: t.id, kind, score: kind === 'auto' ? 0.42 + (i % 20) / 100 : null, takenAt: t.takenAt }))
const findPerson = (id: number) => {
  const p = people.find((x) => x.id === id)
  if (!p) throw new MockError({ code: 'not_found', message: `not found: person ${id}` })
  return p
}
const resolve = (to: { personId: number | null; name: string | null }) => {
  if (to.personId != null) return findPerson(to.personId)
  const name = (to.name ?? '').trim()
  if (!name) throw new MockError({ code: 'invalid', message: 'Please enter a name' })
  let p = people.find((x) => x.name.toLowerCase() === name.toLowerCase())
  if (!p) {
    p = { id: nextId++, name, birthDate: null, isFamily: false, coverFaceId: null, faceCount: 0, photoCount: 0 }
    people.push(p)
  }
  return p
}

export const mock: Api = {
  appInfo: async () => ({ version: '0.1.0-preview', os: 'browser', arch: '' }),
  libraryStats: async () => ({
    root: 'C:\\Users\\you\\AppData\\Local\\Smriti',
    schemaVersion: 2,
    sources: sources.length,
    photos: sources.length ? tiles.filter((t) => t.kind === 'photo').length : 0,
    videos: sources.length ? tiles.filter((t) => t.kind === 'video').length : 0,
    people: 0,
    pendingJobs: 0,
  }),
  listSources: async () => [...sources],
  addSource: async (path) => {
    if (sources.some((s) => s.path === path)) {
      throw new MockError({ code: 'source_already_added', message: `${path} is already part of the library` })
    }
    const s: Source = {
      id: sources.length + 1,
      path,
      isOnline: true,
      addedAt: new Date().toISOString(),
      lastScanAt: null,
      photos: 0,
      videos: 0,
    }
    sources.push(s)
    return s
  },
  removeSource: async (id) => {
    const i = sources.findIndex((s) => s.id === id)
    if (i >= 0) sources.splice(i, 1)
    return null
  },
  rescan: async () => {},
  indexStatus: async () => ({ phase: 'idle', total: 0, done: 0, failed: 0, generation: 0, peopleGeneration: 0 }),
  timeline: async (filter) => {
    if (!sources.length) return []
    if (!filter?.people.length) return tiles
    // Fake but stable: each person "appears" in a slice of the library.
    return tiles.filter((t) => filter.people.every((p) => (t.id * 31 + p * 17) % 5 < (filter.only ? 1 : 3)))
  },
  people: async () => (sources.length ? [...people] : []),
  person: async (id) => ({ ...findPerson(id) }),
  faceGroups: async (limit) => (sources.length ? groups.slice(0, limit) : []),
  groupFaces: async (group) => faceRefs(groups.find((g) => g.id === group)?.size ?? 0, null),
  personFaces: async (_person, suggestions) => faceRefs(suggestions ? 18 : 60, suggestions ? 'auto' : 'confirmed'),
  assetFaces: async (asset) =>
    asset % 3 === 0
      ? [
          { id: asset * 10, bbox: [0.22, 0.2, 0.14, 0.2], personId: 1, personName: 'Malika', confirmed: true },
          { id: asset * 10 + 1, bbox: [0.55, 0.3, 0.12, 0.17], personId: 2, personName: 'Divik', confirmed: false },
          { id: asset * 10 + 2, bbox: [0.8, 0.25, 0.08, 0.12], personId: null, personName: null, confirmed: false },
        ]
      : [],
  nameGroup: async (group, to) => {
    const g = groups.find((x) => x.id === group)
    const p = resolve(to)
    p.faceCount += g?.size ?? 0
    p.photoCount += g?.size ?? 0
    groups = groups.filter((x) => x.id !== group)
    return p.id
  },
  assignFaces: async (faces, to) => {
    const p = resolve(to)
    p.faceCount += faces.length
    return p.id
  },
  rejectFaces: async () => null,
  ignoreFaces: async () => null,
  ignoreGroup: async (group) => {
    groups = groups.filter((x) => x.id !== group)
    return null
  },
  updatePerson: async (id, u) => {
    const p = findPerson(id)
    if (!u.name.trim()) throw new MockError({ code: 'invalid', message: 'Please enter a name' })
    Object.assign(p, { name: u.name.trim(), birthDate: u.birthDate || null, isFamily: u.isFamily })
    return null
  },
  mergePeople: async (from, into) => {
    const a = findPerson(from)
    const b = findPerson(into)
    b.faceCount += a.faceCount
    b.photoCount += a.photoCount
    people.splice(people.indexOf(a), 1)
    return null
  },
  deletePerson: async (id) => {
    people.splice(people.indexOf(findPerson(id)), 1)
    return null
  },
  setCoverFace: async () => null,
  assetDetail: async (id) => {
    const t = tiles.find((x) => x.id === id)
    return {
      id,
      kind: t?.kind ?? 'photo',
      path: `D:\\photos and videos\\sample\\IMG_${id}.jpg`,
      fileName: `IMG_${id}.jpg`,
      folder: 'D:\\photos and videos\\sample',
      mime: 'image/jpeg',
      sizeBytes: 3_400_000,
      takenAt: t?.takenAt ?? null,
      takenAtSource: 'exif',
      width: 4000,
      height: 3000,
      camera: 'vivo V2027',
      lat: 12.97,
      lon: 77.59,
      duration: t?.duration ?? null,
    }
  },
}
