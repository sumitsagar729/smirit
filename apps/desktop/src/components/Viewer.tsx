import { Calendar, Camera, ChevronLeft, ChevronRight, FolderOpen, Info, MapPin, X } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'
import { useEffect, useState } from 'react'

import { mediaUrl, revealInExplorer, thumbUrl } from '../lib/api'
import type { AssetDetail, Tile } from '../lib/bindings'
import { cn } from '../lib/cn'
import { formatBytes, formatDuration, formatLongDate as formatDate, formatTime } from '../lib/format'
import { useAssetDetail } from '../lib/queries'
import { useViewer } from '../state/viewer'

const DATE_SOURCE: Record<string, string> = {
  exif: 'From the camera',
  video: 'From the video',
  filename: 'From the file name',
  mtime: 'From the file date (may be inaccurate)',
  user: 'Set by you',
}

/** Formats the browser can show natively; others fall back to the thumbnail. */
function displayable(d: AssetDetail | undefined) {
  if (!d?.mime) return false
  return /^image\/(jpeg|png|webp|gif|bmp)$/.test(d.mime) || /^video\/(mp4|quicktime|webm)$/.test(d.mime)
}

function InfoPanel({ d }: { d: AssetDetail }) {
  const row = (icon: React.ReactNode, main: React.ReactNode, sub?: React.ReactNode) => (
    <div className="flex gap-3 py-3">
      <div className="mt-0.5 text-faint">{icon}</div>
      <div className="min-w-0">
        <div className="text-[13.5px] text-text">{main}</div>
        {sub && <div className="mt-0.5 text-[12.5px] text-muted">{sub}</div>}
      </div>
    </div>
  )
  return (
    <motion.aside
      initial={{ x: 24, opacity: 0 }}
      animate={{ x: 0, opacity: 1 }}
      exit={{ x: 24, opacity: 0 }}
      transition={{ type: 'spring', stiffness: 400, damping: 36 }}
      className="w-[320px] shrink-0 overflow-y-auto border-l border-white/10 bg-[#141416]/95 px-5 py-5 text-text"
    >
      <h3 className="mb-1 text-[12px] font-semibold tracking-[0.1em] text-faint uppercase">Details</h3>
      <div className="divide-y divide-white/5">
        {row(<Calendar size={16} />, formatDate(d.takenAt), (
          <>
            {formatTime(d.takenAt)} · {DATE_SOURCE[d.takenAtSource ?? ''] ?? 'Unknown source'}
          </>
        ))}
        {row(
          <FolderOpen size={16} />,
          <span className="selectable break-all">{d.fileName}</span>,
          <button
            type="button"
            onClick={() => revealInExplorer(d.path)}
            className="selectable text-left break-all text-accent hover:underline"
          >
            {d.folder}
          </button>,
        )}
        {row(
          <Camera size={16} />,
          d.camera ?? 'Camera unknown',
          [d.width && d.height ? `${d.width} × ${d.height}` : null, formatBytes(d.sizeBytes ?? 0), d.duration ? formatDuration(d.duration) : null]
            .filter(Boolean)
            .join(' · '),
        )}
        {d.lat != null &&
          d.lon != null &&
          row(<MapPin size={16} />, 'Location saved', `${d.lat.toFixed(4)}, ${d.lon.toFixed(4)} · map arrives with Places`)}
      </div>
    </motion.aside>
  )
}

/** Keyed by asset id by the caller, so `loaded` starts fresh for each item. */
function Media({ tile, detail }: { tile: Tile; detail?: AssetDetail }) {
  const [loaded, setLoaded] = useState(false)
  const native = displayable(detail)

  if (tile.kind === 'video' && native) {
    return <video key={tile.id} src={mediaUrl(tile.id)} controls autoPlay className="max-h-full max-w-full rounded-md" />
  }
  return (
    <div className="relative flex size-full items-center justify-center">
      {/* Thumbnail first for an instant frame, then the original on top. */}
      {tile.hasThumb && (
        <img
          src={thumbUrl(tile.id)}
          alt=""
          className={cn('absolute max-h-full max-w-full object-contain blur-[1px]', loaded && 'invisible')}
        />
      )}
      {native && tile.kind === 'photo' && (
        <img
          key={tile.id}
          src={mediaUrl(tile.id)}
          alt=""
          onLoad={() => setLoaded(true)}
          className={cn('relative max-h-full max-w-full object-contain transition-opacity duration-200', loaded ? 'opacity-100' : 'opacity-0')}
        />
      )}
      {detail && !native && (
        <div className="absolute bottom-6 rounded-full bg-black/60 px-4 py-2 text-[13px] text-white/80">
          Full preview for this format is coming soon
        </div>
      )}
    </div>
  )
}

export function Viewer() {
  const { items, index, close, step } = useViewer()
  const [showInfo, setShowInfo] = useState(true)
  const tile = index != null ? items[index] : undefined
  const { data: detail } = useAssetDetail(tile?.id ?? null)

  useEffect(() => {
    if (index == null) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close()
      else if (e.key === 'ArrowRight') step(1)
      else if (e.key === 'ArrowLeft') step(-1)
      else if (e.key.toLowerCase() === 'i') setShowInfo((v) => !v)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [index, close, step])

  // Warm the cache for the neighbours so arrowing feels instant.
  useEffect(() => {
    if (index == null) return
    for (const n of [items[index + 1], items[index - 1]]) {
      if (n?.kind === 'photo') new Image().src = mediaUrl(n.id)
    }
  }, [index, items])

  return (
    <AnimatePresence>
      {tile && (
        <motion.div
          key="viewer"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.18 }}
          className="fixed inset-0 z-40 flex bg-black/95"
          role="dialog"
          aria-modal="true"
          aria-label="Photo viewer"
        >
          <div className="relative flex min-w-0 flex-1 flex-col">
            <div className="flex h-14 shrink-0 items-center justify-between px-4 text-white/80">
              <button type="button" onClick={close} className="rounded-lg p-2 hover:bg-white/10" aria-label="Close">
                <X size={20} />
              </button>
              <div className="text-[13px] text-white/60">
                {formatDate(tile.takenAt)} · {index! + 1} of {items.length.toLocaleString()}
              </div>
              <button
                type="button"
                onClick={() => setShowInfo((v) => !v)}
                className={cn('rounded-lg p-2 hover:bg-white/10', showInfo && 'bg-white/10 text-white')}
                aria-label="Toggle details"
              >
                <Info size={19} />
              </button>
            </div>
            <div className="relative min-h-0 flex-1 px-16 pb-8">
              <Media key={tile.id} tile={tile} detail={detail} />
              <button
                type="button"
                onClick={() => step(-1)}
                disabled={index === 0}
                className="absolute top-1/2 left-3 -translate-y-1/2 rounded-full bg-white/5 p-2.5 text-white/80 hover:bg-white/15 disabled:opacity-0"
                aria-label="Previous"
              >
                <ChevronLeft size={22} />
              </button>
              <button
                type="button"
                onClick={() => step(1)}
                disabled={index === items.length - 1}
                className="absolute top-1/2 right-3 -translate-y-1/2 rounded-full bg-white/5 p-2.5 text-white/80 hover:bg-white/15 disabled:opacity-0"
                aria-label="Next"
              >
                <ChevronRight size={22} />
              </button>
            </div>
          </div>
          <AnimatePresence>{showInfo && detail && <InfoPanel d={detail} />}</AnimatePresence>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
