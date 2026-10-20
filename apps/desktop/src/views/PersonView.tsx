import { ArrowLeft, Check, Heart, MoreHorizontal, Trash2, Users, X } from 'lucide-react'
import { DropdownMenu, Dialog } from 'radix-ui'
import { useEffect, useState } from 'react'

import { FaceAvatar } from '../components/people/FaceAvatar'
import { PersonPicker } from '../components/people/PersonPicker'
import { TimelineGrid } from '../components/TimelineGrid'
import { Button } from '../components/ui/Button'
import type { FaceRef, Id, Person } from '../lib/api'
import { cn } from '../lib/cn'
import {
  useAssignFaces,
  useDeletePerson,
  useMergePeople,
  usePerson,
  usePersonFaces,
  useRejectFaces,
  useTimeline,
  useUpdatePerson,
} from '../lib/queries'
import { useNav } from '../state/nav'
import { toast } from '../state/toast'

// Birth dates can't be in the future; computed once per session.
const TODAY = new Date().toISOString().slice(0, 10)

function ageText(birth: string | null) {
  if (!birth) return null
  const b = new Date(birth)
  const now = new Date()
  let months = (now.getFullYear() - b.getFullYear()) * 12 + now.getMonth() - b.getMonth()
  if (now.getDate() < b.getDate()) months--
  if (months < 0) return null
  if (months < 24) return `${months} ${months === 1 ? 'month' : 'months'} old`
  return `${Math.floor(months / 12)} years old`
}

function Details({ p }: { p: Person }) {
  const update = useUpdatePerson()
  const [name, setName] = useState(p.name)
  const [birth, setBirth] = useState(p.birthDate ?? '')

  const save = (patch: Partial<{ name: string; birthDate: string; isFamily: boolean }>) => {
    const next = { name, birthDate: birth || null, isFamily: p.isFamily, ...patch }
    if (next.name === p.name && (next.birthDate || null) === p.birthDate && next.isFamily === p.isFamily) return
    update.mutate(
      { id: p.id, update: { name: next.name, birthDate: next.birthDate || null, isFamily: next.isFamily } },
      {
        onError: () => {
          setName(p.name)
          setBirth(p.birthDate ?? '')
        },
      },
    )
  }

  const age = ageText(p.birthDate)
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <input
        value={name}
        aria-label="Name"
        onChange={(e) => setName(e.target.value)}
        onBlur={() => save({ name })}
        onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
        className="-ml-1.5 w-full max-w-[420px] rounded-lg bg-transparent px-1.5 font-display text-[34px] leading-tight font-medium tracking-tight text-text hover:bg-hover focus:bg-hover focus:outline-none"
      />
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[13.5px] text-muted">
        <span className="tabular-nums">
          {p.photoCount.toLocaleString()} photos{age && <> · {age}</>}
        </span>
        <label className="flex items-center gap-2">
          <span className="text-faint">Born</span>
          <input
            type="date"
            value={birth}
            max={TODAY}
            onChange={(e) => setBirth(e.target.value)}
            onBlur={() => save({ birthDate: birth })}
            className="h-8 rounded-lg border border-border bg-bg px-2 text-[13px] text-text [color-scheme:inherit] focus:border-accent/60 focus:outline-none"
          />
        </label>
        <button
          type="button"
          aria-pressed={p.isFamily}
          onClick={() => save({ isFamily: !p.isFamily })}
          className={cn(
            'flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-[13px] transition-colors',
            p.isFamily ? 'border-accent/50 bg-accent-soft text-accent' : 'border-border text-muted hover:text-text',
          )}
        >
          <Heart size={13} className={cn(p.isFamily && 'fill-accent')} /> Family
        </button>
      </div>
      {!p.birthDate && (
        <p className="text-[12.5px] text-faint">
          Tip: for children, adding a birth date helps Smriti recognise them from newborn to school age.
        </p>
      )}
    </div>
  )
}

function ReviewFace({ f, busy, onYes, onNo }: { f: FaceRef; busy: boolean; onYes: () => void; onNo: () => void }) {
  return (
    <div className="group relative">
      <FaceAvatar faceId={f.id} size={200} square className="!aspect-square !h-auto !w-full" />
      <div className="absolute inset-x-1.5 bottom-1.5 flex gap-1 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100">
        <button
          type="button"
          disabled={busy}
          onClick={onYes}
          aria-label="Yes, that's them"
          className="grid h-7 flex-1 place-items-center rounded-md bg-success/90 text-white hover:bg-success"
        >
          <Check size={15} />
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={onNo}
          aria-label="No, someone else"
          className="grid h-7 flex-1 place-items-center rounded-md bg-black/70 text-white hover:bg-danger"
        >
          <X size={15} />
        </button>
      </div>
    </div>
  )
}

