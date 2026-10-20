export function formatDuration(sec: number) {
  const s = Math.round(sec)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const ss = String(s % 60).padStart(2, '0')
  return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`
}

export function formatBytes(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`
  return `${Math.round(n / 1e3)} KB`
}

export function formatLongDate(iso: string | null) {
  if (!iso) return 'Date unknown'
  return new Date(iso).toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })
}

export function formatTime(iso: string | null) {
  return iso ? new Date(iso).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }) : ''
}

/** "20–23 Nov 2017", "30 Oct – 2 Nov 2021" or "28 Dec 2021 – 2 Jan 2022", from `YYYY-MM-DD` dates. */
export function formatTripDates(start: string, end: string) {
  const s = new Date(`${start}T12:00:00`)
  const e = new Date(`${end}T12:00:00`)
  const month = (d: Date) => d.toLocaleDateString(undefined, { month: 'short' })
  if (start === end) return `${s.getDate()} ${month(s)} ${s.getFullYear()}`
  if (s.getFullYear() !== e.getFullYear()) {
    return `${s.getDate()} ${month(s)} ${s.getFullYear()} – ${e.getDate()} ${month(e)} ${e.getFullYear()}`
  }
  if (s.getMonth() !== e.getMonth()) return `${s.getDate()} ${month(s)} – ${e.getDate()} ${month(e)} ${e.getFullYear()}`
  return `${s.getDate()}–${e.getDate()} ${month(e)} ${e.getFullYear()}`
}
