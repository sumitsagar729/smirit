import { Monitor, Moon, ShieldCheck, Sun } from 'lucide-react'
import { useState } from 'react'

import { PageHeader } from '../components/PageHeader'
import { cn } from '../lib/cn'
import { useAppInfo, useStats } from '../lib/queries'
import { getThemePref, setThemePref, type ThemePref } from '../lib/theme'

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-2xl border border-border bg-surface p-6">
      <h2 className="mb-4 text-[13px] font-semibold tracking-[0.08em] text-faint uppercase">{title}</h2>
      {children}
    </section>
  )
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-6 border-b border-border/60 py-2.5 last:border-0">
      <span className="text-[13.5px] text-muted">{label}</span>
      <span className="selectable text-right text-[13.5px] break-all text-text">{value}</span>
    </div>
  )
}

const themes: { value: ThemePref; label: string; icon: typeof Sun }[] = [
  { value: 'system', label: 'System', icon: Monitor },
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'light', label: 'Light', icon: Sun },
]

function ThemePicker() {
  const [pref, setPref] = useState(getThemePref)
  return (
    <div role="radiogroup" aria-label="Theme" className="inline-flex rounded-xl border border-border bg-bg p-1">
      {themes.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={pref === value}
          onClick={() => {
            setThemePref(value)
            setPref(value)
          }}
          className={cn(
            'flex h-8 items-center gap-2 rounded-lg px-3.5 text-[13px] font-medium transition-colors',
            pref === value ? 'bg-elevated text-text shadow-panel' : 'text-muted hover:text-text',
          )}
        >
          <Icon size={15} />
          {label}
        </button>
      ))}
    </div>
  )
}

export function SettingsView() {
  const { data: info } = useAppInfo()
  const { data: stats } = useStats()
  return (
    <div className="h-full overflow-y-auto">
      <PageHeader title="Settings" />
      <div className="grid max-w-3xl gap-5 px-10 pb-10">
        <Card title="Appearance">
          <ThemePicker />
        </Card>
        <Card title="Library">
          <Row label="Location" value={stats?.root ?? '—'} />
          <Row label="Folders" value={stats?.sources ?? 0} />
          <Row label="Photos and videos" value={((stats?.photos ?? 0) + (stats?.videos ?? 0)).toLocaleString()} />
          <Row label="Database version" value={stats?.schemaVersion ?? '—'} />
        </Card>
        <Card title="Privacy">
          <div className="flex gap-3 text-[13.5px] leading-relaxed text-muted">
            <ShieldCheck size={20} className="mt-0.5 shrink-0 text-success" />
            <p>
              Everything Smriti learns, including faces, names and places, stays in the library folder on this
              computer. No account, no cloud, no tracking.
            </p>
          </div>
        </Card>
        <Card title="About">
          <Row label="Version" value={info ? `${info.version}` : '—'} />
          <Row label="Platform" value={info ? `${info.os} ${info.arch}`.trim() : '—'} />
        </Card>
      </div>
    </div>
  )
}
