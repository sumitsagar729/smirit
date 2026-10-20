import { ArrowLeft, Home as HomeIcon } from 'lucide-react'
import { useEffect, useState } from 'react'

import { WorldMap } from '../components/places/WorldMap'
import { lonLat } from '../lib/geo'
import { TimelineGrid } from '../components/TimelineGrid'
import type { Trip } from '../lib/api'
import { formatTripDates } from '../lib/format'
import { useHome, useRenameTrip, useTimeline, useTrip } from '../lib/queries'
import { useNav } from '../state/nav'

function Title({ t }: { t: Trip }) {
  const rename = useRenameTrip()
  const [title, setTitle] = useState(t.title)
  const save = () => {
    if (title.trim() && title !== t.title) rename.mutate({ id: t.id, title }, { onError: () => setTitle(t.title) })
    else setTitle(t.title)
  }
  return (
    <input
      value={title}
      aria-label="Trip name"
      onChange={(e) => setTitle(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
      className="-ml-1.5 w-full max-w-[520px] rounded-lg bg-transparent px-1.5 font-display text-[34px] leading-tight font-medium tracking-tight text-text hover:bg-hover focus:bg-hover focus:outline-none"
    />
  )
}

export function TripView() {
  const id = useNav((s) => s.tripId)!
  const go = useNav((s) => s.go)
  const { data: t, isError } = useTrip(id)
  const { data: home } = useHome()
  const { data: tiles = [] } = useTimeline({ trip: id })

  useEffect(() => {
    if (isError) go('places')
  }, [isError, go])
  if (!t) return null

  const route = [home?.name ?? 'Home', ...t.stops.map((s) => s.name), home?.name ?? 'Home']
  const focus: [number, number][] = [...(home ? [lonLat(home)] : []), ...t.stops.map(lonLat)]

  return (
    <div className="flex h-full flex-col">
      <header className="flex shrink-0 gap-6 px-10 pt-6 pb-4">
        <div className="min-w-0 flex-1">
          <button
            type="button"
            onClick={() => go('places')}
            className="-ml-2 mb-3 flex items-center gap-1.5 rounded-lg px-2 py-1 text-[13px] text-muted hover:bg-hover hover:text-text"
          >
            <ArrowLeft size={15} /> Places
          </button>
          <Title key={t.title} t={t} />
          <p className="mt-1 text-[13.5px] text-muted">
            {formatTripDates(t.start, t.end)} · {tiles.length.toLocaleString()} photos and videos
            {t.distanceKm != null && ` · ${Math.round(t.distanceKm).toLocaleString()} km from home`}
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-1.5 text-[13px] text-muted">
            {route.map((name, i) => (
              <span key={i} className="flex items-center gap-1.5">
                {i > 0 && <span className="text-faint">→</span>}
                <span className={i === 0 || i === route.length - 1 ? 'flex items-center gap-1 text-faint' : 'text-text'}>
                  {(i === 0 || i === route.length - 1) && <HomeIcon size={12} />}
                  {name}
                </span>
              </span>
            ))}
          </div>
        </div>
        <WorldMap
          className="h-[180px] w-[320px] shrink-0 overflow-hidden rounded-2xl border border-border"
          places={[]}
          trips={[t]}
          home={home}
          selectedTrip={t.id}
          focus={focus}
        />
      </header>
      <div className="min-h-0 flex-1">
        <TimelineGrid tiles={tiles} rowHeight={200} />
      </div>
    </div>
  )
}
