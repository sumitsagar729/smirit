import { create } from 'zustand'

import type { Id } from '../lib/bindings'

export type View = 'timeline' | 'search' | 'people' | 'person' | 'places' | 'trip' | 'albums' | 'folders' | 'settings'

type NavState = {
  view: View
  /** The person shown when `view` is 'person'. */
  personId: Id | null
  /** The trip shown when `view` is 'trip'. */
  tripId: Id | null
  go: (view: View) => void
  openPerson: (id: Id) => void
  openTrip: (id: Id) => void
}

export const useNav = create<NavState>((set) => ({
  view: 'timeline',
  personId: null,
  tripId: null,
  go: (view) => set({ view }),
  openPerson: (personId) => set({ view: 'person', personId }),
  openTrip: (tripId) => set({ view: 'trip', tripId }),
}))
