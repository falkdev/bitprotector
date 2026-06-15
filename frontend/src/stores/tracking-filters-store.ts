import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import type { TrackingListParams } from '@/types/tracking'

type TrackingFilters = Omit<TrackingListParams, 'page'>

export const DEFAULT_TRACKING_FILTERS: TrackingFilters = {
  per_page: 50,
  item_kind: 'all',
  source: 'all',
}

interface TrackingFiltersState {
  filters: TrackingFilters
  setFilters(filters: TrackingFilters): void
  resetFilters(): void
}

export const useTrackingFiltersStore = create<TrackingFiltersState>()(
  persist(
    (set) => ({
      filters: DEFAULT_TRACKING_FILTERS,

      setFilters(filters) {
        set({ filters })
      },

      resetFilters() {
        set({ filters: DEFAULT_TRACKING_FILTERS })
      },
    }),
    {
      name: 'bitprotector-tracking-filters',
      storage: createJSONStorage(() => sessionStorage),
    }
  )
)
