import { apiClient } from './client'
import type { TrackedFolder, CreateFolderRequest, ScanFolderResult } from '@/types/folder'

export const foldersApi = {
  list(): Promise<TrackedFolder[]> {
    return apiClient.get<TrackedFolder[]>('/folders').then((r) => r.data)
  },

  get(id: number): Promise<TrackedFolder> {
    return apiClient.get<TrackedFolder>(`/folders/${id}`).then((r) => r.data)
  },

  create(data: CreateFolderRequest): Promise<TrackedFolder> {
    return apiClient.post<TrackedFolder>('/folders', data).then((r) => r.data)
  },

  delete(id: number): Promise<void> {
    return apiClient.delete(`/folders/${id}`).then(() => undefined)
  },

  scan(id: number): Promise<ScanFolderResult> {
    return apiClient.post<ScanFolderResult>(`/folders/${id}/scan`).then((r) => r.data)
  },
}
