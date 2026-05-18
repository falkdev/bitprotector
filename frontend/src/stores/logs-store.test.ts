import { beforeEach, describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { useLogsStore } from './logs-store'
import type { EventLogEntry } from '@/types/log'

function makeLogEntry(overrides: Partial<EventLogEntry> = {}): EventLogEntry {
  return {
    id: 1,
    event_type: 'integrity_pass',
    tracked_file_id: 1,
    file_path: '/test/file.txt',
    message: 'all good',
    details: null,
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function resetStore() {
  useLogsStore.setState({ entries: [], loading: false, error: null, params: { per_page: 50 } })
}

describe('logs-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('fetch sets entries on success', async () => {
    const entry = makeLogEntry()
    server.use(
      api.get('/logs', () => HttpResponse.json({ logs: [entry], total: 1, page: 1, per_page: 50 }))
    )

    await useLogsStore.getState().fetch()

    expect(useLogsStore.getState().entries).toEqual([entry])
    expect(useLogsStore.getState().loading).toBe(false)
    expect(useLogsStore.getState().error).toBeNull()
  })

  it('fetch merges provided params with stored params', async () => {
    server.use(
      api.get('/logs', () => HttpResponse.json({ logs: [], total: 0, page: 1, per_page: 10 }))
    )

    await useLogsStore.getState().fetch({ per_page: 10 })

    expect(useLogsStore.getState().params.per_page).toBe(10)
  })

  it('fetch sets error on failure', async () => {
    server.use(api.get('/logs', () => HttpResponse.json({ error: 'fail' }, { status: 500 })))

    await useLogsStore.getState().fetch()

    expect(useLogsStore.getState().error).toBeTruthy()
    expect(useLogsStore.getState().loading).toBe(false)
  })

  it('setParams merges into existing params', () => {
    useLogsStore.getState().setParams({ per_page: 25 })

    expect(useLogsStore.getState().params.per_page).toBe(25)
  })
})
