import { create } from 'zustand'
import { syncApi } from '@/api/sync'
import type { SyncQueueItem, SyncStatus } from '@/types/sync'

interface SyncStore {
  items: SyncQueueItem[]
  loading: boolean
  error: string | null
  filter: SyncStatus | 'all'

  fetch(): Promise<void>
  setFilter(filter: SyncStatus | 'all'): void
  refreshItem(item: SyncQueueItem): void
  filteredItems(): SyncQueueItem[]
}

export const useSyncStore = create<SyncStore>((set, get) => ({
  items: [],
  loading: false,
  error: null,
  filter: 'all',

  async fetch() {
    set({ loading: true, error: null })
    try {
      const items = await syncApi.listQueue()
      set({ items, loading: false })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },

  setFilter(filter) {
    set({ filter })
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

  filteredItems() {
    const { items, filter } = get()
    if (filter === 'all') return items
    return items.filter((i) => i.status === filter)
  },
}))
