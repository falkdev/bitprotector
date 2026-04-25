import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { syncApi } from './sync'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeSyncQueueItem } from '@/test/factories'

describe('syncApi', () => {
  it('queue/list/get/resolve/process/clear endpoints return payloads', async () => {
    const item = makeSyncQueueItem({ id: 8 })
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({ queue: [item], total: 1, page: 1, per_page: 50 })
      ),
      api.post('/sync/queue', () => HttpResponse.json(item, { status: 201 })),
      api.get('/sync/queue/:id', () => HttpResponse.json(item)),
      api.post('/sync/queue/:id/resolve', () =>
        HttpResponse.json({ ...item, status: 'completed' })
      ),
      api.post('/sync/process', () => HttpResponse.json({ processed: 3 })),
      api.delete('/sync/queue/completed', () => HttpResponse.json({ deleted: 2 }))
    )

    await expect(syncApi.listQueue()).resolves.toHaveLength(1)
    await expect(
      syncApi.addQueueItem({ tracked_file_id: 1, action: 'mirror' })
    ).resolves.toMatchObject({ id: 8 })
    await expect(syncApi.getQueueItem(8)).resolves.toMatchObject({ id: 8 })
    await expect(syncApi.resolveQueueItem(8, { resolution: 'keep_master' })).resolves.toMatchObject(
      {
        status: 'completed',
      }
    )
    await expect(syncApi.processQueue()).resolves.toMatchObject({ processed: 3 })
    await expect(syncApi.clearCompletedQueue()).resolves.toMatchObject({ deleted: 2 })
  })

  it('addQueueItem propagates backend errors', async () => {
    server.use(api.post('/sync/queue', () => apiError(400, 'bad queue request')))
    await expect(
      syncApi.addQueueItem({ tracked_file_id: 1, action: 'bad' as never })
    ).rejects.toBeTruthy()
  })
})