function Review({ p }: { p: Person }) {
  const { data: faces = [], isLoading } = usePersonFaces(p.id, true)
  const [hidden, setHidden] = useState<Set<Id>>(new Set())
  const assign = useAssignFaces()
  const reject = useRejectFaces()
  const shown = faces.filter((f) => !hidden.has(f.id)).slice(0, 120)
  const busy = assign.isPending || reject.isPending
  const drop = (ids: Id[]) => setHidden((s) => new Set([...s, ...ids]))

  if (isLoading) return null
  if (!shown.length) {
    return (
      <div className="grid place-items-center py-24 text-center">
        <div className="text-[15px] text-text">Nothing to review</div>
        <div className="mt-1 text-[13.5px] text-muted">Every photo of {p.name} is confirmed.</div>
      </div>
    )
  }
  return (
    <div className="px-10 pb-16">
      <div className="mb-4 flex items-center justify-between gap-4">
        <p className="text-[13.5px] text-muted">
          Smriti thinks these are {p.name}, least sure first. Confirming helps it recognise {p.name} better.
        </p>
        <Button
          size="sm"
          variant="primary"
          icon={<Check size={14} />}
          disabled={busy}
          onClick={() => {
            const ids = shown.map((f) => f.id)
            assign.mutate({ faces: ids, to: { personId: p.id, name: null } }, { onSuccess: () => drop(ids) })
          }}
        >
          All {shown.length} are {p.name}
        </Button>
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] gap-2">
        {shown.map((f) => (
          <ReviewFace
            key={f.id}
            f={f}
            busy={busy}
            onYes={() => {
              drop([f.id])
              assign.mutate({ faces: [f.id], to: { personId: p.id, name: null } })
            }}
            onNo={() => {
              drop([f.id])
              reject.mutate([f.id])
            }}
          />
        ))}
      </div>
    </div>
  )
}

function MergeDialog({ p, open, onClose }: { p: Person; open: boolean; onClose: () => void }) {
  const merge = useMergePeople()
  const openPerson = useNav((s) => s.openPerson)
  return (
    <Dialog.Root open={open} onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60" />
        <Dialog.Content className="fixed top-1/2 left-1/2 z-50 w-[420px] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-border bg-surface p-6 shadow-panel focus:outline-none">
          <Dialog.Title className="font-display text-[22px] font-medium text-text">Same person as…</Dialog.Title>
          <Dialog.Description className="mt-1 mb-4 text-[13.5px] text-muted">
            All of {p.name}'s photos move to the person you pick, and “{p.name}” goes away.
          </Dialog.Description>
          <PersonPicker
            autoFocus
            existingOnly
            exclude={[p.id]}
            placeholder="Search people"
            onPick={(to, label) => {
              if (to.personId == null) return
              const into = to.personId
              merge.mutate(
                { from: p.id, into },
                {
                  onSuccess: () => {
                    toast.success(`Merged into ${label}`)
                    onClose()
                    openPerson(into)
                  },
                },
              )
            }}
          />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

export function PersonView() {
  const id = useNav((s) => s.personId)!
  const go = useNav((s) => s.go)
  const { data: p, isError } = usePerson(id)
  const { data: suggestions = [] } = usePersonFaces(id, true)
  const { data: tiles = [] } = useTimeline({ people: [id], only: false })
  const remove = useDeletePerson()
  const [tab, setTab] = useState<'photos' | 'review'>('photos')
  const [merging, setMerging] = useState(false)

  useEffect(() => {
    if (isError) go('people')
  }, [isError, go])
  if (!p) return null

  return (
    <div className="flex h-full flex-col">
      <header className="shrink-0 px-10 pt-6">
        <button
          type="button"
          onClick={() => go('people')}
          className="-ml-2 mb-3 flex items-center gap-1.5 rounded-lg px-2 py-1 text-[13px] text-muted hover:bg-hover hover:text-text"
        >
          <ArrowLeft size={15} /> People
        </button>
        <div className="flex items-start gap-6">
          <FaceAvatar faceId={p.coverFaceId} name={p.name} size={112} />
          <Details key={`${p.name}|${p.birthDate}`} p={p} />
          <DropdownMenu.Root>
            <DropdownMenu.Trigger className="ml-auto rounded-lg p-2 text-muted hover:bg-hover hover:text-text" aria-label="More">
              <MoreHorizontal size={18} />
            </DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content
                align="end"
                sideOffset={4}
                className="z-50 min-w-[200px] rounded-xl border border-border bg-elevated p-1 shadow-panel"
              >
                <DropdownMenu.Item
                  onSelect={() => setMerging(true)}
                  className="flex h-9 cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-[13.5px] text-text outline-none data-[highlighted]:bg-hover"
                >
                  <Users size={15} /> Same person as…
                </DropdownMenu.Item>
                <DropdownMenu.Item
                  onSelect={() =>
                    remove.mutate(p.id, {
                      onSuccess: () => {
                        toast.info(`Removed ${p.name}`, 'Their faces are back in "Name these faces".')
                        go('people')
                      },
                    })
                  }
                  className="flex h-9 cursor-pointer items-center gap-2.5 rounded-lg px-2.5 text-[13.5px] text-danger outline-none data-[highlighted]:bg-danger/10"
                >
                  <Trash2 size={15} /> Remove person
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
        <div role="tablist" className="mt-6 flex gap-1 border-b border-border">
          {(
            [
              ['photos', `Photos · ${tiles.length.toLocaleString()}`],
              ['review', `Review${suggestions.length ? ` · ${suggestions.length.toLocaleString()}` : ''}`],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={tab === key}
              onClick={() => setTab(key)}
              className={cn(
                '-mb-px border-b-2 px-3 pb-2.5 text-[13.5px] font-medium transition-colors',
                tab === key ? 'border-accent text-text' : 'border-transparent text-muted hover:text-text',
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </header>
      <div className="min-h-0 flex-1 pt-2">
        {tab === 'photos' ? (
          <TimelineGrid tiles={tiles} rowHeight={200} />
        ) : (
          <div className="h-full overflow-y-auto pt-4">
            <Review p={p} />
          </div>
        )}
      </div>
      <MergeDialog p={p} open={merging} onClose={() => setMerging(false)} />
    </div>
  )
}
