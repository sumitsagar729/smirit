import { create } from 'zustand'

import type { Id } from '../lib/bindings'

export type View = 'timeline' | 'people' | 'person' | 'places' | 'albums' | 'folders' | 'settings'

type NavState = {
  view: View
  /** The person shown when `view` is 'person'. */
  personId: Id | null
  go: (view: View) => void
  openPerson: (id: Id) => void
}

export const useNav = create<NavState>((set) => ({
  view: 'timeline',
  personId: null,
  go: (view) => set({ view }),
  openPerson: (personId) => set({ view: 'person', personId }),
}))
