import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'

import { useIndexStore } from '../state/indexStatus'
import { toast } from '../state/toast'
import { api, CommandError, type Id, type IndexStatus, onIndexStatus, pickFolder } from './api'

export const keys = {
  appInfo: ['appInfo'] as const,
  stats: ['stats'] as const,
  sources: ['sources'] as const,
  timeline: ['timeline'] as const,
  asset: (id: Id) => ['asset', id] as const,
}

export const useAppInfo = () => useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo, staleTime: Infinity })
export const useStats = () => useQuery({ queryKey: keys.stats, queryFn: api.libraryStats })
export const useSources = () => useQuery({ queryKey: keys.sources, queryFn: api.listSources })
export const useTimeline = () =>
  useQuery({ queryKey: keys.timeline, queryFn: api.timeline, placeholderData: (prev) => prev, staleTime: 60_000 })
export const useAssetDetail = (id: Id | null) =>
  useQuery({ queryKey: keys.asset(id ?? -1), queryFn: () => api.assetDetail(id!), enabled: id != null })

/** Current indexing status (kept fresh by `useIndexStatusSync` in the app shell). */
export const useIndexStatus = () => useIndexStore((s) => s.status)

/**
 * Mount once: mirrors core indexing status into the store. While items are
 * being indexed, the timeline and counts refresh at most every 4 s so the
 * grid fills in without thrashing.
 */
export function useIndexStatusSync() {
  const qc = useQueryClient()
  const lastRefresh = useRef(0)
  const pending = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let alive = true
    const refresh = () => {
      lastRefresh.current = Date.now()
      qc.invalidateQueries({ queryKey: keys.timeline })
      qc.invalidateQueries({ queryKey: keys.stats })
      qc.invalidateQueries({ queryKey: keys.sources })
    }
    const onStatus = (s: IndexStatus) => {
      const prev = useIndexStore.getState().status
      useIndexStore.setState({ status: s })
      if (s.generation === prev.generation && !(prev.phase !== 'idle' && s.phase === 'idle')) return
      const wait = 4000 - (Date.now() - lastRefresh.current)
      if (s.phase === 'idle' || wait <= 0) refresh()
      else if (!pending.current) {
        pending.current = setTimeout(() => {
          pending.current = null
          refresh()
        }, wait)
      }
    }
    api.indexStatus().then((s) => alive && onStatus(s))
    onIndexStatus(onStatus).then((u) => (alive ? (unlisten = u) : u()))
    return () => {
      alive = false
      unlisten?.()
      if (pending.current) clearTimeout(pending.current)
    }
  }, [qc])
}

function useInvalidateLibrary() {
  const qc = useQueryClient()
  return () => {
    qc.invalidateQueries({ queryKey: keys.sources })
    qc.invalidateQueries({ queryKey: keys.stats })
    qc.invalidateQueries({ queryKey: keys.timeline })
  }
}

/** Opens the folder picker and adds the chosen folder; scanning starts automatically. */
export function useAddFolder() {
  const invalidate = useInvalidateLibrary()
  return useMutation({
    mutationFn: async () => {
      const path = await pickFolder()
      return path ? api.addSource(path) : null
    },
    onSuccess: (source) => {
      if (!source) return
      invalidate()
      toast.success('Folder added', 'Smriti is looking for photos and videos in it now.')
    },
    onError: (e) => toast.error("Couldn't add that folder", e instanceof CommandError ? e.message : String(e)),
  })
}

export function useRemoveFolder() {
  const invalidate = useInvalidateLibrary()
  return useMutation({
    mutationFn: (id: Id) => api.removeSource(id),
    onSuccess: () => {
      invalidate()
      toast.info('Folder removed', 'Your files were not touched.')
    },
    onError: (e) => toast.error("Couldn't remove that folder", e instanceof Error ? e.message : String(e)),
  })
}

export function useRescan() {
  return useMutation({
    mutationFn: () => api.rescan(),
    onSuccess: () => toast.info('Checking your folders', 'New and changed files will appear shortly.'),
  })
}
