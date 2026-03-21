import { apiClient } from './client'
import type { SystemStatus } from '@/types/status'

export const statusApi = {
  get(): Promise<SystemStatus> {
    return apiClient.get<SystemStatus>('/status').then((r) => r.data)
  },
}
