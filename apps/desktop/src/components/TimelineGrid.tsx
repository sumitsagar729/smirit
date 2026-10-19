import { useVirtualizer } from '@tanstack/react-virtual'
import { ImageOff, Play } from 'lucide-react'
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { thumbUrl } from '../lib/api'
import type { Tile } from '../lib/bindings'
import { cn } from '../lib/cn'
import { formatDuration } from '../lib/format'
import { GAP, justify, type PhotoRow, yearIndex } from '../lib/layout'
import { useViewer } from '../state/viewer'

const SIDE_PADDING = 40
const SCRUBBER_WIDTH = 56

// Warm placeholder tones so loading rows feel like photos, not grey boxes.
const PLACEHOLDERS = ['#2a2420', '#22262b', '#272228', '#25281f', '#2b2522']

const Thumb = memo(function Thumb({ tile, width, height, onOpen }: { tile: Tile; width: number; height: number; onOpen: () => void }) {
  const [loaded, setLoaded] = useState(false)
  const [failed, setFailed] = useState(false)
  return (
    <button
      type="button"
      onClick={onOpen}
      style={{ width, height, background: PLACEHOLDERS[tile.id % PLACEHOLDERS.length] }}
      className="group relative shrink-0 overflow-hidden rounded-[6px] outline-offset-2 focus-visible:outline-2 focus-visible:outline-accent"
      aria-label={tile.kind === 'video' ? 'Open video' : 'Open photo'}
    >
      {tile.hasThumb && !failed ? (
        <img
          src={thumbUrl(tile.id)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setLoaded(true)}
          onError={() => setFailed(true)}
          className={cn(
            'size-full object-cover transition-[opacity,transform] duration-300 ease-spring group-hover:scale-[1.03]',
            loaded ? 'opacity-100' : 'opacity-0',
          )}
        />
      ) : failed ? (
        <ImageOff size={18} className="absolute inset-0 m-auto text-faint" />
      ) : null}
      <span className="pointer-events-none absolute inset-0 rounded-[6px] ring-1 ring-white/0 ring-inset transition group-hover:bg-black/10 group-hover:ring-white/15" />
      {tile.kind === 'video' && (
        <span className="absolute right-1.5 bottom-1.5 flex items-center gap-1 rounded-md bg-black/55 px-1.5 py-0.5 text-[11px] font-medium text-white backdrop-blur-sm">
          <Play size={10} fill="currentColor" />
          {tile.duration != null ? formatDuration(tile.duration) : 'Video'}
        </span>
      )}
    </button>
  )
})

function PhotoRowView({ row, tiles }: { row: PhotoRow; tiles: Tile[] }) {
  const open = useViewer((s) => s.open)
  return (
    <div className="flex" style={{ gap: GAP, height: row.height }}>
      {row.items.map((it) => (
        <Thumb key={it.tile.id} tile={it.tile} width={it.width} height={row.height} onOpen={() => open(tiles, it.index)} />
      ))}
    </div>
  )
}

export function TimelineGrid({ tiles, rowHeight = 220 }: { tiles: Tile[]; rowHeight?: number }) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [width, setWidth] = useState(0)

  useLayoutEffect(() => {
    const el = scrollRef.current
    if (!el) return
    const ro = new ResizeObserver(([e]) => setWidth(Math.floor(e.contentRect.width)))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // contentRect already excludes the container's padding.
  const contentWidth = Math.max(0, width)
  const rows = useMemo(() => justify(tiles, contentWidth, rowHeight), [tiles, contentWidth, rowHeight])
  const years = useMemo(() => yearIndex(rows), [rows])

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => rows[i].height + (rows[i].kind === 'photos' ? GAP : 0),
    overscan: 6,
    getItemKey: (i) => rows[i].key,
  })
  // Row heights change with the window width; re-measure.
  useEffect(() => virtualizer.measure(), [rows, virtualizer])

  const items = virtualizer.getVirtualItems()
  const currentYear = items.length ? rows[items[0].index]?.year : undefined

  return (
    <div className="relative h-full">
      <div ref={scrollRef} className="h-full overflow-y-auto" style={{ paddingLeft: SIDE_PADDING, paddingRight: SIDE_PADDING + SCRUBBER_WIDTH }}>
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {items.map((v) => {
            const row = rows[v.index]
            return (
              <div
                key={v.key}
                style={{ position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${v.start}px)` }}
              >
                {row.kind === 'month' ? (
                  <div className="flex h-16 items-end gap-3 pb-3">
                    <h2 className="font-display text-[22px] leading-none font-medium tracking-tight text-text">{row.label}</h2>
                    <span className="pb-px text-[12.5px] text-faint">{row.count.toLocaleString()}</span>
                  </div>
                ) : (
                  <PhotoRowView row={row} tiles={tiles} />
                )}
              </div>
            )
          })}
        </div>
        <div className="h-10" />
      </div>

      {years.length > 1 && (
        <nav
          aria-label="Jump to year"
          className="absolute top-6 right-3 bottom-6 flex w-12 flex-col items-center justify-start gap-0.5 overflow-y-auto"
        >
          {years.map((y) => (
            <button
              key={y.year}
              type="button"
              onClick={() => virtualizer.scrollToIndex(y.row, { align: 'start' })}
              className={cn(
                'w-full rounded-md py-1 text-[11.5px] font-medium tabular-nums transition-colors',
                y.year === currentYear ? 'bg-accent-soft text-accent' : 'text-faint hover:bg-hover hover:text-text',
              )}
            >
              {y.year}
            </button>
          ))}
        </nav>
      )}
    </div>
  )
}
