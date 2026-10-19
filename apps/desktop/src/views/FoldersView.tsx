import { Folder, FolderPlus, HardDrive, Trash2 } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useState } from 'react'

import { PageHeader } from '../components/PageHeader'
import { Button } from '../components/ui/Button'
import type { Source } from '../lib/api'
import { useAddFolder, useRemoveFolder, useSources } from '../lib/queries'

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' })
}

function SourceRow({ source }: { source: Source }) {
  const remove = useRemoveFolder()
  const [confirming, setConfirming] = useState(false)
  const name = source.path.split(/[\\/]/).filter(Boolean).pop() ?? source.path

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, x: -16 }}
      className="group flex items-center gap-4 rounded-2xl border border-border bg-surface px-5 py-4"
    >
      <div className="grid size-11 shrink-0 place-items-center rounded-xl bg-accent-soft text-accent">
        <Folder size={21} strokeWidth={1.8} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-[15px] font-medium text-text">{name}</div>
        <div className="selectable truncate text-[12.5px] text-muted" title={source.path}>
          {source.path}
        </div>
      </div>
      <div className="hidden text-right text-[12.5px] text-muted md:block">
        <div className="text-text">
          {source.photos.toLocaleString()} photos · {source.videos.toLocaleString()} videos
        </div>
        <div>
          {source.lastScanAt ? `Scanned ${formatDate(source.lastScanAt)}` : `Added ${formatDate(source.addedAt)}`}
        </div>
      </div>
      {confirming ? (
        <div className="flex items-center gap-1">
          <Button size="sm" variant="ghost" onClick={() => setConfirming(false)}>
            Cancel
          </Button>
          <Button size="sm" variant="danger" onClick={() => remove.mutate(source.id)} disabled={remove.isPending}>
            Remove from library
          </Button>
        </div>
      ) : (
        <Button
          size="sm"
          variant="ghost"
          aria-label={`Remove ${name}`}
          className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
          icon={<Trash2 size={15} />}
          onClick={() => setConfirming(true)}
        />
      )}
    </motion.li>
  )
}

export function FoldersView() {
  const { data: sources = [] } = useSources()
  const addFolder = useAddFolder()

  return (
    <div className="h-full overflow-y-auto">
      <PageHeader
        title="Folders"
        subtitle="Smriti only reads these folders. Your original files are never changed, moved or uploaded."
        actions={
          <Button variant="primary" icon={<FolderPlus size={16} />} onClick={() => addFolder.mutate()}>
            Add folder
          </Button>
        }
      />
      <div className="px-10 pb-10">
        {sources.length === 0 ? (
          <button
            type="button"
            onClick={() => addFolder.mutate()}
            className="flex w-full flex-col items-center gap-3 rounded-2xl border border-dashed border-border px-6 py-14 text-muted transition-colors hover:border-accent/50 hover:bg-accent-soft/40 hover:text-text"
          >
            <HardDrive size={28} strokeWidth={1.6} />
            <span className="text-[15px] font-medium">Add your first folder</span>
            <span className="text-[13px]">Local disks, USB drives and network folders all work.</span>
          </button>
        ) : (
          <ul className="space-y-2.5">
            <AnimatePresence initial={false}>
              {sources.map((s) => (
                <SourceRow key={s.id} source={s} />
              ))}
            </AnimatePresence>
          </ul>
        )}
      </div>
    </div>
  )
}
