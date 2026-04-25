import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { filesApi } from './files'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeTrackedFile, makeTrackedFileListResponse } from '@/test/factories'

describe('filesApi', () => {
  it('list/get/track/delete/mirror call expected endpoints', async () => {
    const file = makeTrackedFile({ id: 9 })
    server.use(
      api.get('/files', () => HttpResponse.json(makeTrackedFileListResponse([file]))),
      api.get('/files/:id', () => HttpResponse.json(file)),
      api.post('/files', () => HttpResponse.json(file, { status: 201 })),
      api.delete('/files/:id', () => new HttpResponse(null, { status: 204 })),
      api.post('/files/:id/mirror', () => HttpResponse.json({ ...file, is_mirrored: true }))
    )

    await expect(filesApi.list()).resolves.toMatchObject({ total: 1 })
    await expect(filesApi.get(9)).resolves.toMatchObject({ id: 9 })
    await expect(
      filesApi.track({ drive_pair_id: 1, relative_path: 'a.txt' })
    ).resolves.toMatchObject({
      id: 9,
    })
    await expect(filesApi.delete(9)).resolves.toBeUndefined()
    await expect(filesApi.mirror(9)).resolves.toMatchObject({ is_mirrored: true })
  })

  it('track propagates backend errors', async () => {
    server.use(api.post('/files', () => apiError(400, 'bad file path')))
    await expect(
      filesApi.track({ drive_pair_id: 1, relative_path: '../../etc/passwd' })
    ).rejects.toBeTruthy()
  })
})
