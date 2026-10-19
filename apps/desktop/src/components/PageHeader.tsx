import type { ReactNode } from 'react'

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="flex items-end justify-between gap-6 px-10 pt-9 pb-6">
      <div>
        <h1 className="font-display text-[34px] leading-none font-medium tracking-tight text-text">{title}</h1>
        {subtitle && <p className="mt-2.5 text-[14px] text-muted">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </header>
  )
}
