import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { logsApi } from './logs'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeLogEntry } from '@/test/factories'

describe('logsApi', () => {
  it('list supports both array and wrapped response formats', async () => {
    const row = makeLogEntry({ id: 3 })
    server.use(
      api.get('/logs', () => HttpResponse.json({ logs: [row], total: 1, page: 1, per_page: 50 }))
    )
    await expect(logsApi.list()).resolves.toHaveLength(1)

    server.use(api.get('/logs', () => HttpResponse.json([row])))
    await expect(logsApi.list()).resolves.toHaveLength(1)
  })

  it('get propagates backend errors', async () => {
    server.use(api.get('/logs/:id', () => apiError(404, 'missing log')))
    await expect(logsApi.get(999)).rejects.toBeTruthy()
  })
})
