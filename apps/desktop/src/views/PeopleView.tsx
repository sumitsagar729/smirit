import { EyeOff, Heart, Images, Maximize2, Sparkles, Users } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'

import { IconHalo } from '../components/art'
import { PageHeader } from '../components/PageHeader'
import { FaceAvatar } from '../components/people/FaceAvatar'
import { GroupDialog } from '../components/people/GroupDialog'
import { PersonPicker } from '../components/people/PersonPicker'
import { Button } from '../components/ui/Button'
import { EmptyState } from '../components/ui/EmptyState'
import type { FaceGroup, Id, Person } from '../lib/api'
import { cn } from '../lib/cn'
import { useFaceGroups, useIgnoreGroup, useIndexStatus, useNameGroup, usePeople } from '../lib/queries'
import { useNav } from '../state/nav'
import { usePeopleFilter } from '../state/peopleFilter'
import { toast } from '../state/toast'

function PersonCard({ p, selected, onSelect }: { p: Person; selected: boolean; onSelect: () => void }) {
  const openPerson = useNav((s) => s.openPerson)
  return (
    <motion.div layout initial={{ opacity: 0, scale: 0.96 }} animate={{ opacity: 1, scale: 1 }} className="group relative">
      <button
        type="button"
        onClick={() => openPerson(p.id)}
        className="flex w-full flex-col items-center rounded-2xl px-2 pt-4 pb-3 text-center transition-colors hover:bg-hover"
      >
        <span className={cn('rounded-full p-[3px] transition', selected ? 'bg-accent' : 'bg-transparent')}>
          <FaceAvatar faceId={p.coverFaceId} name={p.name} size={104} />
        </span>
        <span className="mt-3 flex max-w-full items-center gap-1.5 text-[14.5px] font-medium text-text">
          <span className="truncate">{p.name}</span>
          {p.isFamily && <Heart size={12} className="shrink-0 fill-accent text-accent" aria-label="Family" />}
        </span>
        <span className="text-[12.5px] text-faint tabular-nums">{p.photoCount.toLocaleString()} photos</span>
      </button>
      <button
        type="button"
        onClick={onSelect}
        aria-pressed={selected}
        aria-label={selected ? `Remove ${p.name} from selection` : `Select ${p.name}`}
        className={cn(
          'absolute top-3 right-4 grid size-6 place-items-center rounded-full border text-[11px] font-bold transition',
          selected
            ? 'border-accent bg-accent text-accent-fg'
            : 'border-white/30 bg-black/40 text-transparent opacity-0 group-hover:opacity-100 hover:border-white/60',
        )}
      >
        ✓
      </button>
    </motion.div>
  )
}

function GroupCard({ g, onOpen }: { g: FaceGroup; onOpen: () => void }) {
  const name = useNameGroup()
  const ignore = useIgnoreGroup()
  const busy = name.isPending || ignore.isPending
  const [hero, ...rest] = g.faceIds
  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.94 }}
      transition={{ duration: 0.22 }}
      className="flex flex-col rounded-2xl border border-border bg-surface p-3"
    >
      <button type="button" onClick={onOpen} className="group relative flex gap-1.5" aria-label={`See all ${g.size} faces`}>
        <FaceAvatar faceId={hero} size={112} square />
        <div className="grid flex-1 grid-cols-2 gap-1.5">
          {rest.slice(0, 4).map((f) => (
            <FaceAvatar key={f} faceId={f} size={53} square className="!w-full" />
          ))}
        </div>
        <span className="absolute right-1.5 bottom-1.5 flex items-center gap-1 rounded-md bg-black/60 px-1.5 py-0.5 text-[11px] font-medium text-white opacity-0 transition group-hover:opacity-100">
          <Maximize2 size={11} /> See all
        </span>
      </button>
      <div className="mt-2.5 mb-2 flex items-center justify-between text-[12.5px] text-muted">
        <span className="tabular-nums">{g.size.toLocaleString()} faces</span>
        <button
          type="button"
          disabled={busy}
          onClick={() => ignore.mutate(g.id)}
          className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-faint hover:bg-hover hover:text-text"
          title="Strangers, statues, posters…"
        >
          <EyeOff size={12} /> Hide
        </button>
      </div>
      <PersonPicker
        disabled={busy}
        onPick={(to, label) =>
          name.mutate(
            { group: g.id, to },
            { onSuccess: () => toast.success(`Named ${label}`, 'Smriti is now looking for them in your other photos.') },
          )
        }
      />
    </motion.div>
  )
}

