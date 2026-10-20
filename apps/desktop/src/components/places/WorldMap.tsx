import { geoMercator, geoPath, type GeoPermissibleObjects, type GeoProjection } from 'd3-geo'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { geoUrl, type Home, type Id, type PlaceSummary, type Trip } from '../../lib/api'
import { lonLat } from '../../lib/geo'

type Geo = { type: 'FeatureCollection'; features: GeoPermissibleObjects[] }

// Outlines are loaded once per session and shared by every map.
const cache: Partial<Record<'countries' | 'states', Promise<Geo | null>>> = {}
function loadGeo(name: 'countries' | 'states') {
  cache[name] ??= fetch(geoUrl(name))
    .then((r) => (r.ok ? (r.json() as Promise<Geo>) : null))
    .catch(() => null)
  return cache[name]!
}

type Props = {
  places: PlaceSummary[]
  trips: Trip[]
  home: Home | null | undefined
  /** Trip to highlight; its route animates in. */
  selectedTrip?: Id | null
  onPlace?: (p: PlaceSummary) => void
  /** Fit the view to these points instead of everything. */
  focus?: [number, number][]
  className?: string
}

const PAD = 48
/** Extra room at the top for the page title over the map. */
const PAD_TOP = 110
const MIN_SCALE = 120
const MAX_SCALE = 60_000
/** States appear once zoomed in this far. */
const STATES_FROM_SCALE = 1500

