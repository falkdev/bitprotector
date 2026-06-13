import { create } from 'zustand'
import { syncApi } from '@/api/sync'
import type { SyncQueueItem, SyncStatus } from '@/types/sync'

interface SyncStore {
  items: SyncQueueItem[]
  queuePaused: boolean
  activeItems: number
  inProgressItems: number
  loading: boolean
  error: string | null
  filter: SyncStatus | 'all'
  page: number
  perPage: number
  total: number

  fetch(): Promise<void>
  setFilter(filter: SyncStatus | 'all'): Promise<void>
  setPage(page: number): Promise<void>
  refreshItem(item: SyncQueueItem): void
}

export const useSyncStore = create<SyncStore>((set, get) => ({
  items: [],
  queuePaused: false,
  activeItems: 0,
  inProgressItems: 0,
  loading: false,
  error: null,
  filter: 'all',
  page: 1,
  perPage: 50,
  total: 0,

  async fetch() {
    set({ loading: true, error: null })
    try {
      const { filter, page, perPage } = get()
      const response = await syncApi.listQueue({
        status: filter === 'all' ? undefined : filter,
        page,
        perPage,
      })
      set({
        items: response.queue,
        total: response.total,
        page: response.page,
        perPage: response.per_page,
        queuePaused: response.queue_paused,
        activeItems: response.active_items,
        inProgressItems: response.in_progress_items,
        loading: false,
      })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },

  async setFilter(filter) {
    set({ filter, page: 1 })
    await get().fetch()
  },

  async setPage(page) {
    set({ page })
    await get().fetch()
  },

  refreshItem(item) {
    set((s) => {
      const exists = s.items.find((i) => i.id === item.id)
      if (exists) {
        return { items: s.items.map((i) => (i.id === item.id ? item : i)) }
      }
      return { items: [item, ...s.items] }
    })
  },
}))
