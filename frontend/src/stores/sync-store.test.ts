import { beforeEach, describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { useSyncStore } from './sync-store'
import type { SyncQueueItem } from '@/types/sync'

function makeSyncItem(overrides: Partial<SyncQueueItem> = {}): SyncQueueItem {
  const { reason, ...rest } = overrides

  return {
    id: 1,
    tracked_file_id: 1,
    relative_path: 'documents/report.pdf',
    action: 'mirror',
    status: 'pending',
    error_message: null,
    reason: reason ?? null,
    created_at: '2026-01-01T00:00:00Z',
    completed_at: null,
    ...rest,
  }
}

function resetStore() {
  useSyncStore.setState({
    summary: null,
    items: [],
    loading: false,
    error: null,
    filter: 'all',
    page: 1,
    perPage: 50,
    filteredTotal: 0,
  })
}

describe('sync-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('fetchItems sets items and filteredTotal on success', async () => {
    const item = makeSyncItem()
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [item],
          total: 1,
          page: 1,
          per_page: 50,
          queue_paused: false,
          active_items: 1,
          in_progress_items: 0,
          pending_items: 1,
          completed_items: 0,
          failed_items: 0,
        })
      )
    )

    await useSyncStore.getState().fetchItems()

    expect(useSyncStore.getState().items).toEqual([item])
    expect(useSyncStore.getState().filteredTotal).toBe(1)
    expect(useSyncStore.getState().loading).toBe(false)
    expect(useSyncStore.getState().error).toBeNull()
  })

  it('fetchItems sets error on failure', async () => {
    server.use(api.get('/sync/queue', () => HttpResponse.json({ error: 'fail' }, { status: 500 })))

    await useSyncStore.getState().fetchItems()

    expect(useSyncStore.getState().error).toBeTruthy()
    expect(useSyncStore.getState().loading).toBe(false)
  })

  it('setFilter updates filter and resets to page 1', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [],
          total: 0,
          page: 1,
          per_page: 50,
          queue_paused: false,
          active_items: 0,
          in_progress_items: 0,
          pending_items: 0,
          completed_items: 0,
          failed_items: 0,
        })
      )
    )

    await useSyncStore.getState().setFilter('completed')
    expect(useSyncStore.getState().filter).toBe('completed')
    expect(useSyncStore.getState().page).toBe(1)
  })

  it('setPage updates page', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [],
          total: 0,
          page: 2,
          per_page: 50,
          queue_paused: false,
          active_items: 0,
          in_progress_items: 0,
          pending_items: 0,
          completed_items: 0,
          failed_items: 0,
        })
      )
    )

    await useSyncStore.getState().setPage(2)
    expect(useSyncStore.getState().page).toBe(2)
  })

  it('refreshItem updates existing item by id', () => {
    const original = makeSyncItem({ id: 1, status: 'pending' })
    const other = makeSyncItem({ id: 2, status: 'pending' })
    useSyncStore.setState({ items: [original, other] })

    useSyncStore.getState().refreshItem({ ...original, status: 'completed' })

    expect(useSyncStore.getState().items[0].status).toBe('completed')
    expect(useSyncStore.getState().items[1].id).toBe(2)
  })

  it('refreshItem prepends new item when not found', () => {
    useSyncStore.setState({ items: [] })
    const item = makeSyncItem({ id: 99 })

    useSyncStore.getState().refreshItem(item)

    expect(useSyncStore.getState().items).toHaveLength(1)
    expect(useSyncStore.getState().items[0].id).toBe(99)
  })
})