function css(name: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

/** A gentle arc between two screen points (control point lifted sideways). */
function arc(ctx: CanvasRenderingContext2D, a: [number, number], b: [number, number], t = 1) {
  const [x1, y1] = a
  const [x2, y2] = b
  const mx = (x1 + x2) / 2
  const my = (y1 + y2) / 2
  const dx = x2 - x1
  const dy = y2 - y1
  const lift = Math.min(0.25 * Math.hypot(dx, dy), 160)
  const cx = mx + (dy / (Math.hypot(dx, dy) || 1)) * lift
  const cy = my - (dx / (Math.hypot(dx, dy) || 1)) * lift
  // Sample the quadratic so we can stop part-way for the animation.
  const steps = 32
  const end = Math.max(0, Math.min(1, t))
  ctx.moveTo(x1, y1)
  let last: [number, number] = [x1, y1]
  for (let i = 1; i <= steps * end; i++) {
    const u = i / steps
    const x = (1 - u) * (1 - u) * x1 + 2 * (1 - u) * u * cx + u * u * x2
    const y = (1 - u) * (1 - u) * y1 + 2 * (1 - u) * u * cy + u * u * y2
    ctx.lineTo(x, y)
    last = [x, y]
  }
  return last
}

export function WorldMap({ places, trips, home, selectedTrip, onPlace, focus, className }: Props) {
  const wrap = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const [size, setSize] = useState({ w: 0, h: 0 })
  // undefined while loading; null when the outlines aren't available.
  const [geo, setGeo] = useState<{ countries: Geo | null; states: Geo | null } | undefined>()
  // The view: projection scale and centre (lon, lat).
  const [view, setView] = useState<{ scale: number; center: [number, number] } | null>(null)
  // Route animation for one trip; any other trip shows its route at 0.
  const [anim, setAnim] = useState<{ trip: Id | null | undefined; t: number }>({ trip: null, t: 1 })
  const progress = anim.trip === selectedTrip ? anim.t : 0
  const [hover, setHover] = useState<Id | null>(null)

  useEffect(() => {
    let alive = true
    Promise.all([loadGeo('countries'), loadGeo('states')]).then(([countries, states]) => {
      if (alive) setGeo({ countries, states })
    })
    return () => {
      alive = false
    }
  }, [])

  useLayoutEffect(() => {
    const el = wrap.current
    if (!el) return
    const ro = new ResizeObserver(([e]) => setSize({ w: Math.floor(e.contentRect.width), h: Math.floor(e.contentRect.height) }))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // Fit to the focus points (or every place plus home) whenever they change.
  const fitPoints = useMemo<[number, number][]>(() => {
    if (focus?.length) return focus
    const pts: [number, number][] = places.map(lonLat)
    if (home) pts.push(lonLat(home))
    return pts
  }, [focus, places, home])
  const fitKey = JSON.stringify(fitPoints) + `${size.w}x${size.h}`
  const lastFit = useRef('')
  useEffect(() => {
    if (!size.w || !size.h || lastFit.current === fitKey) return
    lastFit.current = fitKey
    const p = geoMercator()
    if (fitPoints.length >= 2) {
      const top = size.h > 400 ? PAD_TOP : PAD
      p.fitExtent([[PAD, top], [size.w - PAD, size.h - PAD]], { type: 'MultiPoint', coordinates: fitPoints })
    } else {
      p.scale(fitPoints.length ? 2500 : 160).center(fitPoints[0] ?? [78, 20]).translate([size.w / 2, size.h / 2])
    }
    const scale = Math.min(Math.max(p.scale(), MIN_SCALE), 8000)
    const center = p.invert!([size.w / 2, size.h / 2]) as [number, number]
    setView({ scale, center })
  }, [fitKey, fitPoints, size])

  // Animate the selected trip's route.
  useEffect(() => {
    if (selectedTrip == null) return
    let raf = 0
    const start = performance.now()
    const tick = (now: number) => {
      const t = Math.min(1, (now - start) / 1400)
      setAnim({ trip: selectedTrip, t: 1 - Math.pow(1 - t, 3) })
      if (t < 1) raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [selectedTrip])

  const projection = useMemo<GeoProjection | null>(() => {
    if (!view || !size.w) return null
    return geoMercator().scale(view.scale).center(view.center).translate([size.w / 2, size.h / 2])
  }, [view, size])

  const pins = useMemo(() => {
    if (!projection) return []
    const max = Math.max(1, ...places.map((p) => p.count))
    return places
      .map((p) => {
        const xy = projection(lonLat(p))
        return xy && { p, x: xy[0], y: xy[1], r: 4 + 14 * Math.sqrt(p.count / max) }
      })
      .filter((v): v is NonNullable<typeof v> => !!v)
  }, [places, projection])

  // Draw.
  useEffect(() => {
    const c = canvas.current
    if (!c || !projection) return
    const dpr = window.devicePixelRatio || 1
    c.width = size.w * dpr
    c.height = size.h * dpr
    const ctx = c.getContext('2d')!
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    const colors = {
      sea: css('--bg'),
      land: css('--elevated'),
      border: css('--border'),
      faint: css('--faint'),
      text: css('--text'),
      accent: css('--accent'),
    }
    ctx.fillStyle = colors.sea
    ctx.fillRect(0, 0, size.w, size.h)

    const path = geoPath(projection, ctx)
    if (geo?.countries) {
      ctx.beginPath()
      for (const f of geo.countries.features) path(f)
      ctx.fillStyle = colors.land
      ctx.fill()
      ctx.lineWidth = 0.8
      ctx.strokeStyle = colors.border
      ctx.stroke()
    }
    if (geo?.states && projection.scale() >= STATES_FROM_SCALE) {
      ctx.beginPath()
      for (const f of geo.states.features) path(f)
      ctx.setLineDash([3, 3])
      ctx.lineWidth = 0.6
      ctx.strokeStyle = colors.border
      ctx.stroke()
      ctx.setLineDash([])
    }

    // Trip routes: home -> stops -> home. The selected one animates and glows.
    const homeXY = home ? projection(lonLat(home)) : null
    const drawTrip = (t: Trip, selected: boolean) => {
      if (!homeXY) return
      const pts: [number, number][] = [homeXY as [number, number]]
      for (const s of t.stops) {
        const xy = projection(lonLat(s))
        if (xy) pts.push(xy as [number, number])
      }
      pts.push(homeXY as [number, number])
      const legs = pts.length - 1
      const p = selected ? progress : 1
      ctx.beginPath()
      let head: [number, number] = pts[0]
      for (let i = 0; i < legs; i++) {
        const local = Math.max(0, Math.min(1, p * legs - i))
        if (local <= 0) break
        head = arc(ctx, pts[i], pts[i + 1], local)
      }
      ctx.lineWidth = selected ? 2.5 : 1.2
      ctx.strokeStyle = selected ? colors.accent : colors.faint
      ctx.globalAlpha = selected ? 1 : hover === t.id ? 0.9 : 0.35
      ctx.stroke()
      ctx.globalAlpha = 1
      if (selected && p < 1) {
        ctx.beginPath()
        ctx.arc(head[0], head[1], 5, 0, Math.PI * 2)
        ctx.fillStyle = colors.accent
        ctx.fill()
      }
    }
    for (const t of trips) if (t.id !== selectedTrip) drawTrip(t, false)
    const sel = trips.find((t) => t.id === selectedTrip)
    if (sel) drawTrip(sel, true)

    // Place pins.
    for (const { p, x, y, r } of pins) {
      ctx.beginPath()
      ctx.arc(x, y, r, 0, Math.PI * 2)
      ctx.fillStyle = colors.accent
      ctx.globalAlpha = hover === p.id ? 0.95 : 0.55
      ctx.fill()
      ctx.globalAlpha = 1
      ctx.lineWidth = 1.5
      ctx.strokeStyle = colors.sea
      ctx.stroke()
    }
    // Labels for the bigger places, without overlaps.
    ctx.font = '500 12px Inter Variable, Segoe UI, sans-serif'
    ctx.textBaseline = 'middle'
    const taken: [number, number, number, number][] = []
    for (const { p, x, y, r } of [...pins].sort((a, b) => b.p.count - a.p.count)) {
      const w = ctx.measureText(p.name).width
      const box: [number, number, number, number] = [x + r + 4, y - 8, w, 16]
      if (taken.some((t) => box[0] < t[0] + t[2] && t[0] < box[0] + box[2] && box[1] < t[1] + t[3] && t[1] < box[1] + box[3])) continue
      taken.push(box)
      ctx.fillStyle = colors.text
      ctx.globalAlpha = 0.85
      ctx.fillText(p.name, box[0], y)
      ctx.globalAlpha = 1
    }

    // Home.
    if (homeXY) {
      ctx.beginPath()
      ctx.arc(homeXY[0], homeXY[1], 7, 0, Math.PI * 2)
      ctx.fillStyle = colors.text
      ctx.fill()
      ctx.beginPath()
      ctx.arc(homeXY[0], homeXY[1], 3, 0, Math.PI * 2)
      ctx.fillStyle = colors.sea
      ctx.fill()
    }
  }, [geo, projection, pins, trips, home, selectedTrip, progress, hover, size])

  const pinAt = useCallback(
    (x: number, y: number) => {
      let best: (typeof pins)[number] | null = null
      for (const pin of pins) {
        const d = Math.hypot(pin.x - x, pin.y - y)
        if (d <= Math.max(pin.r, 8) && (!best || d < Math.hypot(best.x - x, best.y - y))) best = pin
      }
      return best
    },
    [pins],
  )

  // Pan and zoom.
  const drag = useRef<{ x: number; y: number; center: [number, number]; moved: boolean } | null>(null)
  const onWheel = (e: React.WheelEvent) => {
    if (!projection || !view) return
    const rect = canvas.current!.getBoundingClientRect()
    const mouse: [number, number] = [e.clientX - rect.left, e.clientY - rect.top]
    const before = projection.invert!(mouse)!
    const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, view.scale * Math.exp(-e.deltaY * 0.0015)))
    // Keep the point under the cursor where it is.
    const p = geoMercator().scale(scale).center(view.center).translate([size.w / 2, size.h / 2])
    const after = p(before)!
    const shifted = p.invert!([size.w / 2 + (after[0] - mouse[0]), size.h / 2 + (after[1] - mouse[1])])!
    setView({ scale, center: shifted as [number, number] })
  }

  return (
    <div ref={wrap} className={className} style={{ position: 'relative', overflow: 'hidden' }}>
      <canvas
        ref={canvas}
        style={{ width: size.w, height: size.h, display: 'block', cursor: hover != null ? 'pointer' : 'grab', touchAction: 'none' }}
        onWheel={onWheel}
        onPointerDown={(e) => {
          if (!view) return
          ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
          drag.current = { x: e.clientX, y: e.clientY, center: view.center, moved: false }
        }}
        onPointerMove={(e) => {
          const rect = canvas.current!.getBoundingClientRect()
          const d = drag.current
          if (d && projection && view) {
            const dx = e.clientX - d.x
            const dy = e.clientY - d.y
            if (Math.abs(dx) + Math.abs(dy) > 3) d.moved = true
            const p = geoMercator().scale(view.scale).center(d.center).translate([size.w / 2, size.h / 2])
            const c = p.invert!([size.w / 2 - dx, size.h / 2 - dy])
            if (c) setView({ scale: view.scale, center: c as [number, number] })
            return
          }
          setHover(pinAt(e.clientX - rect.left, e.clientY - rect.top)?.p.id ?? null)
        }}
        onPointerUp={(e) => {
          const d = drag.current
          drag.current = null
          if (d?.moved) return
          const rect = canvas.current!.getBoundingClientRect()
          const pin = pinAt(e.clientX - rect.left, e.clientY - rect.top)
          if (pin) onPlace?.(pin.p)
        }}
        role="img"
        aria-label="Map of places in your photos"
      />
      {hover != null &&
        (() => {
          const pin = pins.find((p) => p.p.id === hover)
          if (!pin) return null
          return (
            <div
              className="pointer-events-none absolute z-10 rounded-lg border border-border bg-elevated px-2.5 py-1.5 text-[12.5px] shadow-panel"
              style={{ left: pin.x + 12, top: pin.y - 40 }}
            >
              <div className="font-medium text-text">{pin.p.name}</div>
              <div className="text-faint">{pin.p.count.toLocaleString()} photos</div>
            </div>
          )
        })()}
      {geo && !geo.countries && (
        <div className="absolute inset-x-0 bottom-3 text-center text-[12px] text-faint">Map outlines unavailable</div>
      )}
    </div>
  )
}
