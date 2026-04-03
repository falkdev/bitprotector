import { apiClient } from './client'
import { isAxiosError } from 'axios'
import type {
  SetVirtualPathRequest,
  BulkAssignRequest,
  BulkFromRealRequest,
  BulkAssignResult,
  RefreshSymlinksResult,
  VirtualPathTreeResponse,
} from '@/types/virtual-path'

export const virtualPathsApi = {
  set(fileId: number, data: SetVirtualPathRequest): Promise<string> {
    return apiClient.put<string>(`/virtual-paths/${fileId}`, data).then((r) => r.data)
  },

  remove(fileId: number): Promise<string> {
    return apiClient.delete<string>(`/virtual-paths/${fileId}`).then((r) => r.data)
  },

  refresh(): Promise<RefreshSymlinksResult> {
    return apiClient.post<RefreshSymlinksResult>('/virtual-paths/refresh').then((r) => r.data)
  },

  async tree(parent?: string): Promise<VirtualPathTreeResponse> {
    try {
      return await apiClient
        .get<VirtualPathTreeResponse>('/virtual-paths/tree', {
          params: parent ? { parent } : undefined,
        })
        .then((r) => r.data)
    } catch (error) {
      if (isAxiosError(error) && error.response?.status === 404) {
        return {
          parent: parent || '/',
          children: [],
        }
      }
      throw error
    }
  },

  bulk(data: BulkAssignRequest): Promise<BulkAssignResult> {
    return apiClient.post<BulkAssignResult>('/virtual-paths/bulk', data).then((r) => r.data)
  },

  bulkFromReal(data: BulkFromRealRequest): Promise<BulkAssignResult> {
    return apiClient.post<BulkAssignResult>('/virtual-paths/bulk-from-real', data).then((r) => r.data)
  },
}
