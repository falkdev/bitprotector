import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { SyncQueuePage } from './SyncQueuePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeSyncQueueItem } from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('SyncQueuePage', () => {
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
})
