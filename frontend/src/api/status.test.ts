import { describe, expect, it } from 'vitest'
import { statusApi } from './status'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { HttpResponse } from 'msw'
import { makeSystemStatus } from '@/test/factories'

describe('statusApi', () => {
  it('get returns system status', async () => {
    server.use(api.get('/status', () => HttpResponse.json(makeSystemStatus({ drive_pairs: 2 }))))
    await expect(statusApi.get()).resolves.toMatchObject({ drive_pairs: 2 })
  })

  it('get propagates backend errors', async () => {
    server.use(api.get('/status', () => apiError(500, 'status failed')))
    await expect(statusApi.get()).rejects.toBeTruthy()
  })
})
