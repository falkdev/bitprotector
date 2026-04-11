import { apiClient } from './client'
import { isAxiosError } from 'axios'
import type { TrackedFile } from '@/types/file'
import type { TrackedFolder } from '@/types/folder'
import type { TrackingItem, TrackingListParams, TrackingListResponse } from '@/types/tracking'

function normalizeFolderPath(path: string): string {
  return path.replace(/\/+$/, '')
}

function isPathInFolder(relativePath: string, folderPath: string): boolean {
  const normalizedFolder = normalizeFolderPath(folderPath)
  return relativePath === normalizedFolder || relativePath.startsWith(`${normalizedFolder}/`)
}

function deriveFileVirtualPath(file: TrackedFile, folders: TrackedFolder[]): string | null {
  if (file.virtual_path) return file.virtual_path
  const matchingFolders = folders
    .filter(
      (folder) =>
        folder.drive_pair_id === file.drive_pair_id &&
        !!folder.virtual_path &&
        isPathInFolder(file.relative_path, folder.folder_path)
    )
    .sort((a, b) => normalizeFolderPath(b.folder_path).length - normalizeFolderPath(a.folder_path).length)

  const folder = matchingFolders[0]
  if (!folder?.virtual_path) return null
  const normalizedFolderPath = normalizeFolderPath(folder.folder_path)
  if (file.relative_path === normalizedFolderPath) return folder.virtual_path
  const suffix = file.relative_path.slice(normalizedFolderPath.length + 1)
  return `${folder.virtual_path.replace(/\/+$/, '')}/${suffix}`
}

function fileSource(file: TrackedFile): 'direct' | 'folder' {
  const direct = file.tracked_direct ?? true
  const viaFolder = file.tracked_via_folder ?? false
  if (direct) return 'direct'
  if (viaFolder) return 'folder'
  return 'direct'
}

function toFileItem(file: TrackedFile, folders: TrackedFolder[]): TrackingItem {
  return {
    kind: 'file',
    id: file.id,
    drive_pair_id: file.drive_pair_id,
    path: file.relative_path,
    virtual_path: deriveFileVirtualPath(file, folders),
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
    folder_status: 'not_scanned',
    folder_total_files: 0,
    folder_mirrored_files: 0,
    created_at: folder.created_at,
    updated_at: folder.created_at,
  }
}

function includeBySource(item: TrackingItem, source: TrackingListParams['source']) {
  if (!source || source === 'all') return true
  if (item.kind === 'folder') return source === 'folder'
  return item.source === source
}

function includeByHasVirtualPath(item: TrackingItem, hasVirtualPath: boolean | undefined) {
  if (hasVirtualPath == null) return true
  return hasVirtualPath ? !!item.virtual_path : !item.virtual_path
}

function includeByVirtualPrefix(item: TrackingItem, virtualPrefix: string | undefined) {
  if (!virtualPrefix) return true
  return (item.virtual_path ?? '').startsWith(virtualPrefix)
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
        page,
        per_page: perPage,
      },
    }),
    apiClient.get<TrackedFolder[]>('/folders'),
  ])

  const folderRows = foldersResponse.data
  const files = filesResponse.data.files.map((file) => toFileItem(file, folderRows))

  const folderStats = new Map<number, { total: number; mirrored: number }>()
  for (const folder of folderRows) {
    folderStats.set(folder.id, { total: 0, mirrored: 0 })
  }
  for (const file of files) {
    for (const folder of folderRows) {
      if (folder.drive_pair_id !== file.drive_pair_id) continue
      if (!isPathInFolder(file.path, folder.folder_path)) continue
      const stats = folderStats.get(folder.id)
      if (!stats) continue
      stats.total += 1
      if (file.is_mirrored) stats.mirrored += 1
    }
  }

  const foldersWithStatus: TrackingItem[] = folderRows.map((folderRow) => {
    const folder = toFolderItem(folderRow)
    const stats = folderStats.get(folder.id) ?? { total: 0, mirrored: 0 }
    const folderStatus =
      stats.total === 0
        ? folderRow.last_scanned_at
          ? 'empty'
          : 'not_scanned'
        : stats.mirrored === stats.total
          ? 'mirrored'
          : stats.mirrored === 0
            ? 'tracked'
            : 'partial'

    return {
      ...folder,
      folder_status: folderStatus,
      folder_total_files: stats.total,
      folder_mirrored_files: stats.mirrored,
    }
  })

  const itemKind = params?.item_kind ?? 'all'
  let items = [...files, ...foldersWithStatus].filter((item) => {
    if (params?.drive_id != null && item.drive_pair_id !== params.drive_id) return false
    if (itemKind !== 'all' && item.kind !== itemKind) return false
    if (!includeBySource(item, params?.source)) return false
    if (!includeByHasVirtualPath(item, params?.has_virtual_path)) return false
    if (!includeByVirtualPrefix(item, params?.virtual_prefix)) return false
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
