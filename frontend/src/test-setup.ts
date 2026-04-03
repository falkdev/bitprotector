import '@testing-library/jest-dom'
import { afterAll, afterEach, beforeAll, vi } from 'vitest'
import { cleanup } from '@testing-library/react'
import { server } from '@/test/msw/server'
import { useAuthStore } from '@/stores/auth-store'
import { useDrivesStore } from '@/stores/drives-store'
import { useLogsStore } from '@/stores/logs-store'
import { useStatusStore } from '@/stores/status-store'
import { useSyncStore } from '@/stores/sync-store'
import { useVirtualPathsStore } from '@/stores/virtual-paths-store'

function resetStores() {
  useAuthStore.setState({
    token: null,
    username: null,
    expiresAt: null,
    isAuthenticated: false,
  })
  useDrivesStore.setState({
    drives: [],
    loading: false,
    error: null,
  })
  useLogsStore.setState({
    entries: [],
    loading: false,
    error: null,
    params: { per_page: 50 },
  })
  useStatusStore.setState({
    status: null,
    loading: false,
    error: null,
  })
  useSyncStore.setState({
    items: [],
    loading: false,
    error: null,
    filter: 'all',
  })
  useVirtualPathsStore.setState({
    files: [],
    loading: false,
    error: null,
  })
}

beforeAll(() => {
  server.listen({ onUnhandledRequest: 'error' })
})

afterEach(() => {
  cleanup()
  server.resetHandlers()
  localStorage.clear()
  resetStores()
  window.history.replaceState({}, '', '/')
  vi.useRealTimers()
})

afterAll(() => {
  server.close()
})
