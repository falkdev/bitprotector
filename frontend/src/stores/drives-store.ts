import { create } from 'zustand'
import { drivesApi } from '@/api/drives'
import type { DrivePair, CreateDrivePairRequest, UpdateDrivePairRequest } from '@/types/drive'

interface DrivesState {
  drives: DrivePair[]
  loading: boolean
  error: string | null

  fetch(): Promise<void>
  create(data: CreateDrivePairRequest): Promise<DrivePair>
  update(id: number, data: UpdateDrivePairRequest): Promise<DrivePair>
  remove(id: number): Promise<void>
  refresh(drive: DrivePair): void
}

export const useDrivesStore = create<DrivesState>((set, get) => ({
  drives: [],
  loading: false,
  error: null,

  async fetch() {
    set({ loading: true, error: null })
    try {
      const drives = await drivesApi.list()
      set({ drives, loading: false })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },

  async create(data) {
    const drive = await drivesApi.create(data)
    set((s) => ({ drives: [...s.drives, drive] }))
    return drive
  },

  async update(id, data) {
    const updated = await drivesApi.update(id, data)
    set((s) => ({ drives: s.drives.map((d) => (d.id === id ? updated : d)) }))
    return updated
  },

  async remove(id) {
    await drivesApi.delete(id)
    set((s) => ({ drives: s.drives.filter((d) => d.id !== id) }))
  },

  refresh(drive) {
    const { drives } = get()
    if (drives.find((d) => d.id === drive.id)) {
      set((s) => ({ drives: s.drives.map((d) => (d.id === drive.id ? drive : d)) }))
    } else {
      set((s) => ({ drives: [...s.drives, drive] }))
    }
  },
}))
