import { create } from 'zustand'

export type Toast = { id: number; tone: 'info' | 'error' | 'success'; title: string; body?: string }

type ToastState = {
  toasts: Toast[]
  push: (t: Omit<Toast, 'id'>) => void
  dismiss: (id: number) => void
}

let next = 1

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (t) => {
    const id = next++
    set({ toasts: [...get().toasts, { ...t, id }] })
    setTimeout(() => get().dismiss(id), t.tone === 'error' ? 8000 : 4000)
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}))

export const toast = {
  error: (title: string, body?: string) => useToasts.getState().push({ tone: 'error', title, body }),
  success: (title: string, body?: string) => useToasts.getState().push({ tone: 'success', title, body }),
  info: (title: string, body?: string) => useToasts.getState().push({ tone: 'info', title, body }),
}
