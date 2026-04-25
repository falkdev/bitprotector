import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { integrityApi } from './integrity'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import {
  makeIntegrityRun,
  makeIntegrityRunResultsResponse,
  makeSingleIntegrityResult,
} from '@/test/factories'

describe('integrityApi', () => {
  it('check/start/active/stop/results endpoints return payloads', async () => {
    const run = makeIntegrityRun({ id: 2 })
    const results = makeIntegrityRunResultsResponse({ run })
    server.use(
      api.post('/integrity/check/:id', () => HttpResponse.json(makeSingleIntegrityResult())),
      api.post('/integrity/runs', () => HttpResponse.json(run, { status: 201 })),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run })),
      api.post('/integrity/runs/:id/stop', () => HttpResponse.json({ ...run, status: 'stopped' })),
      api.get('/integrity/runs/latest', () => HttpResponse.json(results)),
      api.get('/integrity/runs/:id/results', () => HttpResponse.json(results))
    )

    await expect(integrityApi.checkFile(1)).resolves.toMatchObject({ file_id: 1 })
    await expect(integrityApi.startRun(1, true)).resolves.toMatchObject({ id: 2 })
    await expect(integrityApi.activeRun()).resolves.toMatchObject({ run: { id: 2 } })
    await expect(integrityApi.stopRun(2)).resolves.toMatchObject({ status: 'stopped' })
    await expect(integrityApi.latestResults()).resolves.toMatchObject({ total: 1 })
    await expect(integrityApi.runResults(2)).resolves.toMatchObject({ total: 1 })
  })

  it('startRun propagates backend errors', async () => {
    server.use(api.post('/integrity/runs', () => apiError(409, 'already running')))
    await expect(integrityApi.startRun()).rejects.toBeTruthy()
  })
})
