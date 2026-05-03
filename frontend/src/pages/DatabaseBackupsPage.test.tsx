import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { DatabaseBackupsPage } from './DatabaseBackupsPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import {
  makeBackupConfig,
  makeBackupIntegrityResult,
  makeBackupSettings,
  makeRestoreBackupResult,
  makeRunBackupResult,
} from '@/test/factories'
import { renderWithApp } from '@/test/render'

vi.mock('@/components/shared/PathPickerDialog', () => ({
  PathPickerDialog: ({
    open,
    title,
    mode,
    onPick,
  }: {
    open: boolean
    title: string
    mode: 'file' | 'directory'
    onPick: (path: string) => void
  }) =>
    open ? (
      <div role="dialog" aria-label={title}>
        <button
          type="button"
          onClick={() =>
            onPick(mode === 'file' ? '/mnt/spare1/bitprotector.db' : '/mnt/spare1/bitprotector')
          }
        >
          Use mocked path
        </button>
      </div>
    ) : null,
}))

describe('DatabaseBackupsPage', () => {
  it('creates a browsed destination and runs a manual backup', async () => {
    const user = userEvent.setup()
    const backups = [makeBackupConfig()]
    let createdBody: unknown = null

    server.use(
      api.get('/database/backups', () => HttpResponse.json(backups)),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings())),
      api.post('/database/backups', async ({ request }) => {
        createdBody = await request.json()
        const created = makeBackupConfig({
          id: 2,
          backup_path: '/mnt/spare1/bitprotector',
          drive_label: 'usb-e2e',
          priority: 1,
        })
        backups.push(created)
        return HttpResponse.json(created)
      }),
      api.post('/database/backups/run', () =>
        HttpResponse.json([
          makeRunBackupResult({
            backup_config_id: 2,
            backup_path: '/mnt/spare1/bitprotector/bitprotector.db',
          }),
        ])
      )
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByTestId('database-backup-row-1')
    expect(screen.getByTestId('database-backup-row-1')).toHaveTextContent('ok')
    await user.click(screen.getByRole('button', { name: 'Add Destination' }))
    await user.click(
      within(screen.getByLabelText('Backup Path').parentElement!).getByText('Browse')
    )
    await user.click(screen.getByRole('button', { name: 'Use mocked path' }))
    await user.type(screen.getByLabelText('Drive Label'), 'usb-e2e')
    await user.click(screen.getByRole('button', { name: 'Create Destination' }))

    expect(await screen.findByText('Backup destination created')).toBeInTheDocument()
    expect(createdBody).toEqual({
      backup_path: '/mnt/spare1/bitprotector',
      drive_label: 'usb-e2e',
      enabled: true,
    })

    await user.click(screen.getByRole('button', { name: 'Run Backup Now' }))

    expect(await screen.findByText('Backed up to 1 destination(s)')).toBeInTheDocument()
    expect(await screen.findByText('/mnt/spare1/bitprotector/bitprotector.db')).toBeInTheDocument()
  })

  it('saves backup settings', async () => {
    const user = userEvent.setup()
    let settingsBody: unknown = null

    server.use(
      api.get('/database/backups', () => HttpResponse.json([])),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings())),
      api.put('/database/backups/settings', async ({ request }) => {
        settingsBody = await request.json()
        return HttpResponse.json(
          makeBackupSettings({
            backup_enabled: true,
            backup_interval_seconds: 3600,
            integrity_enabled: true,
            integrity_interval_seconds: 7200,
          })
        )
      })
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByText('No backup destinations configured')
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await user.click(screen.getByRole('button', { name: 'Enable automatic backups' }))
    await user.clear(screen.getByLabelText('Automatic backups interval value'))
    await user.type(screen.getByLabelText('Automatic backups interval value'), '1')
    await user.selectOptions(screen.getByLabelText('Automatic backups interval unit'), 'hours')
    await user.click(screen.getByRole('button', { name: 'Enable automatic integrity checks' }))
    await user.clear(screen.getByLabelText('Automatic integrity checks interval value'))
    await user.type(screen.getByLabelText('Automatic integrity checks interval value'), '2')
    await user.selectOptions(
      screen.getByLabelText('Automatic integrity checks interval unit'),
      'hours'
    )
    await user.click(screen.getByRole('button', { name: 'Save Settings' }))

    expect(await screen.findByText('Backup settings updated')).toBeInTheDocument()
    expect(settingsBody).toEqual({
      backup_enabled: true,
      backup_interval_seconds: 3600,
      integrity_enabled: true,
      integrity_interval_seconds: 7200,
    })
  })

  it('disables manual actions when there are no enabled destinations', async () => {
    server.use(
      api.get('/database/backups', () =>
        HttpResponse.json([makeBackupConfig({ id: 1, enabled: false })])
      ),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings()))
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByTestId('database-backup-row-1')
    expect(screen.getByRole('button', { name: 'Run Backup Now' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Check Integrity Now' })).toBeDisabled()
    expect(screen.getByTestId('database-backups-manual-actions-disabled-hint')).toHaveTextContent(
      'Enable at least one backup destination to run manual backup and integrity checks.'
    )
  })

  it('runs integrity check and stages restore from a browsed backup file', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/database/backups', () =>
        HttpResponse.json([makeBackupConfig({ last_integrity_status: 'corrupt' })])
      ),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings())),
      api.post('/database/backups/integrity-check', () =>
        HttpResponse.json([
          makeBackupIntegrityResult({
            status: 'repaired',
            backup_path: '/mnt/backups/bitprotector/bitprotector.db',
          }),
        ])
      ),
      api.post('/database/backups/restore', () => HttpResponse.json(makeRestoreBackupResult()))
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByTestId('database-backup-row-1')
    await user.click(screen.getByRole('button', { name: 'Check Integrity Now' }))
    expect(await screen.findByText('Backup integrity check completed')).toBeInTheDocument()
    expect(await screen.findByText('Latest Integrity Check')).toBeInTheDocument()
    expect(await screen.findByText('repaired')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Restore Older Backup' }))
    await user.click(
      within(screen.getByLabelText('Backup File').parentElement!).getByText('Browse')
    )
    await user.click(screen.getByRole('button', { name: 'Use mocked path' }))
    await user.click(screen.getByRole('button', { name: 'Stage Restore' }))

    expect(
      await screen.findByText('Restore staged; restart BitProtector to apply it')
    ).toBeInTheDocument()
    expect(await screen.findByText('Restore Staged')).toBeInTheDocument()
  })

  it('validates the backup form before submitting', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/database/backups', () => HttpResponse.json([])),
      api.get('/database/backups/settings', () => HttpResponse.json(makeBackupSettings()))
    )

    renderWithApp(<DatabaseBackupsPage />)

    await screen.findByText('No backup destinations configured')
    await user.click(screen.getByRole('button', { name: 'Add Destination' }))
    await user.click(screen.getByRole('button', { name: 'Create Destination' }))

    expect(await screen.findByText('Backup path is required.')).toBeInTheDocument()
  })
})
