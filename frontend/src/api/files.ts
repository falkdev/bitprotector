import { apiClient } from './client'
import type {
  TrackedFile,
  TrackedFileListResponse,
  TrackFileRequest,
  FilesQueryParams,
} from '@/types/file'

export const filesApi = {
  list(params?: FilesQueryParams): Promise<TrackedFileListResponse> {
    return apiClient.get<TrackedFileListResponse>('/files', { params }).then((r) => r.data)
  },

  get(id: number): Promise<TrackedFile> {
    return apiClient.get<TrackedFile>(`/files/${id}`).then((r) => r.data)
  },

  track(data: TrackFileRequest): Promise<TrackedFile> {
    return apiClient.post<TrackedFile>('/files', data).then((r) => r.data)
  },

  delete(id: number): Promise<void> {
    return apiClient.delete(`/files/${id}`).then(() => undefined)
  },

  mirror(id: number): Promise<TrackedFile> {
    return apiClient.post<TrackedFile>(`/files/${id}/mirror`).then((r) => r.data)
  },
}
