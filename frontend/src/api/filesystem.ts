import axios from 'axios'
import { apiClient } from './client'
import type { BrowseFilesystemParams, BrowseFilesystemResponse } from '@/types/filesystem'

interface ApiErrorResponse {
  error?: {
    message?: string
  }
}

function readApiErrorMessage(data: unknown): string | null {
  const message = (data as ApiErrorResponse | undefined)?.error?.message
  return typeof message === 'string' && message.trim() ? message : null
}

export function getFilesystemBrowserErrorMessage(error: unknown): string {
  if (axios.isAxiosError(error)) {
    if (error.response?.status === 404) {
      return 'The running BitProtector API does not expose the filesystem browser endpoint yet. Rebuild and restart the backend, then refresh the page.'
    }

    const apiMessage = readApiErrorMessage(error.response?.data)
    if (apiMessage) {
      return apiMessage
    }

    if (error.message.trim()) {
      return error.message
    }
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  return 'Failed to load the filesystem browser'
}

export const filesystemApi = {
  children(params?: BrowseFilesystemParams): Promise<BrowseFilesystemResponse> {
    return apiClient
      .get<BrowseFilesystemResponse>('/filesystem/children', { params })
      .then((response) => response.data)
      .catch((error: unknown) => {
        throw new Error(getFilesystemBrowserErrorMessage(error))
      })
  },
}
