import { FolderPlus, ScanSearch } from 'lucide-react'

import { PhotoStack } from '../components/art'
import { Button } from '../components/ui/Button'
import { EmptyState } from '../components/ui/EmptyState'
import { useAddFolder, useStats } from '../lib/queries'
import { useNav } from '../state/nav'

export function TimelineView() {
  const { data: stats, isLoading } = useStats()
  const addFolder = useAddFolder()
  const go = useNav((s) => s.go)

  if (isLoading) return null

  if (!stats?.sources) {
    return (
      <div className="grid h-full place-items-center pb-16">
        <EmptyState
          art={<PhotoStack />}
          eyebrow="Welcome to Smriti"
          title="Your family's story starts here"
          body={
            <>
              Add the folders where you keep photos and videos. Smriti organises them by who, when and where, right
              on this computer. Nothing is ever uploaded.
            </>
          }
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

  return (
    <div className="grid h-full place-items-center pb-16">
      <EmptyState
        art={<PhotoStack />}
        eyebrow="Folders ready"
        title="Scanning arrives in the next build"
        body={
          <>
            {stats.sources === 1 ? 'Your folder is' : `Your ${stats.sources} folders are`} saved. The next milestone
            adds scanning, thumbnails and the timeline, so your photos will appear here.
          </>
        }
        actions={
          <Button icon={<ScanSearch size={16} />} onClick={() => go('folders')}>
            Manage folders
          </Button>
        }
      />
    </div>
  )
}
