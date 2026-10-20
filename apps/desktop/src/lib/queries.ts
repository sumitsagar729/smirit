import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'

import { useIndexStore } from '../state/indexStatus'
import { toast } from '../state/toast'
import {
  api,
  CommandError,
  type Id,
  type IndexStatus,
  onIndexStatus,
  type PersonRef,
  type PersonUpdate,
  pickFolder,
  type TimelineFilter,
} from './api'

export const keys = {
  appInfo: ['appInfo'] as const,
  stats: ['stats'] as const,
  sources: ['sources'] as const,
  timeline: ['timeline'] as const,
  filteredTimeline: (f: TimelineFilter | null) => ['timeline', f] as const,
  asset: (id: Id) => ['asset', id] as const,
  /** Everything that changes when faces are named or recognised. */
  people: ['people'] as const,
  personList: ['people', 'list'] as const,
  person: (id: Id) => ['people', 'person', id] as const,
  personFaces: (id: Id, suggestions: boolean) => ['people', 'faces', id, suggestions] as const,
  groups: ['people', 'groups'] as const,
  groupFaces: (id: Id) => ['people', 'group', id] as const,
  assetFaces: (id: Id) => ['people', 'asset', id] as const,
}

const noFilter = (f: TimelineFilter | null | undefined) => !f || f.people.length === 0

export const useAppInfo = () => useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo, staleTime: Infinity })
export const useStats = () => useQuery({ queryKey: keys.stats, queryFn: api.libraryStats })
export const useSources = () => useQuery({ queryKey: keys.sources, queryFn: api.listSources })
export const useTimeline = (filter?: TimelineFilter | null) => {
  const f = noFilter(filter) ? null : filter!
  return useQuery({
    queryKey: keys.filteredTimeline(f),
    queryFn: () => api.timeline(f),
    placeholderData: (prev) => prev,
    staleTime: 60_000,
  })
}
export const useAssetDetail = (id: Id | null) =>
  useQuery({ queryKey: keys.asset(id ?? -1), queryFn: () => api.assetDetail(id!), enabled: id != null })

export const usePeople = () => useQuery({ queryKey: keys.personList, queryFn: api.people, staleTime: 30_000 })
export const usePerson = (id: Id) => useQuery({ queryKey: keys.person(id), queryFn: () => api.person(id) })
export const useFaceGroups = (limit = 60) =>
  useQuery({ queryKey: [...keys.groups, limit], queryFn: () => api.faceGroups(limit), staleTime: 30_000 })
export const useGroupFaces = (id: Id | null) =>
  useQuery({ queryKey: keys.groupFaces(id ?? -1), queryFn: () => api.groupFaces(id!), enabled: id != null })
export const usePersonFaces = (id: Id, suggestions: boolean) =>
  useQuery({ queryKey: keys.personFaces(id, suggestions), queryFn: () => api.personFaces(id, suggestions) })
export const useAssetFaces = (id: Id | null) =>
  useQuery({ queryKey: keys.assetFaces(id ?? -1), queryFn: () => api.assetFaces(id!), enabled: id != null })

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
      if (s.peopleGeneration !== prev.peopleGeneration) {
        qc.invalidateQueries({ queryKey: keys.people })
        // Filtered timelines depend on who was recognised.
        qc.invalidateQueries({ queryKey: keys.timeline, predicate: (q) => q.queryKey[1] != null })
      }
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
    qc.invalidateQueries({ queryKey: keys.people })
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

function errorText(e: unknown) {
  return e instanceof CommandError || e instanceof Error ? e.message : String(e)
}

/** People mutations refresh people screens at once; recognition then refines them in the background. */
function usePeopleMutation<V, R>(fn: (v: V) => Promise<R>, failure: string, onDone?: (r: R, v: V) => void) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSuccess: (r, v) => {
      qc.invalidateQueries({ queryKey: keys.people })
      qc.invalidateQueries({ queryKey: keys.stats })
      onDone?.(r, v)
    },
    onError: (e) => toast.error(failure, errorText(e)),
  })
}

export const useNameGroup = () =>
  usePeopleMutation(
    ({ group, to }: { group: Id; to: PersonRef }) => api.nameGroup(group, to),
    "Couldn't name those faces",
  )
export const useAssignFaces = () =>
  usePeopleMutation(
    ({ faces, to }: { faces: Id[]; to: PersonRef }) => api.assignFaces(faces, to),
    "Couldn't name those faces",
  )
export const useRejectFaces = () => usePeopleMutation((faces: Id[]) => api.rejectFaces(faces), "Couldn't update those faces")
export const useIgnoreFaces = () => usePeopleMutation((faces: Id[]) => api.ignoreFaces(faces), "Couldn't hide those faces")
export const useIgnoreGroup = () =>
  usePeopleMutation((group: Id) => api.ignoreGroup(group), "Couldn't hide that group", () =>
    toast.info('Group hidden', 'These faces won’t be suggested again.'),
  )
export const useUpdatePerson = () =>
  usePeopleMutation(
    ({ id, update }: { id: Id; update: PersonUpdate }) => api.updatePerson(id, update),
    "Couldn't save those details",
  )
export const useMergePeople = () =>
  usePeopleMutation(({ from, into }: { from: Id; into: Id }) => api.mergePeople(from, into), "Couldn't merge them")
export const useDeletePerson = () => usePeopleMutation((id: Id) => api.deletePerson(id), "Couldn't remove that person")
export const useSetCoverFace = () =>
  usePeopleMutation(({ person, face }: { person: Id; face: Id }) => api.setCoverFace(person, face), "Couldn't change the photo")

export function useRescan() {
  return useMutation({
    mutationFn: () => api.rescan(),
    onSuccess: () => toast.info('Checking your folders', 'New and changed files will appear shortly.'),
  })
}
