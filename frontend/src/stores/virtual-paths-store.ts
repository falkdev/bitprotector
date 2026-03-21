import { create } from 'zustand'
import { virtualPathsApi } from '@/api/virtual-paths'
import type { TrackedFile } from '@/types/file'

interface VirtualPathsState {
  files: TrackedFile[]
  loading: boolean
  error: string | null
  setFiles: (files: TrackedFile[]) => void
  setVirtualPath: (fileId: number, virtualPath: string) => Promise<string>
  removeVirtualPath: (fileId: number) => Promise<string>
  clearError: () => void
}

export const useVirtualPathsStore = create<VirtualPathsState>((set) => ({
  files: [],
  loading: false,
  error: null,

  setFiles: (files) => set({ files }),

  setVirtualPath: async (fileId, virtualPath) => {
    return virtualPathsApi.set(fileId, { virtual_path: virtualPath })
  },

  removeVirtualPath: async (fileId) => {
    return virtualPathsApi.remove(fileId)
  },

  clearError: () => set({ error: null }),
}))
