import { create } from 'zustand'

export type View = 'timeline' | 'people' | 'places' | 'albums' | 'folders' | 'settings'

type NavState = {
  view: View
  go: (view: View) => void
}

export const useNav = create<NavState>((set) => ({
  view: 'timeline',
  go: (view) => set({ view }),
}))
