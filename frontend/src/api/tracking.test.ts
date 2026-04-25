import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { trackingApi } from './tracking'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import {
  makeTrackedFile,
  makeTrackedFolder,
  makeTrackingListResponse,
  makeTrackingItem,
} from '@/test/factories'

describe('trackingApi', () => {
  it('list returns direct tracking endpoint response when available', async () => {
    const item = makeTrackingItem({ id: 1 })
    server.use(
      api.get('/tracking/items', () => HttpResponse.json(makeTrackingListResponse([item])))
    )
    await expect(trackingApi.list()).resolves.toMatchObject({ total: 1 })
  })

  it('list falls back to files+folders when tracking endpoint is missing', async () => {
    server.use(
      api.get('/tracking/items', () => apiError(404, 'not found')),
      api.get('/files', () =>
        HttpResponse.json({
          files: [makeTrackedFile({ id: 10, relative_path: 'docs/a.txt' })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      ),
      api.get('/folders', () =>
        HttpResponse.json([
          makeTrackedFolder({ id: 11, folder_path: 'docs', virtual_path: '/v/docs' }),
        ])
      )
    )

    const result = await trackingApi.list({ page: 1, per_page: 50 })
    expect(result.items.length).toBeGreaterThan(0)
    expect(result.total).toBeGreaterThan(0)
  })

  it('list propagates non-404 errors from tracking endpoint', async () => {
    server.use(api.get('/tracking/items', () => apiError(500, 'tracking failed')))
    await expect(trackingApi.list()).rejects.toBeTruthy()
  })
})
