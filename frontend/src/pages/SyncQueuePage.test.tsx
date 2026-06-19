import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SyncQueuePage } from './SyncQueuePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeDrivePair, makeSyncQueueItem, makeSyncSummary } from '@/test/factories'
import { renderWithApp } from '@/test/render'
import type { SyncSummary } from '@/types/sync'

// ── SSE stream helpers ────────────────────────────────────────────────────

function makeSseStream(summaries: SyncSummary[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder()
  const chunks = summaries.map((s) => encoder.encode(`data: ${JSON.stringify(s)}\n\n`))

  let index = 0
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index < chunks.length) {
        controller.enqueue(chunks[index++])
      } else {
        controller.close()
      }
    },
  })
}

function mockSseStream(...summaries: SyncSummary[]) {
  const realFetch = globalThis.fetch
  const spy = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url
    if (url.includes('/sync/queue/stream')) {
      return new Response(makeSseStream(summaries), {
        status: 200,
        headers: { 'content-type': 'text/event-stream' },
      })
    }
    return realFetch(input, init)
  })
  return spy
}

// ── Queue REST response builder ────────────────────────────────────────────

interface QueueResponseOptions {
  total?: number
  page?: number
  perPage?: number
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
    queue_paused: false,
    active_items: items.filter((i) => i.status === 'pending' || i.status === 'in_progress').length,
    in_progress_items: items.filter((i) => i.status === 'in_progress').length,
    pending_items: items.filter((i) => i.status === 'pending').length,
    completed_items: items.filter((i) => i.status === 'completed').length,
    failed_items: items.filter((i) => i.status === 'failed').length,
  })

// ── Suite setup ────────────────────────────────────────────────────────────

