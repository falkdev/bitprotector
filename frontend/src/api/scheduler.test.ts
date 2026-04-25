import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { schedulerApi } from './scheduler'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeSchedule } from '@/test/factories'

describe('schedulerApi', () => {
  it('list/get/create/update/delete return schedule payloads', async () => {
    const schedule = makeSchedule({ id: 11 })
    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [schedule], total: 1 })),
      api.get('/scheduler/schedules/:id', () => HttpResponse.json(schedule)),
      api.post('/scheduler/schedules', () => HttpResponse.json(schedule, { status: 201 })),
      api.put('/scheduler/schedules/:id', () => HttpResponse.json(schedule)),
      api.delete('/scheduler/schedules/:id', () => new HttpResponse(null, { status: 204 }))
    )

    await expect(schedulerApi.list()).resolves.toHaveLength(1)
    await expect(schedulerApi.get(11)).resolves.toMatchObject({ id: 11 })
    await expect(
      schedulerApi.create({ task_type: 'sync', cron_expr: '0 2 * * *' })
    ).resolves.toMatchObject({
      id: 11,
    })
    await expect(schedulerApi.update(11, { enabled: false })).resolves.toMatchObject({ id: 11 })
    await expect(schedulerApi.delete(11)).resolves.toBeUndefined()
  })

  it('create propagates backend errors', async () => {
    server.use(api.post('/scheduler/schedules', () => apiError(400, 'invalid schedule')))
    await expect(schedulerApi.create({ task_type: 'sync' })).rejects.toBeTruthy()
  })
})
