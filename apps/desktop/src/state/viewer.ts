import { create } from 'zustand'

import type { Tile } from '../lib/bindings'

type ViewerState = {
  /** The list being browsed (e.g. the timeline) and the open position in it. */
  items: Tile[]
  index: number | null
  open: (items: Tile[], index: number) => void
  close: () => void
  step: (delta: number) => void
}

export const useViewer = create<ViewerState>((set, get) => ({
  items: [],
  index: null,
  open: (items, index) => set({ items, index }),
  close: () => set({ index: null }),
  step: (delta) => {
    const { index, items } = get()
    if (index == null) return
    const next = index + delta
    if (next >= 0 && next < items.length) set({ index: next })
  },
}))
