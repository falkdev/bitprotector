import { beforeEach, describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { useSyncStore } from './sync-store'
import type { SyncQueueItem } from '@/types/sync'

function makeSyncItem(overrides: Partial<SyncQueueItem> = {}): SyncQueueItem {
  return {
    id: 1,
    tracked_file_id: 1,
    action: 'mirror',
    status: 'pending',
    error_message: null,
    created_at: '2026-01-01T00:00:00Z',
    completed_at: null,
    ...overrides,
  }
}

function resetStore() {
  useSyncStore.setState({
    items: [],
    queuePaused: false,
    activeItems: 0,
    loading: false,
    error: null,
    filter: 'all',
  })
}

describe('sync-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('fetch sets items and queuePaused on success', async () => {
    const item = makeSyncItem()
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({ queue: [item], queue_paused: true, active_items: 1 })
      )
    )

    await useSyncStore.getState().fetch()

    expect(useSyncStore.getState().items).toEqual([item])
    expect(useSyncStore.getState().queuePaused).toBe(true)
    expect(useSyncStore.getState().activeItems).toBe(1)
    expect(useSyncStore.getState().loading).toBe(false)
    expect(useSyncStore.getState().error).toBeNull()
  })

  it('fetch sets error on failure', async () => {
    server.use(api.get('/sync/queue', () => HttpResponse.json({ error: 'fail' }, { status: 500 })))

    await useSyncStore.getState().fetch()

    expect(useSyncStore.getState().error).toBeTruthy()
    expect(useSyncStore.getState().loading).toBe(false)
  })

  it('setFilter updates filter', () => {
    useSyncStore.getState().setFilter('completed')
    expect(useSyncStore.getState().filter).toBe('completed')
  })

  it('filteredItems returns all items when filter is all', () => {
    const item1 = makeSyncItem({ id: 1, status: 'pending' })
    const item2 = makeSyncItem({ id: 2, status: 'completed' })
    useSyncStore.setState({ items: [item1, item2], filter: 'all' })

    expect(useSyncStore.getState().filteredItems()).toHaveLength(2)
  })

  it('filteredItems returns only matching status items', () => {
    const item1 = makeSyncItem({ id: 1, status: 'pending' })
    const item2 = makeSyncItem({ id: 2, status: 'completed' })
    useSyncStore.setState({ items: [item1, item2], filter: 'completed' })

    const filtered = useSyncStore.getState().filteredItems()
    expect(filtered).toHaveLength(1)
    expect(filtered[0].id).toBe(2)
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
