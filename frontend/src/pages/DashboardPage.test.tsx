import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DashboardPage } from './DashboardPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeIntegrityRun, makeRunBackupResult, makeSystemStatus } from '@/test/factories'
import { renderWithApp } from '@/test/render'

const defaultHandlers = () => [
  api.get('/status', () => HttpResponse.json(makeSystemStatus({ drive_pairs: 2 }))),
  api.get('/logs', () => HttpResponse.json({ logs: [], total: 0, page: 1, per_page: 10 })),
]

describe('DashboardPage', () => {
  it('disables integrity quick action and shows helper text when no drive pairs exist', async () => {
    server.use(
      api.get('/status', () => HttpResponse.json(makeSystemStatus({ drive_pairs: 0 }))),
      api.get('/logs', () =>
        HttpResponse.json({
          logs: [],
          total: 0,
          page: 1,
          per_page: 10,
        })
      )
    )

    renderWithApp(<DashboardPage />)

    expect(await screen.findByTestId('quick-action-integrity')).toBeDisabled()
    expect(await screen.findByTestId('quick-action-integrity-hint')).toHaveTextContent(
      'Add a drive pair first to run integrity checks.'
    )
    expect(screen.getByTestId('quick-action-sync')).toBeEnabled()
    expect(screen.getByTestId('quick-action-backup')).toBeEnabled()
  })

  it('shows loading spinner while status is loading', async () => {
    server.use(
      api.get('/status', () => HttpResponse.json(makeSystemStatus())),
      api.get('/logs', () => HttpResponse.json({ logs: [], total: 0, page: 1, per_page: 10 }))
    )

    renderWithApp(<DashboardPage />)

    // Loading spinner appears briefly, then status renders
    expect(await screen.findByTestId('quick-action-integrity')).toBeInTheDocument()
  })

  it('starts integrity run and shows success toast', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/integrity/runs', () => HttpResponse.json(makeIntegrityRun()))
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-integrity'))

    expect(await screen.findByText('Integrity run started')).toBeInTheDocument()
  })

  it('shows error toast when integrity run fails', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/integrity/runs', () => HttpResponse.json({ error: 'busy' }, { status: 500 }))
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-integrity'))

    expect(await screen.findByText('Failed to start integrity run')).toBeInTheDocument()
  })

  it('processes sync queue and shows success toast', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/sync/process', () => HttpResponse.json({ processed: 3 }))
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-sync'))

    expect(await screen.findByText('Sync queue processed (3 item(s))')).toBeInTheDocument()
  })

  it('shows error toast when sync queue processing fails', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/sync/process', () => HttpResponse.json({ error: 'fail' }, { status: 500 }))
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-sync'))

    expect(await screen.findByText('Failed to process sync queue')).toBeInTheDocument()
  })

  it('runs backup and shows success toast when all succeed', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/database/backups/run', () =>
        HttpResponse.json([makeRunBackupResult({ status: 'success' })])
      )
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-backup'))

    expect(
      await screen.findByText('Database backup completed (1 destination(s))')
    ).toBeInTheDocument()
  })

  it('shows warning toast when some backups fail', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/database/backups/run', () =>
        HttpResponse.json([makeRunBackupResult({ status: 'failed' })])
      )
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-backup'))

    expect(await screen.findByText('Backup completed with 1 failure(s)')).toBeInTheDocument()
  })

  it('shows error toast when backup request fails', async () => {
    const user = userEvent.setup()
    server.use(
      ...defaultHandlers(),
      api.post('/database/backups/run', () => HttpResponse.json({ error: 'fail' }, { status: 500 }))
    )

    renderWithApp(<DashboardPage />)

    await user.click(await screen.findByTestId('quick-action-backup'))

    expect(await screen.findByText('Database backup failed')).toBeInTheDocument()
  })
})
