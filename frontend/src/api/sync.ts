import { apiClient } from './client'
import type {
  SyncQueueItem,
  AddQueueItemRequest,
  ResolveQueueItemRequest,
  SyncTask,
  ProcessQueueResult,
  RunTaskResult,
} from '@/types/sync'

export const syncApi = {
  listQueue(): Promise<SyncQueueItem[]> {
    return apiClient.get<SyncQueueItem[]>('/sync/queue').then((r) => r.data)
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

  runTask(task: SyncTask): Promise<RunTaskResult> {
    return apiClient.post<RunTaskResult>(`/sync/run/${task}`).then((r) => r.data)
  },
}
