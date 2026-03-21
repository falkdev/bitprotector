import { create } from 'zustand'
import { statusApi } from '@/api/status'
import type { SystemStatus } from '@/types/status'

interface StatusState {
  status: SystemStatus | null
  loading: boolean
  error: string | null

  fetch(): Promise<void>
}

export const useStatusStore = create<StatusState>((set) => ({
  status: null,
  loading: false,
  error: null,

  async fetch() {
    set({ loading: true, error: null })
    try {
      const status = await statusApi.get()
      set({ status, loading: false })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },
}))
