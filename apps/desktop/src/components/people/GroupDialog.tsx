import { EyeOff, X } from 'lucide-react'
import { Dialog } from 'radix-ui'
import { useState } from 'react'

import type { Id } from '../../lib/api'
import { cn } from '../../lib/cn'
import { useAssignFaces, useGroupFaces, useIgnoreFaces } from '../../lib/queries'
import { toast } from '../../state/toast'
import { Button } from '../ui/Button'
import { FaceAvatar } from './FaceAvatar'
import { PersonPicker } from './PersonPicker'

/**
 * Every face in a group. Click faces that don't belong to leave them out,
 * then name the rest in one go.
 */
export function GroupDialog({ groupId, onClose }: { groupId: Id | null; onClose: () => void }) {
  const { data: faces = [], isLoading } = useGroupFaces(groupId)
  const [left, setLeft] = useState<Set<Id>>(new Set())
  const assign = useAssignFaces()
  const ignore = useIgnoreFaces()
  const chosen = faces.filter((f) => !left.has(f.id)).map((f) => f.id)

  const close = () => {
    setLeft(new Set())
    onClose()
  }
  const toggle = (id: Id) =>
    setLeft((s) => {
      const n = new Set(s)
      if (n.has(id)) n.delete(id)
      else n.add(id)
      return n
    })

  return (
    <Dialog.Root open={groupId != null} onOpenChange={(o) => !o && close()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/60 backdrop-blur-[2px]" />
        <Dialog.Content className="fixed top-1/2 left-1/2 z-50 flex max-h-[86vh] w-[min(880px,92vw)] -translate-x-1/2 -translate-y-1/2 flex-col rounded-2xl border border-border bg-surface shadow-panel focus:outline-none">
          <header className="flex items-start justify-between gap-4 px-6 pt-5 pb-3">
            <div>
              <Dialog.Title className="font-display text-[22px] font-medium text-text">Who is this?</Dialog.Title>
              <Dialog.Description className="mt-1 text-[13px] text-muted">
                {faces.length.toLocaleString()} similar faces. Click any that are someone else to leave them out. Young
                brothers and sisters can look alike, so check the years.
              </Dialog.Description>
            </div>
            <Dialog.Close className="rounded-lg p-1.5 text-muted hover:bg-hover hover:text-text" aria-label="Close">
              <X size={18} />
            </Dialog.Close>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-4">
            {isLoading ? (
              <div className="py-16 text-center text-[13px] text-faint">Loading faces…</div>
            ) : (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(76px,1fr))] gap-2">
                {faces.map((f) => {
                  const out = left.has(f.id)
                  return (
                    <button
                      key={f.id}
                      type="button"
                      onClick={() => toggle(f.id)}
                      aria-pressed={!out}
                      aria-label={out ? 'Left out' : 'Included'}
                      className={cn(
                        'relative aspect-square overflow-hidden rounded-[10px] ring-2 transition',
                        out ? 'opacity-30 ring-danger/70 grayscale' : 'ring-transparent hover:ring-accent/50',
                      )}
                    >
                      <FaceAvatar faceId={f.id} size={200} square className="!size-full" />
                      {f.takenAt && (
                        <span className="absolute bottom-1 left-1 rounded bg-black/60 px-1 text-[10.5px] font-medium text-white/90 tabular-nums">
                          {f.takenAt.slice(0, 4)}
                        </span>
                      )}
                      {out && <X size={22} className="absolute inset-0 m-auto text-white" />}
                    </button>
                  )
                })}
              </div>
            )}
          </div>

          <footer className="flex items-center gap-3 border-t border-border px-6 py-4">
            <PersonPicker
              className="flex-1"
              autoFocus
              placeholder={`Name ${chosen.length.toLocaleString()} ${chosen.length === 1 ? 'face' : 'faces'}…`}
              disabled={!chosen.length || assign.isPending}
              onPick={(to, label) =>
                assign.mutate(
                  { faces: chosen, to },
                  {
                    onSuccess: () => {
                      toast.success(`Named ${label}`, 'Smriti is now looking for them in your other photos.')
                      close()
                    },
                  },
                )
              }
            />
            <Button
              variant="ghost"
              icon={<EyeOff size={15} />}
              disabled={!chosen.length || ignore.isPending}
              onClick={() => ignore.mutate(chosen, { onSuccess: close })}
            >
              Not anyone I know
            </Button>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
