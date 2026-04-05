import { apiClient } from './client'
import type {
  TrackedFolder,
  CreateFolderRequest,
  UpdateFolderRequest,
  ScanFolderResult,
  MirrorFolderResult,
} from '@/types/folder'

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

  update(id: number, data: UpdateFolderRequest): Promise<TrackedFolder> {
    return apiClient.put<TrackedFolder>(`/folders/${id}`, data).then((r) => r.data)
  },

  delete(id: number): Promise<void> {
    return apiClient.delete(`/folders/${id}`).then(() => undefined)
  },

  scan(id: number): Promise<ScanFolderResult> {
    return apiClient.post<ScanFolderResult>(`/folders/${id}/scan`).then((r) => r.data)
  },

  mirror(id: number): Promise<MirrorFolderResult> {
    return apiClient.post<MirrorFolderResult>(`/folders/${id}/mirror`).then((r) => r.data)
  },
}
