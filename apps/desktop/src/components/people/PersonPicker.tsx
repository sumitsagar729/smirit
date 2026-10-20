import { Check, UserPlus } from 'lucide-react'
import { useId, useMemo, useState } from 'react'

import type { Id, PersonRef } from '../../lib/api'
import { cn } from '../../lib/cn'
import { usePeople } from '../../lib/queries'
import { FaceAvatar } from './FaceAvatar'

type Props = {
  onPick: (to: PersonRef, label: string) => void
  placeholder?: string
  autoFocus?: boolean
  /** People not to offer (e.g. the person being merged). */
  exclude?: Id[]
  /** Only existing people (no "add new"). */
  existingOnly?: boolean
  disabled?: boolean
  className?: string
}

/**
 * "Who is this?": type a name, pick someone you've already named, or press
 * Enter to add a new person.
 */
export function PersonPicker({ onPick, placeholder = 'Who is this?', autoFocus, exclude = [], existingOnly, disabled, className }: Props) {
  const { data: people = [] } = usePeople()
  const [text, setText] = useState('')
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const listId = useId()

  const q = text.trim().toLowerCase()
  const matches = useMemo(
    () =>
      people
        .filter((p) => !exclude.includes(p.id) && (!q || p.name.toLowerCase().includes(q)))
        .sort((a, b) => Number(b.name.toLowerCase().startsWith(q)) - Number(a.name.toLowerCase().startsWith(q)))
        .slice(0, 6),
    [people, q, exclude],
  )
  const exact = people.some((p) => p.name.toLowerCase() === q)
  const options: { key: string; label: string; to: PersonRef; faceId?: Id | null; isNew?: boolean }[] = [
    ...matches.map((p) => ({ key: `p${p.id}`, label: p.name, to: { personId: p.id, name: null }, faceId: p.coverFaceId })),
    ...(!existingOnly && q && !exact ? [{ key: 'new', label: text.trim(), to: { personId: null, name: text.trim() }, isNew: true }] : []),
  ]

  const pick = (i: number) => {
    const o = options[i]
    if (!o) return
    onPick(o.to, o.label)
    setText('')
    setOpen(false)
  }

  return (
    <div className={cn('relative', className)}>
      <input
        value={text}
        disabled={disabled}
        autoFocus={autoFocus}
        placeholder={placeholder}
        role="combobox"
        aria-expanded={open && options.length > 0}
        aria-controls={listId}
        aria-autocomplete="list"
        onChange={(e) => {
          setText(e.target.value)
          setActive(0)
          setOpen(true)
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setTimeout(() => setOpen(false), 120)}
        onKeyDown={(e) => {
          if (e.key === 'ArrowDown') {
            e.preventDefault()
            setActive((a) => Math.min(a + 1, options.length - 1))
          } else if (e.key === 'ArrowUp') {
            e.preventDefault()
            setActive((a) => Math.max(a - 1, 0))
          } else if (e.key === 'Enter') {
            e.preventDefault()
            pick(active)
          } else if (e.key === 'Escape') {
            e.stopPropagation()
            setOpen(false)
          }
        }}
        className="h-9 w-full rounded-lg border border-border bg-bg px-3 text-[13.5px] text-text placeholder:text-faint focus:border-accent/60 focus:outline-none disabled:opacity-50"
      />
      {open && options.length > 0 && (
        <ul
          id={listId}
          role="listbox"
          className="absolute top-full right-0 left-0 z-50 mt-1 overflow-hidden rounded-xl border border-border bg-elevated p-1 shadow-panel"
        >
          {options.map((o, i) => (
            <li
              key={o.key}
              role="option"
              aria-selected={i === active}
              onMouseDown={(e) => {
                e.preventDefault()
                pick(i)
              }}
              onMouseEnter={() => setActive(i)}
              className={cn(
                'flex h-9 cursor-pointer items-center gap-2.5 rounded-lg px-2 text-[13.5px]',
                i === active ? 'bg-hover text-text' : 'text-muted',
              )}
            >
              {o.isNew ? (
                <span className="grid size-6 place-items-center rounded-full bg-accent-soft text-accent">
                  <UserPlus size={13} />
                </span>
              ) : (
                <FaceAvatar faceId={o.faceId} name={o.label} size={24} />
              )}
              <span className="flex-1 truncate">{o.isNew ? <>Add “{o.label}”</> : o.label}</span>
              {i === active && <Check size={14} className="text-accent" />}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
