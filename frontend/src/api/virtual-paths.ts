import { apiClient } from './client'
import { isAxiosError } from 'axios'
import type { SetVirtualPathRequest, VirtualPathTreeResponse } from '@/types/virtual-path'

export const virtualPathsApi = {
  set(fileId: number, data: SetVirtualPathRequest): Promise<string> {
    return apiClient.put<string>(`/virtual-paths/${fileId}`, data).then((r) => r.data)
  },

  remove(fileId: number): Promise<string> {
    return apiClient.delete<string>(`/virtual-paths/${fileId}`).then((r) => r.data)
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
}
