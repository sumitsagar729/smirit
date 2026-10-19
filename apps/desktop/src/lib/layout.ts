import type { Tile } from './bindings'

export type MonthRow = { kind: 'month'; key: string; label: string; year: number; count: number; height: number }
export type PhotoRow = {
  kind: 'photos'
  key: string
  year: number
  height: number
  items: { tile: Tile; index: number; width: number }[]
}
export type Row = MonthRow | PhotoRow

export const MONTH_HEADER_HEIGHT = 64
export const GAP = 4

/** Rust f32 arrives as `number | null`; treat unknown as square. */
const aspectOf = (t: Tile) => t.aspect ?? 1

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

/**
 * Justified layout: rows of equal height that fill the width exactly, keeping
 * each photo's aspect ratio. Grouped by month with a header row per month.
 * `tiles` must be sorted newest first; `index` points back into `tiles`.
 */
export function justify(tiles: Tile[], width: number, targetHeight: number): Row[] {
  const rows: Row[] = []
  if (width <= 0) return rows

  let i = 0
  while (i < tiles.length) {
    const month = tiles[i].takenAt?.slice(0, 7) ?? 'unknown'
    let j = i
    while (j < tiles.length && (tiles[j].takenAt?.slice(0, 7) ?? 'unknown') === month) j++

    const year = month === 'unknown' ? 0 : Number(month.slice(0, 4))
    rows.push({
      kind: 'month',
      key: `m-${month}`,
      label: month === 'unknown' ? 'Date unknown' : `${MONTHS[Number(month.slice(5, 7)) - 1]} ${year}`,
      year,
      count: j - i,
      height: MONTH_HEADER_HEIGHT,
    })

    let row: { tile: Tile; index: number }[] = []
    let aspectSum = 0
    const flush = (last: boolean) => {
      if (!row.length) return
      const gaps = GAP * (row.length - 1)
      const fitted = (width - gaps) / aspectSum
      // A short last row keeps the target height instead of being stretched.
      const h = last && fitted > targetHeight ? targetHeight : fitted
      rows.push({
        kind: 'photos',
        key: `p-${row[0].tile.id}`,
        year,
        height: Math.round(h),
        items: row.map((r) => ({ ...r, width: aspectOf(r.tile) * h })),
      })
      row = []
      aspectSum = 0
    }
    for (let k = i; k < j; k++) {
      row.push({ tile: tiles[k], index: k })
      aspectSum += aspectOf(tiles[k])
      if (aspectSum * targetHeight + GAP * (row.length - 1) >= width) flush(false)
    }
    flush(true)
    i = j
  }
  return rows
}

/** First row index of each year, for the scrubber. */
export function yearIndex(rows: Row[]): { year: number; row: number }[] {
  const out: { year: number; row: number }[] = []
  rows.forEach((r, idx) => {
    if (r.kind === 'month' && r.year && out.at(-1)?.year !== r.year) out.push({ year: r.year, row: idx })
  })
  return out
}
