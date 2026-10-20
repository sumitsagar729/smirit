import { Users, X } from 'lucide-react'
import { useState } from 'react'

import type { Id } from '../../lib/api'
import { useAssetFaces, useAssignFaces, useRejectFaces } from '../../lib/queries'
import { useNav } from '../../state/nav'
import { useViewer } from '../../state/viewer'
import { FaceAvatar } from './FaceAvatar'
import { PersonPicker } from './PersonPicker'

/** Who is in the open photo, with quick naming and corrections. */
export function PeopleInPhoto({ assetId }: { assetId: Id }) {
  const { data: faces = [] } = useAssetFaces(assetId)
  const assign = useAssignFaces()
  const reject = useRejectFaces()
  const openPerson = useNav((s) => s.openPerson)
  const closeViewer = useViewer((s) => s.close)
  const [naming, setNaming] = useState<Id | null>(null)
  if (!faces.length) return null

  return (
    <div className="flex gap-3 py-3">
      <div className="mt-0.5 text-faint">
        <Users size={16} />
      </div>
      <div className="min-w-0 flex-1 space-y-1.5">
        {faces.map((f) =>
          f.personId != null ? (
            <div key={f.id} className="group flex items-center gap-2.5">
              <button
                type="button"
                onClick={() => {
                  closeViewer()
                  openPerson(f.personId!)
                }}
                className="flex min-w-0 flex-1 items-center gap-2.5 rounded-lg text-left text-[13.5px] text-text hover:text-accent"
              >
                <FaceAvatar faceId={f.id} name={f.personName} size={32} />
                <span className="truncate">{f.personName}</span>
                {!f.confirmed && <span className="text-[11.5px] text-faint">recognised</span>}
              </button>
              <button
                type="button"
                onClick={() => reject.mutate([f.id])}
                title={`Not ${f.personName}`}
                aria-label={`Not ${f.personName}`}
                className="rounded-md p-1 text-faint opacity-0 group-hover:opacity-100 hover:bg-white/10 hover:text-text"
              >
                <X size={14} />
              </button>
            </div>
          ) : naming === f.id ? (
            <div key={f.id} className="flex items-center gap-2.5">
              <FaceAvatar faceId={f.id} size={32} />
              <PersonPicker
                autoFocus
                className="flex-1"
                onPick={(to) => {
                  setNaming(null)
                  assign.mutate({ faces: [f.id], to })
                }}
              />
            </div>
          ) : (
            <button
              key={f.id}
              type="button"
              onClick={() => setNaming(f.id)}
              className="flex w-full items-center gap-2.5 rounded-lg text-left text-[13.5px] text-muted hover:text-text"
            >
              <FaceAvatar faceId={f.id} size={32} />
              <span>Who is this?</span>
            </button>
          ),
        )}
      </div>
    </div>
  )
}
