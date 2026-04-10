import { apiClient } from './client'
import type {
  SyncQueueItem,
  AddQueueItemRequest,
  ResolveQueueItemRequest,
  ProcessQueueResult,
  ClearCompletedQueueResult,
} from '@/types/sync'

interface SyncQueueListResponse {
  queue: SyncQueueItem[]
  total: number
  page: number
  per_page: number
}

export const syncApi = {
  listQueue(): Promise<SyncQueueItem[]> {
    return apiClient
      .get<SyncQueueItem[] | SyncQueueListResponse>('/sync/queue')
      .then((r) => (Array.isArray(r.data) ? r.data : r.data.queue))
  },

  addQueueItem(data: AddQueueItemRequest): Promise<SyncQueueItem> {
    return apiClient.post<SyncQueueItem>('/sync/queue', data).then((r) => r.data)
  },

  getQueueItem(id: number): Promise<SyncQueueItem> {
    return apiClient.get<SyncQueueItem>(`/sync/queue/${id}`).then((r) => r.data)
  },

  resolveQueueItem(id: number, data: ResolveQueueItemRequest): Promise<SyncQueueItem> {
    return apiClient.post<SyncQueueItem>(`/sync/queue/${id}/resolve`, data).then((r) => r.data)
  },

  processQueue(): Promise<ProcessQueueResult> {
    return apiClient.post<ProcessQueueResult>('/sync/process').then((r) => r.data)
  },

  clearCompletedQueue(): Promise<ClearCompletedQueueResult> {
    return apiClient.delete<ClearCompletedQueueResult>('/sync/queue/completed').then((r) => r.data)
  },
}
