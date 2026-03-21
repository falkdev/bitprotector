import { apiClient } from './client'
import type {
  SetVirtualPathRequest,
  BulkAssignRequest,
  BulkFromRealRequest,
  BulkAssignResult,
  RefreshSymlinksResult,
} from '@/types/virtual-path'

export const virtualPathsApi = {
  set(fileId: number, data: SetVirtualPathRequest): Promise<string> {
    return apiClient.put<string>(`/virtual-paths/${fileId}`, data).then((r) => r.data)
  },

  remove(fileId: number, symlinkBase?: string): Promise<string> {
    return apiClient
      .delete<string>(`/virtual-paths/${fileId}`, { params: { symlink_base: symlinkBase } })
      .then((r) => r.data)
  },

  refresh(): Promise<RefreshSymlinksResult> {
    return apiClient.post<RefreshSymlinksResult>('/virtual-paths/refresh').then((r) => r.data)
  },

  bulk(data: BulkAssignRequest): Promise<BulkAssignResult> {
    return apiClient.post<BulkAssignResult>('/virtual-paths/bulk', data).then((r) => r.data)
  },

  bulkFromReal(data: BulkFromRealRequest): Promise<BulkAssignResult> {
    return apiClient.post<BulkAssignResult>('/virtual-paths/bulk-from-real', data).then((r) => r.data)
  },
}
