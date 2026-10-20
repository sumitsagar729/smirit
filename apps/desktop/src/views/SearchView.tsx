import { CalendarDays, Clock, Film, Folder, Image, MapPin, Plane, Search, Sparkles, UserCheck, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'

import { FaceAvatar } from '../components/people/FaceAvatar'
import { TimelineGrid } from '../components/TimelineGrid'
import type { Chip, Suggestion } from '../lib/api'
import { cn } from '../lib/cn'
import { localToday, useSuggest, useTimeline } from '../lib/queries'
import { chipLabel, chipsToQuery, describe, sameChip } from '../lib/search'
import { useSearch } from '../state/search'

function ChipIcon({ chip, size = 14 }: { chip: Chip; size?: number }) {
  switch (chip.type) {
    case 'person':
      return <FaceAvatar faceId={chip.faceId} name={chip.name} size={size + 8} />
    case 'place':
    case 'region':
    case 'country':
      return <MapPin size={size} />
    case 'trip':
      return <Plane size={size} />
    case 'dates':
      return <CalendarDays size={size} />
    case 'age':
      return <Sparkles size={size} />
    case 'onThisDay':
      return <Clock size={size} />
    case 'folder':
      return <Folder size={size} />
    case 'kind':
      return chip.kind === 'video' ? <Film size={size} /> : <Image size={size} />
    case 'only':
      return <UserCheck size={size} />
  }
}

function SearchBox() {
  const { chips, add, remove, clear } = useSearch()
  const [text, setText] = useState('')
  const [active, setActive] = useState(0)
  const [open, setOpen] = useState(true)
  const input = useRef<HTMLInputElement>(null)
  const { data: raw = [] } = useSuggest(text)
  const suggestions = raw.filter((s) => !chips.some((c) => sameChip(c, s.chip)))

  useEffect(() => input.current?.focus(), [])

  const pick = (s: Suggestion | undefined) => {
    if (!s) return
    add(s.chip)
    setText('')
    setActive(0)
    setOpen(false)
    input.current?.focus()
  }

  return (
    <div className="relative">
      <div
        className="flex min-h-12 flex-wrap items-center gap-1.5 rounded-2xl border border-border bg-surface px-3 py-2 shadow-panel focus-within:border-accent/50"
        onClick={() => input.current?.focus()}
      >
        <Search size={18} className="mr-1 text-faint" />
        {chips.map((c, i) => (
          <span
            key={i}
            className="flex h-8 items-center gap-1.5 rounded-full border border-accent/40 bg-accent-soft py-0.5 pr-1.5 pl-1.5 text-[13px] text-text"
          >
            <span className="text-accent">
              <ChipIcon chip={c} size={13} />
            </span>
            {chipLabel(c)}
            <button
              type="button"
              aria-label={`Remove ${chipLabel(c)}`}
              onClick={() => remove(i)}
              className="rounded-full p-0.5 text-muted hover:bg-hover hover:text-text"
            >
              <X size={12} />
            </button>
          </span>
        ))}
        <input
          ref={input}
          value={text}
          onChange={(e) => {
            setText(e.target.value)
            setActive(0)
            setOpen(true)
          }}
          onFocus={() => setOpen(true)}
          onClick={() => setOpen(true)}
          onBlur={() => setTimeout(() => setOpen(false), 120)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault()
              setActive((a) => Math.min(a + 1, suggestions.length - 1))
            } else if (e.key === 'ArrowUp') {
              e.preventDefault()
              setActive((a) => Math.max(a - 1, 0))
            } else if (e.key === 'Enter') {
              e.preventDefault()
              pick(suggestions[active])
            } else if (e.key === 'Backspace' && !text && chips.length) {
              remove(chips.length - 1)
            } else if (e.key === 'Escape') {
              setOpen(false)
            }
          }}
          placeholder={chips.length ? 'Add more…' : 'Search people, places, trips, dates… try “Divik at 3” or “Ooty”'}
          aria-label="Search"
          role="combobox"
          aria-expanded={open && suggestions.length > 0}
          className="h-8 min-w-[220px] flex-1 bg-transparent text-[15px] text-text placeholder:text-faint focus:outline-none"
        />
        {chips.length > 0 && (
          <button type="button" onClick={clear} className="rounded-lg px-2 py-1 text-[13px] text-muted hover:bg-hover hover:text-text">
            Clear
          </button>
        )}
      </div>
      {open && suggestions.length > 0 && (
        <ul
          role="listbox"
          className="absolute top-full right-0 left-0 z-30 mt-2 overflow-hidden rounded-2xl border border-border bg-elevated p-1.5 shadow-panel"
        >
          {suggestions.map((s, i) => (
            <li
              key={i}
              role="option"
              aria-selected={i === active}
              onMouseDown={(e) => {
                e.preventDefault()
                pick(s)
              }}
              onMouseEnter={() => setActive(i)}
              className={cn(
                'flex h-11 cursor-pointer items-center gap-3 rounded-xl px-3 text-[14px]',
                i === active ? 'bg-hover text-text' : 'text-muted',
              )}
            >
              <span className="grid w-6 place-items-center text-accent">
                <ChipIcon chip={s.chip} size={16} />
              </span>
              <span className="flex-1 truncate text-text">{s.label}</span>
              {s.detail && <span className="truncate text-[12.5px] text-faint">{s.detail}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

export function SearchView() {
  const chips = useSearch((s) => s.chips)
  const query = useMemo(() => chipsToQuery(chips, localToday()), [chips])
  const { data: tiles = [], isFetching } = useTimeline(chips.length ? query : null)

  return (
    <div className="flex h-full flex-col">
      <header className="shrink-0 px-10 pt-8 pb-4">
        <h1 className="mb-4 font-display text-[34px] leading-none font-medium tracking-tight text-text">Search</h1>
        <div className="mr-14 max-w-[860px]">
          <SearchBox />
        </div>
        {chips.length > 0 && (
          <p className={cn('mt-3 text-[13.5px] text-muted transition-opacity', isFetching && 'opacity-60')} aria-live="polite">
            {tiles.length.toLocaleString()} {tiles.length === 1 ? 'result' : 'results'} · {describe(chips)}
          </p>
        )}
      </header>
      <div className="min-h-0 flex-1">
        {chips.length === 0 ? (
          <div className="px-10 pt-6 text-[13.5px] text-faint">
            Pick a suggestion or type a name, a place, a year, “videos”, or a child’s age like “Aarna under 1”.
          </div>
        ) : tiles.length ? (
          <TimelineGrid tiles={tiles} rowHeight={200} />
        ) : (
          <div className="grid h-full place-items-center pb-24 text-center">
            <div>
              <div className="text-[15px] text-text">Nothing matches all of these</div>
              <div className="mt-1 text-[13.5px] text-muted">Try removing a chip.</div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