export function PeopleView() {
  const { data: people = [], isLoading } = usePeople()
  const { data: groups = [], isLoading: groupsLoading } = useFaceGroups()
  const index = useIndexStatus()
  const go = useNav((s) => s.go)
  const setFilter = usePeopleFilter((s) => s.set)
  const [selected, setSelected] = useState<Id[]>([])
  const [openGroup, setOpenGroup] = useState<Id | null>(null)

  if (isLoading || groupsLoading) return null

  const busy = index.phase !== 'idle'
  if (!people.length && !groups.length) {
    return (
      <div className="grid h-full place-items-center pb-16">
        <EmptyState
          art={
            <IconHalo>
              <Users size={36} strokeWidth={1.6} />
            </IconHalo>
          }
          eyebrow={busy ? 'Looking for faces' : 'No faces yet'}
          title={busy ? 'Finding everyone in your photos…' : 'Faces will appear here'}
          body={
            busy
              ? 'Smriti groups similar faces as it goes. The first groups appear after a few thousand photos; you can keep using the app meanwhile.'
              : 'Add a folder with photos of people, and Smriti will group their faces so you can name them.'
          }
        />
      </div>
    )
  }

  const toggle = (id: Id) => setSelected((s) => (s.includes(id) ? s.filter((x) => x !== id) : [...s, id]))
  const showTogether = (only: boolean) => {
    setFilter(selected, only)
    go('timeline')
  }

  return (
    <div className="h-full overflow-y-auto pb-24">
      <PageHeader
        title="People"
        subtitle={
          <>
            {people.length.toLocaleString()} {people.length === 1 ? 'person' : 'people'}
            {groups.length > 0 && <> · {groups.length.toLocaleString()} groups to name</>}
            {index.phase === 'recognizing' && <> · updating…</>}
          </>
        }
      />

      {people.length > 0 && (
        <section className="px-8">
          <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-1">
            {people.map((p) => (
              <PersonCard key={p.id} p={p} selected={selected.includes(p.id)} onSelect={() => toggle(p.id)} />
            ))}
          </div>
        </section>
      )}

      {groups.length > 0 && (
        <section className="mt-10 px-10">
          <div className="mb-1 flex items-center gap-2">
            <Sparkles size={16} className="text-accent" />
            <h2 className="font-display text-[22px] font-medium tracking-tight text-text">Name these faces</h2>
          </div>
          <p className="mb-5 text-[13.5px] text-muted">
            Smriti grouped faces that look alike. Name a group once and it finds that person everywhere, including as
            they grow up.
          </p>
          <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">
            <AnimatePresence initial={false}>
              {groups.map((g) => (
                <GroupCard key={g.id} g={g} onOpen={() => setOpenGroup(g.id)} />
              ))}
            </AnimatePresence>
          </div>
        </section>
      )}

      <GroupDialog groupId={openGroup} onClose={() => setOpenGroup(null)} />

      <AnimatePresence>
        {selected.length > 0 && (
          <motion.div
            initial={{ y: 24, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 24, opacity: 0 }}
            className="fixed bottom-6 left-[calc(50%+116px)] z-30 flex -translate-x-1/2 items-center gap-2 rounded-2xl border border-border bg-elevated py-2 pr-2 pl-4 shadow-panel"
          >
            <div className="flex -space-x-2">
              {selected.slice(0, 5).map((id) => {
                const p = people.find((x) => x.id === id)
                return <FaceAvatar key={id} faceId={p?.coverFaceId} name={p?.name} size={28} className="ring-2 ring-elevated" />
              })}
            </div>
            <span className="mx-2 text-[13px] text-muted">{selected.length} selected</span>
            <Button size="sm" variant="primary" icon={<Images size={14} />} onClick={() => showTogether(false)}>
              {selected.length === 1 ? 'Their photos' : 'Photos together'}
            </Button>
            <Button size="sm" onClick={() => showTogether(true)}>
              {selected.length === 1 ? 'Only them' : 'Only them, no one else'}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setSelected([])}>
              Clear
            </Button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
