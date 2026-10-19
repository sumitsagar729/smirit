// In-memory stand-in for the Rust core so every screen can be developed and
// reviewed in a plain browser. Never used inside the desktop app.
import type { Api } from './api'
import type { ApiError, Source, Tile } from './bindings'

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
  indexStatus: async () => ({ phase: 'idle', total: 0, done: 0, failed: 0, generation: 0 }),
  timeline: async () => (sources.length ? tiles : []),
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
