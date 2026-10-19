import { describe, expect, it } from 'vitest'

import type { Tile } from './bindings'
import { GAP, justify, yearIndex } from './layout'

const tile = (id: number, takenAt: string, aspect = 1.5): Tile => ({
  id,
  aspect,
  takenAt,
  kind: 'photo',
  duration: null,
  hasThumb: true,
})

describe('justify', () => {
  it('groups by month and fills full rows exactly', () => {
    const tiles = [
      ...Array.from({ length: 10 }, (_, i) => tile(i, '2021-10-12T10:00:00')),
      ...Array.from({ length: 3 }, (_, i) => tile(100 + i, '2021-08-01T10:00:00')),
    ]
    const rows = justify(tiles, 1000, 200)
    expect(rows.filter((r) => r.kind === 'month').map((r) => r.kind === 'month' && r.label)).toEqual([
      'October 2021',
      'August 2021',
    ])
    const full = rows.find((r) => r.kind === 'photos')!
    if (full.kind !== 'photos') throw new Error('expected photos row')
    const used = full.items.reduce((s, it) => s + it.width, 0) + GAP * (full.items.length - 1)
    expect(Math.abs(used - 1000)).toBeLessThan(1)
  })

  it('does not stretch a short last row', () => {
    const rows = justify([tile(1, '2020-01-01T00:00:00')], 1200, 220)
    const last = rows.at(-1)!
    expect(last.height).toBe(220)
  })

  it('puts undated items last under "Date unknown" and indexes years', () => {
    const tiles = [tile(1, '2022-05-01T00:00:00'), tile(2, '2021-05-01T00:00:00'), { ...tile(3, ''), takenAt: null }]
    const rows = justify(tiles, 800, 200)
    expect(rows.at(-2)).toMatchObject({ kind: 'month', label: 'Date unknown' })
    expect(yearIndex(rows).map((y) => y.year)).toEqual([2022, 2021])
  })
})
