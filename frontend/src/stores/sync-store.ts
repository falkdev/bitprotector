import { create } from 'zustand'
import { syncApi } from '@/api/sync'
import type { SyncQueueItem, SyncStatus, SyncSummary } from '@/types/sync'

interface SyncStore {
  /** Live summary pushed via the SSE stream. */
  summary: SyncSummary | null
  items: SyncQueueItem[]
  loading: boolean
  error: string | null
  filter: SyncStatus | 'all'
  page: number
  perPage: number
  /**
   * Total count of items for the *current filter* from the last REST fetch.
   * Used for pagination. Not the same as `summary.total` (which is always
   * the unfiltered total).
   */
  filteredTotal: number

  /** Update the summary received from the SSE stream. */
  setSummary(summary: SyncSummary): void
  /** Fetch the filtered item list; drops stale responses. */
  fetchItems(): Promise<void>
  setFilter(filter: SyncStatus | 'all'): Promise<void>
  setPage(page: number): Promise<void>
  refreshItem(item: SyncQueueItem): void
}

/** Monotonically increasing request counter used to drop stale responses. */
let fetchSeq = 0

export const useSyncStore = create<SyncStore>((set, get) => ({
  summary: null,
  items: [],
  loading: false,
  error: null,
  filter: 'all',
  page: 1,
  perPage: 50,
  filteredTotal: 0,

  setSummary(summary) {
    set({ summary })
  },

  async fetchItems() {
    const seq = ++fetchSeq
    set({ loading: true, error: null })
    try {
      const { filter, page, perPage } = get()
      const response = await syncApi.listQueue({
        status: filter === 'all' ? undefined : filter,
        page,
        perPage,
      })
      // Drop stale response if a newer fetch completed first.
      if (seq !== fetchSeq) return
      set({
        items: response.queue,
        filteredTotal: response.total,
        page: response.page,
        perPage: response.per_page,
        loading: false,
      })
    } catch (err) {
      if (seq !== fetchSeq) return
      set({ loading: false, error: String(err) })
    }
  },

  async setFilter(filter) {
    set({ filter, page: 1 })
    await get().fetchItems()
  },

  async setPage(page) {
    set({ page })
    await get().fetchItems()
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
