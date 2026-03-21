import { apiClient } from './client'
import type { EventLogEntry, LogsQueryParams } from '@/types/log'

export const logsApi = {
  list(params?: LogsQueryParams): Promise<EventLogEntry[]> {
    const searchParams: Record<string, unknown> = { ...params }
    return apiClient.get<EventLogEntry[]>('/logs', { params: searchParams }).then((r) => r.data)
  },

  get(id: number): Promise<EventLogEntry> {
    return apiClient.get<EventLogEntry>(`/logs/${id}`).then((r) => r.data)
  },
}
