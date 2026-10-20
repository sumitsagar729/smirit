import { useState } from 'react'

import { faceUrl, type Id } from '../../lib/api'
import { cn } from '../../lib/cn'

// Warm, distinct tones for initials while a face loads (or when there's none).
const TONES = ['#7a5a3a', '#4f6a7a', '#6d4f6f', '#5a6b45', '#7a4f45', '#45606b']

function initials(name?: string | null) {
  if (!name) return ''
  const parts = name.trim().split(/\s+/)
  return (parts[0][0] + (parts.length > 1 ? parts[parts.length - 1][0] : '')).toUpperCase()
}

/** A face crop, round by default; falls back to initials. */
export function FaceAvatar({
  faceId,
  name,
  size = 40,
  square = false,
  className,
}: {
  faceId: Id | null | undefined
  name?: string | null
  size?: number
  square?: boolean
  className?: string
}) {
  const [failed, setFailed] = useState(false)
  const seed = faceId ?? [...(name ?? '?')].reduce((a, c) => a + c.charCodeAt(0), 0)
  return (
    <span
      className={cn(
        'relative inline-grid shrink-0 place-items-center overflow-hidden font-medium text-white/85 select-none',
        square ? 'rounded-[10px]' : 'rounded-full',
        className,
      )}
      style={{ width: size, height: size, background: TONES[seed % TONES.length], fontSize: Math.max(10, size * 0.36) }}
    >
      {initials(name)}
      {faceId != null && !failed && (
        <img
          src={faceUrl(faceId)}
          alt=""
          draggable={false}
          loading="lazy"
          onError={() => setFailed(true)}
          className="absolute inset-0 size-full object-cover"
        />
      )}
    </span>
  )
}
