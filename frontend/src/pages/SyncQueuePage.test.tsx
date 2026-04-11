import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SyncQueuePage } from './SyncQueuePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeDrivePair, makeSyncQueueItem } from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('SyncQueuePage', () => {
  beforeEach(() => {
    server.use(api.get('/drives', () => HttpResponse.json([makeDrivePair()])))
  })

  it('resolves a pending manual action through the dialog', async () => {
    const user = userEvent.setup()
    let item = makeSyncQueueItem({
      id: 7,
      tracked_file_id: 44,
      action: 'user_action_required',
      status: 'pending',
    })
    let resolutionBody: unknown = null

    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [item],
          total: 1,
          page: 1,
          per_page: 50,
        })
      ),
      api.post('/sync/queue/7/resolve', async ({ request }) => {
        resolutionBody = await request.json()
        item = { ...item, status: 'completed' }
        return HttpResponse.json(item)
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-7')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Resolve' }))
    const resolveButtons = screen.getAllByRole('button', { name: 'Resolve' })
    await user.click(resolveButtons[resolveButtons.length - 1])

    expect(await screen.findByText('Queue item #7 resolved')).toBeInTheDocument()
    expect(resolutionBody).toEqual({ resolution: 'keep_master' })
  })

  it('polls the queue every five seconds', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })

    let listCalls = 0

    server.use(
      api.get('/sync/queue', () => {
        listCalls += 1
        return HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 9 })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-9')).toBeInTheDocument()

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000)
    })

    await waitFor(() => {
      expect(listCalls).toBeGreaterThan(1)
    })
  })

  it('renders the empty state when no queue items are returned', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [],
          total: 0,
          page: 1,
          per_page: 50,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText('No queue items')).toBeInTheDocument()
  })

  it('shows process queue action and does not render sync/integrity task buttons', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 11 })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('button', { name: 'Process Queue' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Clear Completed' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Run Sync Task' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Run Integrity Task' })).not.toBeInTheDocument()
  })

  it('disables process queue and shows helper text when no drive pairs exist', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json([])),
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 15, status: 'completed' })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('button', { name: 'Process Queue' })).toBeDisabled()
    expect(await screen.findByTestId('sync-queue-no-drives-hint')).toHaveTextContent(
      'Add a drive pair first to process the sync queue.'
    )
    expect(screen.getByRole('button', { name: 'Clear Completed' })).toBeEnabled()
  })

  it('disables clear completed button when there are no completed items', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 12, status: 'pending' })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const clearButton = await screen.findByRole('button', { name: 'Clear Completed' })
    expect(clearButton).toBeDisabled()
  })

  it('enables clear completed button when completed items exist', async () => {
    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 13, status: 'completed' })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const clearButton = await screen.findByRole('button', { name: 'Clear Completed' })
    expect(clearButton).toBeEnabled()
  })

  it('clears completed items and refreshes queue data', async () => {
    const user = userEvent.setup()
    let queue = [
      makeSyncQueueItem({ id: 21, status: 'completed' }),
      makeSyncQueueItem({ id: 22, status: 'pending' }),
    ]
    let clearCalls = 0

    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue,
          total: queue.length,
          page: 1,
          per_page: 50,
        })
      ),
      api.delete('/sync/queue/completed', () => {
        clearCalls += 1
        queue = queue.filter((item) => item.status !== 'completed')
        return HttpResponse.json({ deleted: 1 })
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-21')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Clear Completed' }))

    expect(await screen.findByText('Cleared 1 completed queue item(s)')).toBeInTheDocument()
    await waitFor(() => {
      expect(clearCalls).toBe(1)
    })
    await waitFor(() => {
      expect(screen.queryByTestId('sync-queue-row-21')).not.toBeInTheDocument()
    })
    expect(screen.getByTestId('sync-queue-row-22')).toBeInTheDocument()
  })

  it('shows an error toast when clear completed fails', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/sync/queue', () =>
        HttpResponse.json({
          queue: [makeSyncQueueItem({ id: 31, status: 'completed' })],
          total: 1,
          page: 1,
          per_page: 50,
        })
      ),
      api.delete('/sync/queue/completed', () => HttpResponse.json({ error: 'failed' }, { status: 500 }))
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-31')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Clear Completed' }))

    expect(await screen.findByText('Failed to clear completed queue items')).toBeInTheDocument()
    expect(screen.getByTestId('sync-queue-row-31')).toBeInTheDocument()
  })
})
