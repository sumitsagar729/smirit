import { FolderPlus, Minus, Plus } from 'lucide-react'
import { useState } from 'react'

import { PhotoStack } from '../components/art'
import { PeopleFilterBar } from '../components/people/PeopleFilterBar'
import { TimelineGrid } from '../components/TimelineGrid'
import { Button } from '../components/ui/Button'
import { EmptyState } from '../components/ui/EmptyState'
import { useAddFolder, useIndexStatus, usePeople, useStats, useTimeline } from '../lib/queries'
import { usePeopleFilter } from '../state/peopleFilter'

const SIZES = [140, 180, 220, 280, 360]

export function TimelineView() {
  const { data: stats, isLoading: statsLoading } = useStats()
  const filterPeople = usePeopleFilter((s) => s.people)
  const only = usePeopleFilter((s) => s.only)
  const filtered = filterPeople.length > 0
  const { data: tiles = [], isLoading } = useTimeline(filtered ? { people: filterPeople, only } : null)
  const { data: people = [] } = usePeople()
  const index = useIndexStatus()
  const addFolder = useAddFolder()
  const [size, setSize] = useState(2)

  if (statsLoading || isLoading) return null

  if (!stats?.sources) {
    return (
      <div className="grid h-full place-items-center pb-16">
        <EmptyState
          art={<PhotoStack />}
          eyebrow="Welcome to Smriti"
          title="Your family's story starts here"
          body="Add the folders where you keep photos and videos. Smriti organises them by who, when and where, right on this computer. Nothing is ever uploaded."
          actions={
            <Button
              variant="primary"
              size="lg"
              icon={<FolderPlus size={18} />}
              onClick={() => addFolder.mutate()}
              disabled={addFolder.isPending}
            >
              Add a photo folder
            </Button>
          }
        />
      </div>
    )
  }

  if (!tiles.length && !filtered) {
    const busy = index.phase !== 'idle'
    return (
      <div className="grid h-full place-items-center pb-16">
        <EmptyState
          art={<PhotoStack />}
          eyebrow={busy ? 'Looking through your folders' : 'Nothing found yet'}
          title={busy ? 'Finding your memories…' : 'No photos or videos in these folders'}
          body={
            busy
              ? 'The first photos appear here in a few seconds. You can keep using Smriti while it works.'
              : 'Smriti looks for JPG, PNG, HEIC, RAW, MP4, MOV and more. Try adding another folder.'
          }
        />
      </div>
    )
  }

  const total = (stats.photos + stats.videos).toLocaleString()
  const names = filterPeople.map((id) => people.find((p) => p.id === id)?.name ?? 'someone')
  const who =
    names.length <= 2 ? names.join(' and ') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`
  return (
    <div className="flex h-full flex-col">
      <header className="flex shrink-0 items-end justify-between px-10 pt-8 pb-2">
        <div>
          <h1 className="font-display text-[34px] leading-none font-medium tracking-tight text-text">Timeline</h1>
          <p className="mt-2 text-[13.5px] text-muted">
            {filtered ? (
              <>
                {tiles.length.toLocaleString()} {tiles.length === 1 ? 'photo' : 'photos'} of {who}
                {names.length > 1 && ' together'}
                {only && ', no one else'}
              </>
            ) : (
              <>{total} photos and videos</>
            )}
            {index.phase === 'indexing' && ` · indexing ${index.done.toLocaleString()} of ${index.total.toLocaleString()}`}
          </p>
        </div>
        <div className="mr-14 flex items-center gap-1 rounded-xl border border-border bg-surface p-1" aria-label="Thumbnail size">
          <Button size="sm" variant="ghost" aria-label="Smaller" disabled={size === 0} onClick={() => setSize(size - 1)} icon={<Minus size={15} />} />
          <Button size="sm" variant="ghost" aria-label="Larger" disabled={size === SIZES.length - 1} onClick={() => setSize(size + 1)} icon={<Plus size={15} />} />
        </div>
      </header>
      <PeopleFilterBar />
      <div className="min-h-0 flex-1">
        {tiles.length ? (
          <TimelineGrid tiles={tiles} rowHeight={SIZES[size]} />
        ) : (
          <div className="grid h-full place-items-center pb-24 text-center">
            <div>
              <div className="text-[15px] text-text">No photos of {who}{only ? ' on their own' : ' together'} yet</div>
              <div className="mt-1 text-[13.5px] text-muted">
                {only ? 'Try turning off “No one else”.' : 'Try fewer people, or name more faces in People.'}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
