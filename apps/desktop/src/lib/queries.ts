import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { toast } from '../state/toast'
import { api, CommandError, type Id, pickFolder } from './api'

export const keys = {
  appInfo: ['appInfo'] as const,
  stats: ['stats'] as const,
  sources: ['sources'] as const,
}

export const useAppInfo = () => useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo, staleTime: Infinity })
export const useStats = () => useQuery({ queryKey: keys.stats, queryFn: api.libraryStats })
export const useSources = () => useQuery({ queryKey: keys.sources, queryFn: api.listSources })

function useInvalidateLibrary() {
  const qc = useQueryClient()
  return () => {
    qc.invalidateQueries({ queryKey: keys.sources })
    qc.invalidateQueries({ queryKey: keys.stats })
  }
}

/** Opens the folder picker and adds the chosen folder. */
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
      toast.success('Folder added', source.path)
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
