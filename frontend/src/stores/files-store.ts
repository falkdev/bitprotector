import { create } from 'zustand'
import { filesApi } from '@/api/files'
import type { TrackedFile, FilesQueryParams, TrackedFileListResponse } from '@/types/file'

interface FilesState {
  response: TrackedFileListResponse | null
  loading: boolean
  error: string | null
  params: FilesQueryParams

  fetch(params?: FilesQueryParams): Promise<void>
  setParams(params: FilesQueryParams): void
  refreshFile(file: TrackedFile): void
  removeFile(id: number): void
}

export const useFilesStore = create<FilesState>((set, get) => ({
  response: null,
  loading: false,
  error: null,
  params: { page: 1, per_page: 50 },

  async fetch(params) {
    const merged = { ...get().params, ...params }
    set({ loading: true, error: null })
    try {
      const response = await filesApi.list(merged)
      set({ response, loading: false })
    } catch (err) {
      set({ loading: false, error: String(err) })
    }
  },

  setParams(params) {
    set({ params })
  },

  refreshFile(file) {
    set((s) => {
      if (!s.response) return s
      const files = s.response.files.map((f) => (f.id === file.id ? file : f))
      return { response: { ...s.response, files } }
    })
  },

  removeFile(id) {
    set((s) => {
      if (!s.response) return s
      return {
        response: {
          ...s.response,
          files: s.response.files.filter((f) => f.id !== id),
          total: s.response.total - 1,
        },
      }
    })
  },
}))
