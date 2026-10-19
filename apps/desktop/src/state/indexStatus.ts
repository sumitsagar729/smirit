import { create } from 'zustand'

import type { IndexStatus } from '../lib/bindings'

export const useIndexStore = create<{ status: IndexStatus }>(() => ({
  status: { phase: 'idle', total: 0, done: 0, failed: 0, generation: 0 },
}))
