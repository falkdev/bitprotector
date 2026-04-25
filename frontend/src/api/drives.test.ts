import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { drivesApi } from './drives'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeDrivePair } from '@/test/factories'

describe('drivesApi', () => {
  it('CRUD and replacement endpoints return payloads', async () => {
    const pair = makeDrivePair({ id: 5 })
    server.use(
      api.get('/drives', () => HttpResponse.json([pair])),
      api.get('/drives/:id', () => HttpResponse.json(pair)),
      api.post('/drives', () => HttpResponse.json(pair, { status: 201 })),
      api.put('/drives/:id', () => HttpResponse.json(pair)),
      api.delete('/drives/:id', () => new HttpResponse(null, { status: 204 })),
      api.post('/drives/:id/replacement/mark', () => HttpResponse.json(pair)),
      api.post('/drives/:id/replacement/cancel', () => HttpResponse.json(pair)),
      api.post('/drives/:id/replacement/confirm', () => HttpResponse.json(pair)),
      api.post('/drives/:id/replacement/assign', () =>
        HttpResponse.json({ drive_pair: pair, queued_rebuild_items: 2 })
      )
    )

    await expect(drivesApi.list()).resolves.toHaveLength(1)
    await expect(drivesApi.get(5)).resolves.toMatchObject({ id: 5 })
    await expect(
      drivesApi.create({
        name: 'pair',
        primary_path: '/p',
        secondary_path: '/s',
        skip_validation: true,
      })
    ).resolves.toMatchObject({ id: 5 })
    await expect(drivesApi.update(5, { name: 'new' })).resolves.toMatchObject({ id: 5 })
    await expect(drivesApi.delete(5)).resolves.toBeUndefined()
    await expect(drivesApi.markReplacement(5, { role: 'primary' })).resolves.toMatchObject({
      id: 5,
    })
    await expect(drivesApi.cancelReplacement(5, { role: 'primary' })).resolves.toMatchObject({
      id: 5,
    })
    await expect(drivesApi.confirmFailure(5, { role: 'primary' })).resolves.toMatchObject({ id: 5 })
    await expect(
      drivesApi.assignReplacement(5, { role: 'primary', new_path: '/new', skip_validation: true })
    ).resolves.toMatchObject({ queued_rebuild_items: 2 })
  })

  it('create propagates backend errors', async () => {
    server.use(api.post('/drives', () => apiError(400, 'invalid drives')))
    await expect(
      drivesApi.create({
        name: 'bad',
        primary_path: '/x',
        secondary_path: '/y',
        skip_validation: false,
      })
    ).rejects.toBeTruthy()
  })
})
