import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { databaseApi } from './database'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import { makeBackupConfig, makeRunBackupResult } from '@/test/factories'

describe('databaseApi', () => {
  it('list/get/create/update/delete/run backup calls return expected payloads', async () => {
    const cfg = makeBackupConfig({ id: 7 })
    server.use(
      api.get('/database/backups', () => HttpResponse.json([cfg])),
      api.get('/database/backups/:id', () => HttpResponse.json(cfg)),
      api.post('/database/backups', () => HttpResponse.json(cfg, { status: 201 })),
      api.put('/database/backups/:id', () => HttpResponse.json(cfg)),
      api.delete('/database/backups/:id', () => new HttpResponse(null, { status: 204 })),
      api.post('/database/backups/run', () => HttpResponse.json([makeRunBackupResult()]))
    )

    await expect(databaseApi.listBackups()).resolves.toHaveLength(1)
    await expect(databaseApi.getBackup(7)).resolves.toMatchObject({ id: 7 })
    await expect(
      databaseApi.createBackup({
        backup_path: '/mnt/b',
        drive_label: 'usb1',
        max_copies: 5,
        enabled: true,
      })
    ).resolves.toMatchObject({ id: 7 })
    await expect(databaseApi.updateBackup(7, { max_copies: 10 })).resolves.toMatchObject({ id: 7 })
    await expect(databaseApi.deleteBackup(7)).resolves.toBeUndefined()
    await expect(
      databaseApi.runBackup('/var/lib/bitprotector/bitprotector.db')
    ).resolves.toHaveLength(1)
  })

  it('listBackups propagates backend errors', async () => {
    server.use(api.get('/database/backups', () => apiError(500, 'db failed')))
    await expect(databaseApi.listBackups()).rejects.toBeTruthy()
  })
})
