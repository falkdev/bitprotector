import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DatabaseBackupsPage } from './DatabaseBackupsPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeBackupConfig, makeRunBackupResult } from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('DatabaseBackupsPage', () => {
  it('creates a destination and runs a manual backup', async () => {
    const user = userEvent.setup()
    const backups = [makeBackupConfig()]
    let createdBody: unknown = null

    server.use(
      api.get('/database/backups', () => HttpResponse.json(backups)),
      api.post('/database/backups', async ({ request }) => {
        createdBody = await request.json()
        const created = makeBackupConfig({
          id: 2,
          backup_path: '/mnt/spare1/bitprotector',
          drive_label: 'usb-e2e',
          max_copies: 3,
        })
        backups.push(created)
        return HttpResponse.json(created)
      }),
      api.post('/database/backups/run', () =>
        HttpResponse.json([
          makeRunBackupResult({ backup_config_id: 2, backup_path: '/mnt/spare1/bitprotector' }),
        ])
      )
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByTestId('database-backup-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Destination' }))
    await user.type(screen.getByLabelText('Backup Path'), '/mnt/spare1/bitprotector')
    await user.type(screen.getByLabelText('Drive Label'), 'usb-e2e')
    await user.clear(screen.getByLabelText('Max Copies'))
    await user.type(screen.getByLabelText('Max Copies'), '3')
    await user.click(screen.getByRole('button', { name: 'Create Destination' }))

    expect(await screen.findByText('Backup destination created')).toBeInTheDocument()
    expect(createdBody).toEqual({
      backup_path: '/mnt/spare1/bitprotector',
      drive_label: 'usb-e2e',
      max_copies: 3,
      enabled: true,
    })

    await user.click(screen.getByRole('button', { name: 'Run Backup Now' }))

    expect(await screen.findByText('Backed up to 1 destination(s)')).toBeInTheDocument()
    expect((await screen.findAllByText('/mnt/spare1/bitprotector')).length).toBeGreaterThan(0)
  })

  it('validates the backup form before submitting', async () => {
    const user = userEvent.setup()

    server.use(api.get('/database/backups', () => HttpResponse.json([])))

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByText('No backup destinations configured')
    await user.click(screen.getByRole('button', { name: 'Add Destination' }))
    await user.type(screen.getByLabelText('Backup Path'), '/mnt/backups')
    await user.clear(screen.getByLabelText('Max Copies'))
    await user.type(screen.getByLabelText('Max Copies'), '0')
    await user.click(screen.getByRole('button', { name: 'Create Destination' }))

    expect(await screen.findByText('Max copies must be a positive number.')).toBeInTheDocument()
  })
})
