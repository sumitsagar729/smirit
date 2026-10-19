export type ThemePref = 'system' | 'dark' | 'light'

const KEY = 'smriti.theme'

export function getThemePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY)
    if (v === 'dark' || v === 'light' || v === 'system') return v
  } catch {
    // storage unavailable: fall through to the default
  }
  return 'system'
}

export function setThemePref(pref: ThemePref) {
  try {
    localStorage.setItem(KEY, pref)
  } catch {
    // not fatal; the choice just won't persist
  }
  applyTheme()
}

/** Resolves the preference against the OS and sets html[data-theme]. */
export function applyTheme() {
  const pref = getThemePref()
  const media = window.matchMedia?.('(prefers-color-scheme: light)')
  const resolved = pref === 'system' ? (media?.matches ? 'light' : 'dark') : pref
  document.documentElement.dataset.theme = resolved
  if (media && !media.onchange) media.onchange = () => applyTheme()
}
