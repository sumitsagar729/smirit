import { Home as HomeIcon, MapPin, MapPinned, Plane } from 'lucide-react'
import { useState } from 'react'

import { IconHalo } from '../components/art'
import { WorldMap } from '../components/places/WorldMap'
import { lonLat } from '../lib/geo'
import { EmptyState } from '../components/ui/EmptyState'
import { thumbUrl, type PlaceSummary, type Trip } from '../lib/api'
import { cn } from '../lib/cn'
import { formatTripDates } from '../lib/format'
import { useHome, useIndexStatus, usePlaces, useTrips } from '../lib/queries'
import { useNav } from '../state/nav'
import { useSearch } from '../state/search'

function TripRow({ t, selected, onSelect }: { t: Trip; selected: boolean; onSelect: () => void }) {
  const openTrip = useNav((s) => s.openTrip)
  return (
    <div
      className={cn(
        'group flex cursor-pointer gap-3 rounded-xl p-2 transition-colors',
        selected ? 'bg-elevated shadow-panel' : 'hover:bg-hover',
      )}
      onClick={onSelect}
      onDoubleClick={() => openTrip(t.id)}
    >
      <div className="size-16 shrink-0 overflow-hidden rounded-lg bg-hover">
        {t.coverAssetId != null && <img src={thumbUrl(t.coverAssetId)} alt="" className="size-full object-cover" loading="lazy" />}
      </div>
      <div className="min-w-0 flex-1 py-0.5">
        <div className="truncate font-display text-[16px] font-medium text-text">{t.title}</div>
        <div className="truncate text-[12.5px] text-muted">{formatTripDates(t.start, t.end)}</div>
        <div className="truncate text-[12px] text-faint">
          {t.count.toLocaleString()} photos
          {t.distanceKm != null && ` · ${Math.round(t.distanceKm).toLocaleString()} km from home`}
        </div>
      </div>
      {selected && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            openTrip(t.id)
          }}
          className="self-center rounded-lg bg-accent px-2.5 py-1.5 text-[12.5px] font-medium text-accent-fg"
        >
          Open
        </button>
      )}
    </div>
  )
}

function PlaceRow({ p, onOpen }: { p: PlaceSummary; onOpen: () => void }) {
  return (
    <button type="button" onClick={onOpen} className="flex w-full items-center gap-3 rounded-xl p-2 text-left hover:bg-hover">
      <div className="size-11 shrink-0 overflow-hidden rounded-lg bg-hover">
        {p.coverAssetId != null && <img src={thumbUrl(p.coverAssetId)} alt="" className="size-full object-cover" loading="lazy" />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-medium text-text">{p.name}</div>
        <div className="truncate text-[12px] text-faint">{[p.admin1, p.country].filter(Boolean).join(', ')}</div>
      </div>
      <span className="text-[12.5px] text-muted tabular-nums">{p.count.toLocaleString()}</span>
    </button>
  )
}

export function PlacesView() {
  const { data: places = [], isLoading } = usePlaces()
  const { data: trips = [] } = useTrips()
  const { data: home } = useHome()
  const index = useIndexStatus()
  const go = useNav((s) => s.go)
  const setChips = useSearch((s) => s.set)
  const [tab, setTab] = useState<'trips' | 'places'>('trips')
  const [selected, setSelected] = useState<Trip | null>(null)

  if (isLoading) return null
  if (!places.length) {
    return (
      <div className="grid h-full place-items-center pb-16">
        <EmptyState
          art={
            <IconHalo>
              <MapPinned size={36} strokeWidth={1.6} />
            </IconHalo>
          }
          eyebrow={index.phase !== 'idle' ? 'Working on it' : 'No places yet'}
          title="Every trip on one map"
          body="Places come from photo locations, and from town names in folder names like “ooty - nov 2021”. They appear after your photos are indexed."
        />
      </div>
    )
  }

  const openPlace = (p: PlaceSummary) => {
    setChips([{ type: 'place', id: p.id, name: p.name }])
    go('search')
  }
  const focus: [number, number][] | undefined =
    selected && home ? [lonLat(home), ...selected.stops.map(lonLat)] : undefined

  return (
    <div className="flex h-full">
      <div className="relative min-w-0 flex-1">
        <WorldMap
          className="size-full"
          places={places}
          trips={trips}
          home={home}
          selectedTrip={selected?.id}
          focus={focus}
          onPlace={openPlace}
        />
        <div className="pointer-events-none absolute top-6 left-8">
          <h1 className="font-display text-[34px] leading-none font-medium tracking-tight text-text drop-shadow">Places</h1>
          <p className="mt-2 flex items-center gap-1.5 text-[13.5px] text-muted">
            {home && (
              <>
                <HomeIcon size={13} /> Home: {home.name} ·{' '}
              </>
            )}
            {trips.length} trips · {places.length} places
          </p>
        </div>
      </div>
      <aside className="flex w-[360px] shrink-0 flex-col border-l border-border bg-surface/60">
        <div role="tablist" className="flex gap-1 px-4 pt-4">
          {(
            [
              ['trips', 'Trips', Plane],
              ['places', 'Places', MapPin],
            ] as const
          ).map(([key, label, Icon]) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={tab === key}
              onClick={() => setTab(key)}
              className={cn(
                'flex h-9 flex-1 items-center justify-center gap-2 rounded-lg text-[13.5px] font-medium transition-colors',
                tab === key ? 'bg-elevated text-text shadow-panel' : 'text-muted hover:text-text',
              )}
            >
              <Icon size={15} /> {label}
            </button>
          ))}
        </div>
        <div className="min-h-0 flex-1 space-y-1 overflow-y-auto p-3">
          {tab === 'trips' ? (
            trips.length ? (
              trips.map((t) => (
                <TripRow key={t.id} t={t} selected={selected?.id === t.id} onSelect={() => setSelected(selected?.id === t.id ? null : t)} />
              ))
            ) : (
              <p className="p-3 text-[13px] text-faint">No trips found yet. Trips are days with photos more than 70 km from home.</p>
            )
          ) : (
            places.map((p) => <PlaceRow key={p.id} p={p} onOpen={() => openPlace(p)} />)
          )}
        </div>
      </aside>
    </div>
  )
}
