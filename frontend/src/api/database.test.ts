import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { databaseApi } from './database'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import {
  makeBackupConfig,
  makeBackupIntegrityResult,
  makeBackupSettings,
  makeRestoreBackupResult,
  makeRunBackupResult,
} from '@/test/factories'

describe('databaseApi', () => {
  it('list/get/create/update/delete/run backup calls return expected payloads', async () => {
    const cfg = makeBackupConfig({ id: 7 })
    server.use(
      api.get('/database/backups', () => HttpResponse.json([cfg])),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings())),
      api.get('/database/backups/:id', () => HttpResponse.json(cfg)),
      api.post('/database/backups', () => HttpResponse.json(cfg, { status: 201 })),
      api.put('/database/backups/settings', () =>
        HttpResponse.json(makeBackupSettings({ backup_enabled: true }))
      ),
      api.put('/database/backups/:id', () => HttpResponse.json(cfg)),
      api.delete('/database/backups/:id', () => new HttpResponse(null, { status: 204 })),
      api.post('/database/backups/run', () => HttpResponse.json([makeRunBackupResult()])),
      api.post('/database/backups/integrity-check', () =>
        HttpResponse.json([makeBackupIntegrityResult()])
      ),
      api.post('/database/backups/restore', () => HttpResponse.json(makeRestoreBackupResult()))
    )

    await expect(databaseApi.listBackups()).resolves.toHaveLength(1)
    await expect(databaseApi.getBackup(7)).resolves.toMatchObject({ id: 7 })
    await expect(
      databaseApi.createBackup({
        backup_path: '/mnt/b',
        drive_label: 'usb1',
        enabled: true,
      })
    ).resolves.toMatchObject({ id: 7 })
    await expect(databaseApi.updateBackup(7, { enabled: false })).resolves.toMatchObject({ id: 7 })
    await expect(databaseApi.deleteBackup(7)).resolves.toBeUndefined()
    await expect(databaseApi.runBackup()).resolves.toHaveLength(1)
    await expect(databaseApi.getSettings()).resolves.toMatchObject({ backup_enabled: false })
    await expect(databaseApi.updateSettings({ backup_enabled: true })).resolves.toMatchObject({
      backup_enabled: true,
    })
    await expect(databaseApi.runIntegrityCheck()).resolves.toHaveLength(1)
    await expect(databaseApi.restoreBackup({ source_path: '/mnt/old.db' })).resolves.toMatchObject({
      restart_required: true,
    })
  })

  it('listBackups propagates backend errors', async () => {
    server.use(api.get('/database/backups', () => apiError(500, 'db failed')))
    await expect(databaseApi.listBackups()).rejects.toBeTruthy()
  })
})
