import { create } from 'zustand'
import { logsApi } from '@/api/logs'
import type { EventLogEntry, LogsQueryParams } from '@/types/log'

interface LogsState {
  entries: EventLogEntry[]
  loading: boolean
  error: string | null
  params: LogsQueryParams

  fetch(params?: LogsQueryParams): Promise<void>
  setParams(params: LogsQueryParams): void
}

export const useLogsStore = create<LogsState>((set, get) => ({
  entries: [],
  loading: false,
  error: null,
  params: { per_page: 50 },

  async fetch(params) {
    const merged = { ...get().params, ...params }
    set({ loading: true, error: null, params: merged })
    try {
      const entries = await logsApi.list(merged)
      set({ entries, loading: false })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },

  setParams(params) {
    set((s) => ({ params: { ...s.params, ...params } }))
  },
}))
