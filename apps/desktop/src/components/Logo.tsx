/** Smriti mark: a diya flame inside an aperture ring. */
export function LogoMark({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
      <defs>
        <linearGradient id="smriti-flame" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#F6C66B" />
          <stop offset="1" stopColor="#E0862B" />
        </linearGradient>
      </defs>
      <circle cx="16" cy="16" r="14.5" fill="none" stroke="var(--accent)" strokeOpacity="0.35" strokeWidth="1.5" />
      <path d="M16 6.5c3.6 4.2 5.4 7.4 5.4 10.1A5.4 5.4 0 0 1 16 22a5.4 5.4 0 0 1-5.4-5.4c0-2.7 1.8-5.9 5.4-10.1Z" fill="url(#smriti-flame)" />
      <path d="M9 24.5h14" stroke="var(--accent)" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  )
}

export function Logo() {
  return (
    <div className="flex items-center gap-2.5">
      <LogoMark />
      <span className="font-display text-[21px] leading-none font-semibold tracking-tight text-text">Smriti</span>
    </div>
  )
}
