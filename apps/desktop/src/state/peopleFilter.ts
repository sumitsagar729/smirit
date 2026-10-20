import { create } from 'zustand'

import type { Id } from '../lib/bindings'

/** The timeline's "who is in it" filter. */
type PeopleFilterState = {
  people: Id[]
  only: boolean
  toggle: (id: Id) => void
  setOnly: (only: boolean) => void
  set: (people: Id[], only?: boolean) => void
  clear: () => void
}

export const usePeopleFilter = create<PeopleFilterState>((set) => ({
  people: [],
  only: false,
  toggle: (id) =>
    set((s) => {
      const people = s.people.includes(id) ? s.people.filter((p) => p !== id) : [...s.people, id]
      return { people, only: people.length ? s.only : false }
    }),
  setOnly: (only) => set({ only }),
  set: (people, only = false) => set({ people, only }),
  clear: () => set({ people: [], only: false }),
}))
