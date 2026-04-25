import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { virtualPathsApi } from './virtual-paths'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'

describe('virtualPathsApi', () => {
  it('set/remove/tree return expected payloads', async () => {
    server.use(
      api.put('/virtual-paths/:id', () => HttpResponse.json('/virtual/docs/report.txt')),
      api.delete('/virtual-paths/:id', () => HttpResponse.json('removed')),
      api.get('/virtual-paths/tree', () =>
        HttpResponse.json({
          parent: '/',
          children: [{ name: 'docs', path: '/docs', has_children: false, item_count: 1 }],
        })
      )
    )

    await expect(
      virtualPathsApi.set(1, { virtual_path: '/virtual/docs/report.txt' })
    ).resolves.toBe('/virtual/docs/report.txt')
    await expect(virtualPathsApi.remove(1)).resolves.toBe('removed')
    await expect(virtualPathsApi.tree()).resolves.toMatchObject({ parent: '/' })
  })

  it('tree falls back to empty response on 404 but propagates other errors', async () => {
    server.use(api.get('/virtual-paths/tree', () => apiError(404, 'missing endpoint')))
    await expect(virtualPathsApi.tree('/docs')).resolves.toMatchObject({
      parent: '/docs',
      children: [],
    })

    server.use(api.get('/virtual-paths/tree', () => apiError(500, 'broken')))
    await expect(virtualPathsApi.tree()).rejects.toBeTruthy()
  })
})
