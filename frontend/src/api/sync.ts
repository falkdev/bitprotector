import { apiClient } from './client'
import type {
  SyncQueueItem,
  AddQueueItemRequest,
  ResolveQueueItemRequest,
  ProcessQueueResult,
  ClearCompletedQueueResult,
  QueuePausedResult,
  SyncQueueListResponse,
} from '@/types/sync'

interface ListQueueParams {
  status?: string
  page?: number
  perPage?: number
}

export const syncApi = {
  listQueue(params: ListQueueParams = {}): Promise<SyncQueueListResponse> {
    return apiClient
      .get<SyncQueueListResponse>('/sync/queue', {
        params: {
          status: params.status,
          page: params.page,
          per_page: params.perPage,
        },
      })
      .then((r) => r.data)
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

  pauseQueue(): Promise<QueuePausedResult> {
    return apiClient.post<QueuePausedResult>('/sync/pause').then((r) => r.data)
  },

  resumeQueue(): Promise<QueuePausedResult> {
    return apiClient.post<QueuePausedResult>('/sync/resume').then((r) => r.data)
  },
}
