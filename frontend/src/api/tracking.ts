import { apiClient } from './client'
import { isAxiosError } from 'axios'
import type { TrackedFile } from '@/types/file'
import type { TrackedFolder } from '@/types/folder'
import type { TrackingItem, TrackingListParams, TrackingListResponse } from '@/types/tracking'

function fileSource(file: TrackedFile): 'direct' | 'folder' | 'both' {
  const direct = file.tracked_direct ?? true
  const viaFolder = file.tracked_via_folder ?? false
  if (direct && viaFolder) return 'both'
  if (viaFolder) return 'folder'
  return 'direct'
}

function toFileItem(file: TrackedFile): TrackingItem {
  return {
    kind: 'file',
    id: file.id,
    drive_pair_id: file.drive_pair_id,
    path: file.relative_path,
    virtual_path: file.virtual_path,
    is_mirrored: file.is_mirrored,
    tracked_direct: file.tracked_direct ?? true,
    tracked_via_folder: file.tracked_via_folder ?? false,
    source: fileSource(file),
    created_at: file.created_at,
    updated_at: file.updated_at,
  }
}

function toFolderItem(folder: TrackedFolder): TrackingItem {
  return {
    kind: 'folder',
    id: folder.id,
    drive_pair_id: folder.drive_pair_id,
    path: folder.folder_path,
    virtual_path: folder.virtual_path,
    is_mirrored: null,
    tracked_direct: null,
    tracked_via_folder: null,
    source: 'folder',
    created_at: folder.created_at,
    updated_at: folder.created_at,
  }
}

function includeBySource(item: TrackingItem, source: TrackingListParams['source']) {
  if (!source || source === 'all') return true
  if (item.kind === 'folder') return source === 'folder'
  return item.source === source
}

function includeByPublished(item: TrackingItem, published: boolean | undefined) {
  if (published == null) return true
  return published ? !!item.virtual_path : !item.virtual_path
}

function includeByPublishPrefix(item: TrackingItem, publishPrefix: string | undefined) {
  if (!publishPrefix) return true
  return (item.virtual_path ?? '').startsWith(publishPrefix)
}

function includeByQuery(item: TrackingItem, query: string | undefined) {
  if (!query) return true
  const q = query.toLowerCase()
  return item.path.toLowerCase().includes(q) || (item.virtual_path ?? '').toLowerCase().includes(q)
}

async function listFallback(params?: TrackingListParams): Promise<TrackingListResponse> {
  const page = Math.max(params?.page ?? 1, 1)
  const perPage = Math.min(Math.max(params?.per_page ?? 50, 1), 200)

  const [filesResponse, foldersResponse] = await Promise.all([
    apiClient.get<{ files: TrackedFile[] }>('/files', {
      params: {
        drive_id: params?.drive_id,
        virtual_prefix: params?.publish_prefix,
        page,
        per_page: perPage,
      },
    }),
    apiClient.get<TrackedFolder[]>('/folders'),
  ])

  const files = filesResponse.data.files.map(toFileItem)
  const folders = foldersResponse.data.map(toFolderItem)

  const itemKind = params?.item_kind ?? 'all'
  let items = [...files, ...folders].filter((item) => {
    if (params?.drive_id != null && item.drive_pair_id !== params.drive_id) return false
    if (itemKind !== 'all' && item.kind !== itemKind) return false
    if (!includeBySource(item, params?.source)) return false
    if (!includeByPublished(item, params?.published)) return false
    if (!includeByPublishPrefix(item, params?.publish_prefix)) return false
    if (!includeByQuery(item, params?.q)) return false
    return true
  })

  items = items.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind.localeCompare(b.kind)
    return a.id - b.id
  })

  const total = items.length
  const offset = (page - 1) * perPage
  return {
    items: items.slice(offset, offset + perPage),
    total,
    page,
    per_page: perPage,
  }
}

export const trackingApi = {
  async list(params?: TrackingListParams): Promise<TrackingListResponse> {
    try {
      return await apiClient
      .get<TrackingListResponse>('/tracking/items', { params })
      .then((response) => response.data)
    } catch (error) {
      if (isAxiosError(error) && error.response?.status === 404) {
        return listFallback(params)
      }
      throw error
    }
  },
}
