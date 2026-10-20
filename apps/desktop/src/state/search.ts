import { create } from 'zustand'

import type { Chip } from '../lib/bindings'
import { sameChip } from '../lib/search'

/** The search bar's chips; kept while moving around the app. */
type SearchState = {
  chips: Chip[]
  add: (c: Chip) => void
  remove: (i: number) => void
  /** Replace everything, e.g. "photos from this trip". */
  set: (chips: Chip[]) => void
  clear: () => void
}

export const useSearch = create<SearchState>((set, get) => ({
  chips: [],
  add: (c) => {
    if (get().chips.some((x) => sameChip(x, c))) return
    set({ chips: [...get().chips, c] })
  },
  remove: (i) => set({ chips: get().chips.filter((_, j) => j !== i) }),
  set: (chips) => set({ chips }),
  clear: () => set({ chips: [] }),
}))
