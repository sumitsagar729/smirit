import { BookImage, FolderOpen, Images, MapPinned, Search, Settings, Users } from 'lucide-react'
import { motion } from 'motion/react'
import type { ComponentType } from 'react'

import { cn } from '../lib/cn'
import { useStats } from '../lib/queries'
import { useNav, type View } from '../state/nav'
import { Logo } from './Logo'
import { Kbd } from './ui/Kbd'

type Item = { view: View; label: string; icon: ComponentType<{ size?: number; strokeWidth?: number }> }

const library: Item[] = [
  { view: 'timeline', label: 'Timeline', icon: Images },
  { view: 'people', label: 'People', icon: Users },
  { view: 'places', label: 'Places', icon: MapPinned },
  { view: 'albums', label: 'Albums', icon: BookImage },
]
const manage: Item[] = [{ view: 'folders', label: 'Folders', icon: FolderOpen }]

function NavButton({ item }: { item: Item }) {
  const { view, go } = useNav()
  const active = view === item.view
  const Icon = item.icon
  return (
    <button
      type="button"
      onClick={() => go(item.view)}
      aria-current={active ? 'page' : undefined}
      className={cn(
        'relative flex h-9 w-full items-center gap-3 rounded-lg px-3 text-[13.5px] font-medium transition-colors duration-150',
        active ? 'text-text' : 'text-muted hover:bg-hover hover:text-text',
      )}
    >
      {active && (
        <motion.span
          layoutId="nav-active"
          className="absolute inset-0 rounded-lg bg-elevated shadow-panel"
          transition={{ type: 'spring', stiffness: 500, damping: 40 }}
        />
      )}
      <span className={cn('relative', active && 'text-accent')}>
        <Icon size={17} strokeWidth={1.9} />
      </span>
      <span className="relative">{item.label}</span>
    </button>
  )
}

function Section({ title, items }: { title: string; items: Item[] }) {
  return (
    <div className="space-y-0.5">
      <div className="px-3 pt-4 pb-1.5 text-[11px] font-semibold tracking-[0.1em] text-faint uppercase">{title}</div>
      {items.map((i) => (
        <NavButton key={i.view} item={i} />
      ))}
    </div>
  )
}

function LibraryStatus() {
  const { data } = useStats()
  const total = (data?.photos ?? 0) + (data?.videos ?? 0)
  return (
    <div className="rounded-xl border border-border bg-surface px-3 py-2.5">
      <div className="flex items-center gap-2 text-[12.5px] font-medium text-text">
        <span className={cn('size-1.5 rounded-full', data?.sources ? 'bg-success' : 'bg-faint')} />
        {data?.sources ? 'Library up to date' : 'No folders yet'}
      </div>
      <div className="mt-0.5 text-[12px] text-muted">
        {total.toLocaleString()} items · {data?.sources ?? 0} {data?.sources === 1 ? 'folder' : 'folders'}
      </div>
    </div>
  )
}

export function Sidebar() {
  const { view, go } = useNav()
  return (
    <aside className="flex w-[232px] shrink-0 flex-col border-r border-border bg-surface/60 px-3 pt-5 pb-3">
      <div className="px-2">
        <Logo />
      </div>

      <button
        type="button"
        className="mt-5 flex h-9 items-center gap-2.5 rounded-lg border border-border bg-bg px-3 text-[13px] text-faint transition-colors hover:border-faint/40 hover:text-muted"
      >
        <Search size={15} />
        <span className="flex-1 truncate text-left whitespace-nowrap">Search</span>
        <Kbd>Ctrl K</Kbd>
      </button>

      <nav className="mt-1 flex-1">
        <Section title="Library" items={library} />
        <Section title="Manage" items={manage} />
      </nav>

      <LibraryStatus />
      <button
        type="button"
        onClick={() => go('settings')}
        className={cn(
          'mt-2 flex h-9 items-center gap-3 rounded-lg px-3 text-[13.5px] font-medium transition-colors',
          view === 'settings' ? 'bg-elevated text-text' : 'text-muted hover:bg-hover hover:text-text',
        )}
      >
        <Settings size={17} strokeWidth={1.9} />
        Settings
      </button>
    </aside>
  )
}
