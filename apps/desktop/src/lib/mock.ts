// In-memory stand-in for the Rust core so every screen can be developed and
// reviewed in a plain browser. Never used inside the desktop app.
import type { Api } from './api'
import type { ApiError, FaceGroup, FaceRef, Person, PlaceSummary, Source, Suggestion, Tile, Trip } from './bindings'

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

// Places and trips around a Bengaluru home.
const place = (id: number, name: string, admin1: string, lat: number, lon: number, count: number): PlaceSummary => ({
  id,
  name,
  admin1,
  country: 'India',
  lat,
  lon,
  count,
  coverAssetId: null,
})
const mockPlaces: PlaceSummary[] = [
  place(1, 'Bengaluru', 'Karnataka', 12.97, 77.59, 4216),
  place(2, 'Ooty', 'Tamil Nadu', 11.41, 76.7, 1062),
  place(3, 'Coorg', 'Karnataka', 12.42, 75.74, 439),
  place(4, 'Goa', 'Goa', 15.49, 73.83, 295),
  place(5, 'Mysuru', 'Karnataka', 12.3, 76.65, 197),
  place(6, 'Dhanbad', 'Jharkhand', 23.8, 86.43, 178),
  place(7, 'Delhi', 'Delhi', 28.65, 77.23, 173),
  place(8, 'Munnar', 'Kerala', 10.09, 77.06, 78),
]
const stop = (name: string, lat: number, lon: number, start: string, end: string, count: number) => ({ name, lat, lon, start, end, count })
const mockTrips: Trip[] = [
  { id: 11, title: 'Ooty', start: '2021-11-02', end: '2021-11-07', distanceKm: 198, stops: [stop('Vellore', 12.92, 79.13, '2021-11-02', '2021-11-02', 40), stop('Ooty', 11.41, 76.7, '2021-11-03', '2021-11-07', 900)], count: 2377, coverAssetId: null, userEdited: false },
  { id: 12, title: 'Goa', start: '2021-10-09', end: '2021-10-12', distanceKm: 508, stops: [stop('Goa', 15.49, 73.83, '2021-10-09', '2021-10-12', 295)], count: 2533, coverAssetId: null, userEdited: false },
  { id: 13, title: 'Dhanbad, Delhi & Jammu', start: '2018-04-18', end: '2018-05-11', distanceKm: 2224, stops: [stop('Dhanbad', 23.8, 86.43, '2018-04-18', '2018-04-25', 120), stop('Delhi', 28.65, 77.23, '2018-04-26', '2018-04-30', 90), stop('Jammu', 32.73, 74.86, '2018-05-01', '2018-05-08', 119)], count: 430, coverAssetId: null, userEdited: false },
  { id: 14, title: 'Coorg', start: '2017-06-24', end: '2017-06-26', distanceKm: 219, stops: [stop('Coorg', 12.42, 75.74, '2017-06-24', '2017-06-26', 439)], count: 442, coverAssetId: null, userEdited: false },
]

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
  indexStatus: async () => ({ phase: 'idle', total: 0, done: 0, failed: 0, generation: 0, peopleGeneration: 0, placesGeneration: 0 }),
  timeline: async (q) => {
    if (!sources.length) return []
    if (!q) return tiles
    // Fake but stable: every filter keeps a slice of the library.
    let out = tiles
    for (const p of q.people ?? []) out = out.filter((t) => (t.id * 31 + p * 17) % 5 < (q.only ? 1 : 3))
    if (q.kind) out = out.filter((t) => t.kind === q.kind)
    if (q.from) out = out.filter((t) => (t.takenAt ?? '') >= q.from!)
    if (q.to) out = out.filter((t) => (t.takenAt ?? '').slice(0, 10) <= q.to!)
    if (q.place != null || q.trip != null) out = out.filter((t) => (t.id + (q.place ?? q.trip ?? 0)) % 7 === 0)
    return out
  },
  suggest: async (text) => {
    const t = text.trim().toLowerCase()
    const all: Suggestion[] = [
      ...people.map((p) => ({ chip: { type: 'person' as const, id: p.id, name: p.name, faceId: null }, label: p.name, detail: `${p.photoCount} photos` })),
      ...mockPlaces.map((p) => ({ chip: { type: 'place' as const, id: p.id, name: p.name }, label: p.name, detail: `${p.admin1} · ${p.count} photos` })),
      ...mockTrips.map((tr) => ({ chip: { type: 'trip' as const, id: tr.id, title: tr.title }, label: `${tr.title} trip`, detail: tr.start })),
      { chip: { type: 'kind', kind: 'video' }, label: 'Videos', detail: null },
      { chip: { type: 'onThisDay' }, label: 'On this day', detail: 'This date in past years' },
      { chip: { type: 'only' }, label: 'No one else', detail: 'Only the chosen people in the photo' },
    ]
    if (/^(19|20)\d\d$/.test(t)) {
      return [{ chip: { type: 'dates', from: `${t}-01-01`, to: `${t}-12-31`, label: t }, label: t, detail: 'Year' }]
    }
    return (t ? all.filter((s) => s.label.toLowerCase().includes(t)) : all.slice(0, 6)).slice(0, 8)
  },
  places: async () => (sources.length ? mockPlaces : []),
  trips: async () => (sources.length ? mockTrips : []),
  trip: async (id) => {
    const t = mockTrips.find((x) => x.id === id)
    if (!t) throw new MockError({ code: 'not_found', message: `not found: trip ${id}` })
    return t
  },
  renameTrip: async (id, title) => {
    const t = mockTrips.find((x) => x.id === id)
    if (t) Object.assign(t, { title: title.trim(), userEdited: true })
    return null
  },
  home: async () => (sources.length ? { name: 'Bengaluru', lat: 12.9, lon: 77.7, countryCode: 'IN' } : null),
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
