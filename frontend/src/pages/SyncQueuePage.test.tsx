import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SyncQueuePage } from './SyncQueuePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeDrivePair, makeSyncQueueItem } from '@/test/factories'
import { renderWithApp } from '@/test/render'

interface QueueResponseOptions {
  total?: number
  page?: number
  perPage?: number
  paused?: boolean
  activeItems?: number
  inProgressItems?: number
}

const queueResponse = (
  items: ReturnType<typeof makeSyncQueueItem>[],
  options: QueueResponseOptions = {}
) =>
  HttpResponse.json({
    queue: items,
    total: options.total ?? items.length,
    page: options.page ?? 1,
    per_page: options.perPage ?? 50,
    queue_paused: options.paused ?? false,
    active_items:
      options.activeItems ??
      items.filter((i) => i.status === 'pending' || i.status === 'in_progress').length,
    in_progress_items:
      options.inProgressItems ?? items.filter((i) => i.status === 'in_progress').length,
  })

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
      api.get('/sync/queue', () => queueResponse([item])),
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
        return queueResponse([makeSyncQueueItem({ id: 9 })])
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
    server.use(api.get('/sync/queue', () => queueResponse([])))

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText('No queue items')).toBeInTheDocument()
  })

  it('shows process queue and pause queue buttons', async () => {
    server.use(api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 11 })])))

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('button', { name: 'Process Queue' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Clear Completed' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Pause Queue' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Run Sync Task' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Run Integrity Task' })).not.toBeInTheDocument()
  })

  it('disables process queue and shows helper text when no drive pairs exist', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json([])),
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 15, status: 'completed' })])
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
        queueResponse([makeSyncQueueItem({ id: 12, status: 'pending' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const clearButton = await screen.findByRole('button', { name: 'Clear Completed' })
    expect(clearButton).toBeDisabled()
  })

  it('enables clear completed button when completed items exist', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 13, status: 'completed' })])
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
      api.get('/sync/queue', () => queueResponse(queue)),
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
        queueResponse([makeSyncQueueItem({ id: 31, status: 'completed' })])
      ),
      api.delete('/sync/queue/completed', () =>
        HttpResponse.json({ error: 'failed' }, { status: 500 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-31')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Clear Completed' }))

    expect(await screen.findByText('Failed to clear completed queue items')).toBeInTheDocument()
    expect(screen.getByTestId('sync-queue-row-31')).toBeInTheDocument()
  })

  it('shows paused banner when queue is paused', async () => {
    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 40 })], { paused: true }))
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('queue-paused-banner')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Resume Queue' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Pause Queue' })).not.toBeInTheDocument()
  })

  it('hides paused banner and shows pause button when queue is not paused', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 41 })], { paused: false })
      )
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-41')
    expect(screen.queryByTestId('queue-paused-banner')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Pause Queue' })).toBeInTheDocument()
  })

  it('pauses the queue when pause button is clicked', async () => {
    const user = userEvent.setup()
    let paused = false

    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 42 })], { paused })),
      api.post('/sync/pause', () => {
        paused = true
        return HttpResponse.json({ queue_paused: true })
      })
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Pause Queue' })
    await user.click(screen.getByRole('button', { name: 'Pause Queue' }))

    expect(await screen.findByText('Sync queue processing paused')).toBeInTheDocument()
    expect(await screen.findByTestId('queue-paused-banner')).toBeInTheDocument()
  })

  it('resumes the queue when resume button is clicked', async () => {
    const user = userEvent.setup()
    let paused = true

    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 43 })], { paused })),
      api.post('/sync/resume', () => {
        paused = false
        return HttpResponse.json({ queue_paused: false })
      })
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Resume Queue' })
    await user.click(screen.getByRole('button', { name: 'Resume Queue' }))

    expect(await screen.findByText('Sync queue processing resumed')).toBeInTheDocument()
    await waitFor(() => {
      expect(screen.queryByTestId('queue-paused-banner')).not.toBeInTheDocument()
    })
  })

  it('filters queue items by requesting a filtered page from the server', async () => {
    const user = userEvent.setup()
    const requestedStatuses: Array<string | null> = []

    server.use(
      api.get('/sync/queue', ({ request }) => {
        const url = new URL(request.url)
        const status = url.searchParams.get('status')
        requestedStatuses.push(status)

        if (status === 'pending') {
          return queueResponse([makeSyncQueueItem({ id: 50, status: 'pending' })], { total: 1 })
        }

        return queueResponse(
          [
            makeSyncQueueItem({ id: 50, status: 'pending' }),
            makeSyncQueueItem({ id: 51, status: 'completed' }),
          ],
          { total: 2 }
        )
      })
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByTestId('sync-queue-row-50')
    await screen.findByTestId('sync-queue-row-51')

    await user.selectOptions(screen.getByRole('combobox'), 'pending')

    await waitFor(() => {
      expect(requestedStatuses).toContain('pending')
      expect(screen.getByTestId('sync-queue-row-50')).toBeInTheDocument()
      expect(screen.queryByTestId('sync-queue-row-51')).not.toBeInTheDocument()
    })
  })

  it('shows the server-reported total count', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 52, status: 'pending' })], { total: 7 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText('7 item(s) total')).toBeInTheDocument()
  })

  it('requests the next page when Next is clicked', async () => {
    const user = userEvent.setup()
    const requestedPages: number[] = []

    server.use(
      api.get('/sync/queue', ({ request }) => {
        const url = new URL(request.url)
        const page = Number(url.searchParams.get('page') ?? '1')
        requestedPages.push(page)

        if (page === 2) {
          return queueResponse([makeSyncQueueItem({ id: 91 })], {
            total: 2,
            page: 2,
            perPage: 1,
          })
        }

        return queueResponse([makeSyncQueueItem({ id: 90 })], {
          total: 2,
          page: 1,
          perPage: 1,
        })
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('sync-queue-row-90')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Next' }))

    await waitFor(() => {
      expect(requestedPages).toContain(2)
      expect(screen.getByTestId('sync-queue-row-91')).toBeInTheDocument()
    })
    expect(screen.getByText('Page 2 of 2')).toBeInTheDocument()
  })

  it('closes the resolve dialog when cancel is clicked', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/sync/queue', () =>
        queueResponse([
          makeSyncQueueItem({ id: 60, action: 'user_action_required', status: 'pending' }),
        ])
      )
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByTestId('sync-queue-row-60')

    await user.click(screen.getByRole('button', { name: 'Resolve' }))
    expect(await screen.findByText('Resolve Queue Item')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    await waitFor(() => {
      expect(screen.queryByText('Resolve Queue Item')).not.toBeInTheDocument()
    })
  })

  it('processes the queue when Process Queue is clicked', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 70 })], { activeItems: 1, inProgressItems: 0 })
      ),
      api.post('/sync/process', () => HttpResponse.json({ processed: 3 }))
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByTestId('sync-queue-row-70')

    await user.click(screen.getByRole('button', { name: 'Process Queue' }))
    expect(await screen.findByText('Processed 3 queue item(s)')).toBeInTheDocument()
  })

  it('disables pause button when queue is not paused and no active items exist', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 80, status: 'completed' })], { paused: false })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const pauseButton = await screen.findByRole('button', { name: 'Pause Queue' })
    expect(pauseButton).toBeDisabled()
  })

  it('enables pause button when queue is not paused and there are pending items', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 81, status: 'pending' })], { paused: false })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const pauseButton = await screen.findByRole('button', { name: 'Pause Queue' })
    expect(pauseButton).toBeEnabled()
  })

  it('enables resume button even when no active items exist', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 82, status: 'completed' })], { paused: true })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const resumeButton = await screen.findByRole('button', { name: 'Resume Queue' })
    expect(resumeButton).toBeEnabled()
  })

  it('shows error toast when toggling pause state fails', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 71 })], { paused: false })
      ),
      api.post('/sync/pause', () => HttpResponse.json({}, { status: 500 }))
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByRole('button', { name: 'Pause Queue' })
    await user.click(screen.getByRole('button', { name: 'Pause Queue' }))

    expect(await screen.findByText('Failed to toggle queue pause state')).toBeInTheDocument()
  })

  it('shows error toast when resolving a queue item fails', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/sync/queue', () =>
        queueResponse([
          makeSyncQueueItem({ id: 72, action: 'user_action_required', status: 'pending' }),
        ])
      ),
      api.post('/sync/queue/:id/resolve', () => HttpResponse.json({}, { status: 500 }))
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByTestId('sync-queue-row-72')

    await user.click(screen.getByRole('button', { name: 'Resolve' }))
    await screen.findByText('Resolve Queue Item')

    const resolveButtons = screen.getAllByRole('button', { name: 'Resolve' })
    await user.click(resolveButtons[resolveButtons.length - 1])

    expect(await screen.findByText('Failed to resolve queue item #72')).toBeInTheDocument()
  })

  it('disables Process Queue while the backend already has in-progress work', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 83, status: 'in_progress' })], {
          activeItems: 1,
          inProgressItems: 1,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const processButton = await screen.findByRole('button', { name: 'Processing...' })
    expect(processButton).toBeDisabled()
    expect(screen.queryByTestId('sync-queue-no-drives-hint')).not.toBeInTheDocument()
  })

  it('enables Process Queue when no backend work is in progress and active items exist', async () => {
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 84, status: 'pending' })], {
          activeItems: 1,
          inProgressItems: 0,
        })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const processButton = await screen.findByRole('button', { name: 'Process Queue' })
    expect(processButton).toBeEnabled()
  })
})