describe('SyncQueuePage', () => {
  beforeEach(() => {
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/sync/queue', () => queueResponse([]))
    )
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  // ── Core rendering ────────────────────────────────────────────────────

  it('renders the empty state when no queue items are returned', async () => {
    mockSseStream(makeSyncSummary())

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText('No queue items')).toBeInTheDocument()
  })

  it('shows process queue and pause queue buttons', async () => {
    mockSseStream(makeSyncSummary({ total: 1, pending_items: 1 }))
    server.use(api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 11 })])))

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('button', { name: 'Process Queue' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Clear Completed' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Pause Queue' })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /All/i })).toBeInTheDocument()
  })

  // ── Tab counts driven by SSE summary ─────────────────────────────────

  it('renders status tabs with counts from SSE summary', async () => {
    mockSseStream(
      makeSyncSummary({
        total: 13,
        pending_items: 6,
        in_progress_items: 1,
        completed_items: 4,
        failed_items: 2,
      })
    )
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 70, status: 'pending' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('tab', { name: /All 13/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /Pending 6/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /In Progress 1/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /Completed 4/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /Failed 2/ })).toBeInTheDocument()
  })

  it('"All" tab count equals summary.total', async () => {
    mockSseStream(makeSyncSummary({ total: 7, pending_items: 7 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 52, status: 'pending' })], { total: 1 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('tab', { name: /All 7/ })).toBeInTheDocument()
  })

  it('tab counts stay consistent after switching the active filter tab', async () => {
    const user = userEvent.setup()
    mockSseStream(
      makeSyncSummary({ total: 11, pending_items: 6, in_progress_items: 1, completed_items: 4 })
    )
    server.use(
      api.get('/sync/queue', ({ request }) => {
        const status = new URL(request.url).searchParams.get('status')
        if (status === 'completed') {
          return queueResponse([makeSyncQueueItem({ id: 71, status: 'completed' })], { total: 4 })
        }
        return queueResponse([makeSyncQueueItem({ id: 73, status: 'pending' })], { total: 11 })
      })
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByRole('tab', { name: /All 11/ })

    await user.click(screen.getByRole('tab', { name: /Completed/ }))

    expect(screen.getByRole('tab', { name: /All 11/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /In Progress 1/ })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /Completed 4/ })).toBeInTheDocument()
  })

  // ── Processing indicator driven by SSE ────────────────────────────────

  it('shows Processing queue… indicator when summary.processing_active is true', async () => {
    mockSseStream(makeSyncSummary({ processing_active: true, in_progress_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 75, status: 'in_progress' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('processing-indicator')).toBeInTheDocument()
  })

  it('hides Processing queue… indicator when summary.processing_active is false', async () => {
    mockSseStream(makeSyncSummary({ processing_active: false, pending_items: 8, total: 8 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 78, status: 'pending' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-78')
    expect(screen.queryByTestId('processing-indicator')).not.toBeInTheDocument()
  })

  it('hides Processing queue… indicator when processing_active but queue is paused', async () => {
    mockSseStream(
      makeSyncSummary({ processing_active: true, queue_paused: true, pending_items: 1, total: 1 })
    )
    server.use(api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 77 })])))

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-77')
    expect(screen.queryByTestId('processing-indicator')).not.toBeInTheDocument()
  })

  // ── Scan indicator driven by SSE ─────────────────────────────────────

  it('shows scan indicator when summary.scanning is true', async () => {
    mockSseStream(
      makeSyncSummary({
        scanning: true,
        scan_total_files: 10,
        scan_scanned_files: 3,
        scan_active_folders: 1,
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText(/Updating… 3 \/ 10 files across 1 folder/)).toBeInTheDocument()
  })

  it('hides scan indicator when summary.scanning is false', async () => {
    mockSseStream(makeSyncSummary({ scanning: false }))

    renderWithApp(<SyncQueuePage />)

    await screen.findByText(/Showing/)
    expect(screen.queryByText(/Updating…/)).not.toBeInTheDocument()
  })

  it('shows scan indicator across multiple folders', async () => {
    mockSseStream(
      makeSyncSummary({
        scanning: true,
        scan_total_files: 35,
        scan_scanned_files: 15,
        scan_active_folders: 2,
      })
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText(/Updating… 15 \/ 35 files across 2 folder/)).toBeInTheDocument()
  })

  it('disables Next, filters, pause, and process controls while scanning is active', async () => {
    mockSseStream(
      makeSyncSummary({
        scanning: true,
        scan_total_files: 10,
        scan_scanned_files: 3,
        scan_active_folders: 1,
        total: 120,
        active_items: 120,
        pending_items: 120,
      })
    )
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 88, status: 'pending' })], { total: 120 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-88')

    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Process Queue' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Pause Queue' })).toBeDisabled()

    expect(screen.getByRole('tab', { name: /All/i })).toBeDisabled()
    expect(screen.getByRole('tab', { name: /Pending/i })).toBeDisabled()
    expect(screen.getByRole('tab', { name: /In Progress/i })).toBeDisabled()
    expect(screen.getByRole('tab', { name: /Completed/i })).toBeDisabled()
    expect(screen.getByRole('tab', { name: /Failed/i })).toBeDisabled()
  })

  it('re-enables controls as soon as scanning stops', async () => {
    mockSseStream(
      makeSyncSummary({
        revision: 1,
        scanning: true,
        scan_total_files: 10,
        scan_scanned_files: 9,
        scan_active_folders: 1,
        total: 120,
        active_items: 120,
        pending_items: 120,
      }),
      makeSyncSummary({
        revision: 2,
        scanning: false,
        scan_total_files: 10,
        scan_scanned_files: 10,
        scan_active_folders: 0,
        total: 120,
        active_items: 120,
        pending_items: 120,
      })
    )
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 89, status: 'pending' })], { total: 120 })
      )
    )

    renderWithApp(<SyncQueuePage />)
    await screen.findByTestId('sync-queue-row-89')

    await screen.findByText('Showing 1 of 120 (filter: All)')
    expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled()
  })

  // ── Queue paused state driven by SSE ─────────────────────────────────

  it('shows paused banner when summary.queue_paused is true', async () => {
    mockSseStream(makeSyncSummary({ queue_paused: true, total: 1, active_items: 1 }))
    server.use(api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 40 })])))

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByTestId('queue-paused-banner')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Resume Queue' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Pause Queue' })).not.toBeInTheDocument()
  })

  it('hides paused banner when summary.queue_paused is false', async () => {
    mockSseStream(makeSyncSummary({ queue_paused: false, total: 1 }))
    server.use(api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 41 })])))

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-41')
    expect(screen.queryByTestId('queue-paused-banner')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Pause Queue' })).toBeInTheDocument()
  })

  // ── Pause / resume actions ─────────────────────────────────────────────

  it('pauses the queue when pause button is clicked', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ total: 1, active_items: 1 }))
    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 42 })])),
      api.post('/sync/pause', () => HttpResponse.json({ queue_paused: true }))
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Pause Queue' })
    await user.click(screen.getByRole('button', { name: 'Pause Queue' }))

    expect(await screen.findByText('Sync queue processing paused')).toBeInTheDocument()
  })

  it('resumes the queue when resume button is clicked', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ queue_paused: true, total: 1, active_items: 1 }))
    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 43 })])),
      api.post('/sync/resume', () => HttpResponse.json({ queue_paused: false }))
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Resume Queue' })
    await user.click(screen.getByRole('button', { name: 'Resume Queue' }))

    expect(await screen.findByText('Sync queue processing resumed')).toBeInTheDocument()
  })

  it('shows error toast when toggling pause state fails', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ total: 1, active_items: 1 }))
    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 71 })])),
      api.post('/sync/pause', () => HttpResponse.json({}, { status: 500 }))
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Pause Queue' })
    await user.click(screen.getByRole('button', { name: 'Pause Queue' }))

    expect(await screen.findByText('Failed to toggle queue pause state')).toBeInTheDocument()
  })

  it('disables pause button when queue is not paused and no active items exist', async () => {
    mockSseStream(makeSyncSummary({ active_items: 0, completed_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 80, status: 'completed' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const pauseButton = await screen.findByRole('button', { name: 'Pause Queue' })
    expect(pauseButton).toBeDisabled()
  })

  it('enables pause button when queue is not paused and there are pending items', async () => {
    mockSseStream(makeSyncSummary({ active_items: 1, pending_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 81, status: 'pending' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const pauseButton = await screen.findByRole('button', { name: 'Pause Queue' })
    expect(pauseButton).toBeEnabled()
  })

  it('enables resume button even when no active items exist', async () => {
    mockSseStream(makeSyncSummary({ queue_paused: true, completed_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 82, status: 'completed' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const resumeButton = await screen.findByRole('button', { name: 'Resume Queue' })
    expect(resumeButton).toBeEnabled()
  })

  // ── Process Queue action ───────────────────────────────────────────────

  it('shows "Processing started" toast when Process Queue is clicked', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ total: 1, pending_items: 1 }))
    server.use(
      api.get('/sync/queue', () => queueResponse([makeSyncQueueItem({ id: 70 })])),
      api.post('/sync/process', () => HttpResponse.json({ status: 'started' }, { status: 202 }))
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByRole('button', { name: 'Process Queue' })
    await user.click(screen.getByRole('button', { name: 'Process Queue' }))

    expect(await screen.findByText('Processing started')).toBeInTheDocument()
  })

  it('disables process queue button when no drive pairs exist', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json([])),
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 15, status: 'completed' })])
      )
    )
    mockSseStream(makeSyncSummary({ completed_items: 1, total: 1 }))

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByRole('button', { name: 'Process Queue' })).toBeDisabled()
    expect(await screen.findByTestId('sync-queue-no-drives-hint')).toHaveTextContent(
      'Add a drive pair first to process the sync queue.'
    )
    expect(screen.getByRole('button', { name: 'Clear Completed' })).toBeEnabled()
  })

  it('disables process queue and shows Processing button while processing_active', async () => {
    mockSseStream(makeSyncSummary({ processing_active: true, in_progress_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 83, status: 'in_progress' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const processButton = await screen.findByRole('button', { name: 'Processing...' })
    expect(processButton).toBeDisabled()
    expect(screen.queryByTestId('sync-queue-no-drives-hint')).not.toBeInTheDocument()
  })

  it('enables Process Queue when not processing and active items exist', async () => {
    mockSseStream(makeSyncSummary({ active_items: 1, pending_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 84, status: 'pending' })])
      )
    )

    renderWithApp(<SyncQueuePage />)

    const processButton = await screen.findByRole('button', { name: 'Process Queue' })
    expect(processButton).toBeEnabled()
  })

  // ── Clear Completed action ─────────────────────────────────────────────

  it('disables clear completed button when there are no completed items', async () => {
    mockSseStream(makeSyncSummary({ pending_items: 1, total: 1 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 12, status: 'pending' })], { total: 1 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    const clearButton = await screen.findByRole('button', { name: 'Clear Completed' })
    expect(clearButton).toBeDisabled()
  })

  it('enables clear completed button when completed items exist', async () => {
    mockSseStream(makeSyncSummary({ completed_items: 2, pending_items: 1, total: 3 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 13, status: 'pending' })])
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

    mockSseStream(makeSyncSummary({ completed_items: 1, pending_items: 1, total: 2 }))
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
    mockSseStream(makeSyncSummary({ completed_items: 1, total: 1 }))
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

  // ── Filter tabs ───────────────────────────────────────────────────────

  it('filters queue items by requesting a filtered page from the server', async () => {
    const user = userEvent.setup()
    const requestedStatuses: Array<string | null> = []

    mockSseStream(makeSyncSummary({ total: 2, pending_items: 1, completed_items: 1 }))
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

    await user.click(screen.getByRole('tab', { name: /Pending/ }))

    await waitFor(() => {
      expect(requestedStatuses).toContain('pending')
      expect(screen.getByTestId('sync-queue-row-50')).toBeInTheDocument()
      expect(screen.queryByTestId('sync-queue-row-51')).not.toBeInTheDocument()
    })
  })

  // ── Resolve dialog ────────────────────────────────────────────────────

  it('resolves a pending manual action through the dialog', async () => {
    const user = userEvent.setup()
    let item = makeSyncQueueItem({
      id: 7,
      tracked_file_id: 44,
      action: 'user_action_required',
      status: 'pending',
    })
    let resolutionBody: unknown = null

    mockSseStream(makeSyncSummary({ total: 1, pending_items: 1 }))
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

  it('closes the resolve dialog when cancel is clicked', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ total: 1, pending_items: 1 }))
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

  it('shows error toast when resolving a queue item fails', async () => {
    const user = userEvent.setup()
    mockSseStream(makeSyncSummary({ total: 1, pending_items: 1 }))
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

  // ── Pagination ────────────────────────────────────────────────────────

  it('requests the next page when Next is clicked', async () => {
    const user = userEvent.setup()
    const requestedPages: number[] = []

    mockSseStream(makeSyncSummary({ total: 2 }))
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

  it('shows the server-reported total count in the summary label', async () => {
    mockSseStream(makeSyncSummary({ total: 7, pending_items: 7 }))
    server.use(
      api.get('/sync/queue', () =>
        queueResponse([makeSyncQueueItem({ id: 52, status: 'pending' })], { total: 7 })
      )
    )

    renderWithApp(<SyncQueuePage />)

    expect(await screen.findByText('Showing 1 of 7 (filter: All)')).toBeInTheDocument()
  })

  // ── SSE revision triggers item refetch ────────────────────────────────

  it('refetches items when SSE revision increases', async () => {
    let listCalls = 0
    mockSseStream(
      makeSyncSummary({ revision: 1, total: 1 }),
      makeSyncSummary({ revision: 2, total: 1 })
    )
    server.use(
      api.get('/sync/queue', () => {
        listCalls += 1
        return queueResponse([makeSyncQueueItem({ id: 99 })])
      })
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-99')

    await waitFor(() => {
      expect(listCalls).toBeGreaterThanOrEqual(2)
    })
  })

  it('keeps Next enabled during background SSE refetch', async () => {
    let listCalls = 0
    mockSseStream(
      makeSyncSummary({ revision: 1, total: 120, pending_items: 120 }),
      makeSyncSummary({ revision: 2, total: 120, pending_items: 120 })
    )
    server.use(
      api.get('/sync/queue', async () => {
        listCalls += 1
        if (listCalls >= 2) {
          await new Promise((resolve) => setTimeout(resolve, 80))
        }
        return queueResponse([makeSyncQueueItem({ id: 101 })], { total: 120 })
      })
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-101')

    // During the second (background) fetch, pagination controls should remain
    // stable and not flicker disabled/enabled.
    await waitFor(() => {
      expect(listCalls).toBeGreaterThanOrEqual(2)
    })
    expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled()
  })

  it('does not issue duplicate fetches for the same revision', async () => {
    let listCalls = 0
    mockSseStream(
      makeSyncSummary({ revision: 5, total: 1 }),
      makeSyncSummary({ revision: 5, total: 1 })
    )
    server.use(
      api.get('/sync/queue', () => {
        listCalls += 1
        return queueResponse([makeSyncQueueItem({ id: 100 })])
      })
    )

    renderWithApp(<SyncQueuePage />)

    await screen.findByTestId('sync-queue-row-100')

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50))
    })

    // Initial fetch (revision -1 → 5) + no second fetch for duplicate revision 5.
    expect(listCalls).toBeLessThanOrEqual(2)
  })
})
