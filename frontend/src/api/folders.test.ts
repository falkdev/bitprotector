import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { foldersApi } from './folders'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeTrackedFolder, makeScanFolderResult } from '@/test/factories'

describe('foldersApi', () => {
  it('list/get/create/update/delete/scan/mirror return payloads', async () => {
    const folder = makeTrackedFolder({ id: 4 })
    server.use(
      api.get('/folders', () => HttpResponse.json([folder])),
      api.get('/folders/:id', () => HttpResponse.json(folder)),
      api.post('/folders', () => HttpResponse.json(folder, { status: 201 })),
      api.put('/folders/:id', () => HttpResponse.json(folder)),
      api.delete('/folders/:id', () => new HttpResponse(null, { status: 204 })),
      api.post('/folders/:id/scan', () => HttpResponse.json(makeScanFolderResult())),
      api.post('/folders/:id/mirror', () => HttpResponse.json({ mirrored_files: 3 }))
    )

    await expect(foldersApi.list()).resolves.toHaveLength(1)
    await expect(foldersApi.get(4)).resolves.toMatchObject({ id: 4 })
    await expect(
      foldersApi.create({ drive_pair_id: 1, folder_path: 'docs', virtual_path: null })
    ).resolves.toMatchObject({ id: 4 })
    await expect(foldersApi.update(4, { virtual_path: '/docs' })).resolves.toMatchObject({ id: 4 })
    await expect(foldersApi.delete(4)).resolves.toBeUndefined()
    await expect(foldersApi.scan(4)).resolves.toMatchObject({ new_files: 2 })
    await expect(foldersApi.mirror(4)).resolves.toMatchObject({ mirrored_files: 3 })
  })

  it('create propagates backend errors', async () => {
    server.use(api.post('/folders', () => apiError(400, 'invalid folder')))
    await expect(
      foldersApi.create({ drive_pair_id: 1, folder_path: '../bad', virtual_path: null })
    ).rejects.toBeTruthy()
  })
})
