import { X } from 'lucide-react'
import { Switch } from 'radix-ui'

import { cn } from '../../lib/cn'
import { usePeople } from '../../lib/queries'
import { usePeopleFilter } from '../../state/peopleFilter'
import { FaceAvatar } from './FaceAvatar'

/** Faces across the top of the timeline: tap people to see photos of them together. */
export function PeopleFilterBar() {
  const { data: people = [] } = usePeople()
  const { people: selected, only, toggle, setOnly, clear } = usePeopleFilter()
  if (!people.length) return null

  // Family first, then whoever appears most; selected people always stay visible.
  const shown = people.filter((p, i) => i < 14 || selected.includes(p.id))
  return (
    <div className="flex items-center gap-3 px-10 pt-3 pb-1">
      <div className="flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto py-1" role="group" aria-label="Filter by people">
        {shown.map((p) => {
          const on = selected.includes(p.id)
          return (
            <button
              key={p.id}
              type="button"
              onClick={() => toggle(p.id)}
              aria-pressed={on}
              title={p.name}
              className={cn(
                'flex h-9 shrink-0 items-center gap-2 rounded-full border py-0.5 pr-3 pl-0.5 text-[13px] transition-colors',
                on ? 'border-accent bg-accent-soft text-text' : 'border-border text-muted hover:border-faint/50 hover:text-text',
              )}
            >
              <FaceAvatar faceId={p.coverFaceId} name={p.name} size={30} />
              <span className="max-w-[110px] truncate">{p.name}</span>
            </button>
          )
        })}
      </div>
      {selected.length > 0 && (
        <div className="mr-14 flex shrink-0 items-center gap-3">
          <label className="flex cursor-pointer items-center gap-2 text-[13px] text-muted">
            <Switch.Root
              checked={only}
              onCheckedChange={setOnly}
              className="relative h-5 w-9 rounded-full bg-hover transition-colors data-[state=checked]:bg-accent"
            >
              <Switch.Thumb className="block size-4 translate-x-0.5 rounded-full bg-white shadow transition-transform data-[state=checked]:translate-x-[18px]" />
            </Switch.Root>
            No one else
          </label>
          <button
            type="button"
            onClick={clear}
            className="flex items-center gap-1 rounded-lg px-2 py-1 text-[13px] text-muted hover:bg-hover hover:text-text"
          >
            <X size={14} /> Clear
          </button>
        </div>
      )}
    </div>
  )
}
