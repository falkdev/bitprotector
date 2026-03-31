import { apiClient } from './client'
import type { EventLogEntry, LogsQueryParams } from '@/types/log'

interface LogsListResponse {
  logs: EventLogEntry[]
  total: number
  page: number
  per_page: number
}

export const logsApi = {
  list(params?: LogsQueryParams): Promise<EventLogEntry[]> {
    const searchParams: Record<string, unknown> = { ...params }
    return apiClient
      .get<EventLogEntry[] | LogsListResponse>('/logs', { params: searchParams })
      .then((r) => (Array.isArray(r.data) ? r.data : r.data.logs))
  },

  get(id: number): Promise<EventLogEntry> {
    return apiClient.get<EventLogEntry>(`/logs/${id}`).then((r) => r.data)
  },
}
