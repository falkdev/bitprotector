import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { LogsPage } from './LogsPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeLogEntry } from '@/test/factories'
import { renderWithApp } from '@/test/render'

function makeEntries(count: number) {
  return Array.from({ length: count }, (_, i) =>
    makeLogEntry({ id: i + 1, message: `msg ${i + 1}` })
  )
}

describe('LogsPage', () => {
  it('applies filters and expands log details', async () => {
    const user = userEvent.setup()
    let requestedFileId: string | null = null

    server.use(
      api.get('/logs', ({ request }) => {
        requestedFileId = new URL(request.url).searchParams.get('file_id')
        return HttpResponse.json({
          logs: [
            makeLogEntry({
              id: 9,
              tracked_file_id: 42,
              message: 'Mirror completed',
              details: '{"result":"ok"}',
            }),
          ],
          total: 1,
          page: 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)

    expect(await screen.findByTestId('log-row-9')).toBeInTheDocument()

    await user.type(screen.getByLabelText('File ID'), '42')
    await user.click(screen.getByRole('button', { name: 'Apply' }))
    await user.click(screen.getByRole('button', { name: 'View' }))

    expect(await screen.findByText('Log Entry #9')).toBeInTheDocument()
    expect(screen.getByText('result')).toBeInTheDocument()
    expect(screen.getByText('ok')).toBeInTheDocument()
    expect(requestedFileId).toBe('42')
  })

  it('renders the empty state when the backend returns no matching entries', async () => {
    server.use(
      api.get('/logs', () =>
        HttpResponse.json({
          logs: [],
          total: 0,
          page: 1,
          per_page: 25,
        })
      )
    )

    renderWithApp(<LogsPage />)

    expect(await screen.findByText('No matching log entries')).toBeInTheDocument()
  })

  it('shows error toast when the API fails', async () => {
    server.use(
      api.get('/logs', () => HttpResponse.json({ error: 'server error' }, { status: 500 }))
    )

    renderWithApp(<LogsPage />)

    expect(await screen.findByText('Failed to load event logs')).toBeInTheDocument()
  })

  it('shows pagination buttons when a full page is returned', async () => {
    const entries = makeEntries(25)
    server.use(
      api.get('/logs', () => HttpResponse.json({ logs: entries, total: 50, page: 1, per_page: 25 }))
    )

    renderWithApp(<LogsPage />)

    expect(await screen.findByTestId('log-row-1')).toBeInTheDocument()
    const nextButton = screen.getByRole('button', { name: /Next/ })
    expect(nextButton).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /Previous/ })).toBeDisabled()
  })

  it('navigates to page 2 when Next is clicked', async () => {
    const user = userEvent.setup()
    const page1 = makeEntries(25)
    const page2 = [makeLogEntry({ id: 100, message: 'page2 entry' })]
    let capturedPage: string | null = null

    server.use(
      api.get('/logs', ({ request }) => {
        capturedPage = new URL(request.url).searchParams.get('page')
        const isPage2 = capturedPage === '2'
        return HttpResponse.json({
          logs: isPage2 ? page2 : page1,
          total: 26,
          page: isPage2 ? 2 : 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)

    await screen.findByTestId('log-row-1')
    await user.click(screen.getByRole('button', { name: /Next/ }))

    expect(await screen.findByText('page2 entry')).toBeInTheDocument()
  })

  it('resets filters back to defaults', async () => {
    const user = userEvent.setup()
    let capturedEventType: string | null = null

    server.use(
      api.get('/logs', ({ request }) => {
        capturedEventType = new URL(request.url).searchParams.get('event_type')
        return HttpResponse.json({
          logs: [makeLogEntry({ id: 1 })],
          total: 1,
          page: 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)

    await screen.findByTestId('log-row-1')

    // Apply a filter
    await user.selectOptions(screen.getByLabelText('Event Type'), 'integrity_fail')
    await user.click(screen.getByRole('button', { name: 'Apply' }))

    // Now reset
    await user.click(screen.getByRole('button', { name: 'Reset' }))

    await waitFor(() => {
      expect(capturedEventType).toBeNull()
    })
  })

  it('expands a log entry with raw (non-JSON) details', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/logs', () =>
        HttpResponse.json({
          logs: [
            makeLogEntry({ id: 5, message: 'raw detail entry', details: 'plain text details' }),
          ],
          total: 1,
          page: 1,
          per_page: 25,
        })
      )
    )

    renderWithApp(<LogsPage />)

    await screen.findByTestId('log-row-5')
    await user.click(screen.getByRole('button', { name: 'View' }))

    expect(await screen.findByText('plain text details')).toBeInTheDocument()
  })

  it('navigates back to page 1 when Previous is clicked from page 2', async () => {
    const user = userEvent.setup()
    const page1 = makeEntries(25)
    const page2 = [makeLogEntry({ id: 100, message: 'page2 entry' })]

    server.use(
      api.get('/logs', ({ request }) => {
        const p = new URL(request.url).searchParams.get('page')
        const isPage2 = p === '2'
        return HttpResponse.json({
          logs: isPage2 ? page2 : page1,
          total: 26,
          page: isPage2 ? 2 : 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)

    await screen.findByTestId('log-row-1')
    await user.click(screen.getByRole('button', { name: /Next/ }))
    expect(await screen.findByText('page2 entry')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /Previous/ }))
    expect(await screen.findByTestId('log-row-1')).toBeInTheDocument()
  })

  it('clears the expanded entry when navigating to a page where it does not appear', async () => {
    const user = userEvent.setup()
    const page1 = makeEntries(25)
    const page2 = [makeLogEntry({ id: 100, message: 'page2 only entry' })]

    server.use(
      api.get('/logs', ({ request }) => {
        const p = new URL(request.url).searchParams.get('page')
        const isPage2 = p === '2'
        return HttpResponse.json({
          logs: isPage2 ? page2 : page1,
          total: 26,
          page: isPage2 ? 2 : 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)
    await screen.findByTestId('log-row-1')
    // Expand first entry
    await user.click(screen.getAllByRole('button', { name: 'View' })[0])
    // The expanded details section appears (message = "msg 1" from makeEntries)
    expect(await screen.findAllByText('msg 1')).toBeTruthy()

    // Navigate to page 2 — expanded log (id=1) won't exist on page 2
    await user.click(screen.getByRole('button', { name: /Next/ }))
    expect(await screen.findByText('page2 only entry')).toBeInTheDocument()
    // The expanded details (from page 1) should no longer be shown
    expect(screen.queryByTestId('log-row-1')).not.toBeInTheDocument()
  })

  it('updates from/to date filter inputs and submits them', async () => {
    const user = userEvent.setup()
    let capturedFrom: string | null = null
    let capturedTo: string | null = null

    server.use(
      api.get('/logs', ({ request }) => {
        const url = new URL(request.url)
        capturedFrom = url.searchParams.get('from')
        capturedTo = url.searchParams.get('to')
        return HttpResponse.json({
          logs: [makeLogEntry({ id: 1 })],
          total: 1,
          page: 1,
          per_page: 25,
        })
      })
    )

    renderWithApp(<LogsPage />)
    await screen.findByTestId('log-row-1')

    await user.type(screen.getByLabelText('From'), '2024-01-01T00:00')
    await user.type(screen.getByLabelText('To'), '2024-12-31T23:59')
    await user.click(screen.getByRole('button', { name: 'Apply' }))

    await waitFor(() => {
      expect(capturedFrom).toBeTruthy()
      expect(capturedTo).toBeTruthy()
    })
  })

  it('shows "No additional details" when log entry has null details', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/logs', () =>
        HttpResponse.json({
          logs: [makeLogEntry({ id: 99, message: 'Simple event', details: null })],
          total: 1,
          page: 1,
          per_page: 25,
        })
      )
    )

    renderWithApp(<LogsPage />)
    await screen.findByTestId('log-row-99')

    await user.click(screen.getByRole('button', { name: 'View' }))
    expect(await screen.findByText('No additional details')).toBeInTheDocument()
  })

  it('shows file_path in expanded log entry and "—" when both file_path and tracked_file_id are null', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/logs', () =>
        HttpResponse.json({
          logs: [
            makeLogEntry({
              id: 101,
              file_path: '/mnt/primary/docs/report.pdf',
              tracked_file_id: 42,
            }),
            makeLogEntry({ id: 102, file_path: null, tracked_file_id: null }),
          ],
          total: 2,
          page: 1,
          per_page: 25,
        })
      )
    )

    renderWithApp(<LogsPage />)
    await screen.findByTestId('log-row-101')

    // Row with file_path shows the path
    expect(screen.getByText('/mnt/primary/docs/report.pdf')).toBeInTheDocument()

    // Row with no file_path and no tracked_file_id shows "—"
    expect(screen.getAllByText('—').length).toBeGreaterThan(0)

    // Expand entry with file_path to cover lines 379-383
    await user.click(screen.getAllByRole('button', { name: 'View' })[0])
    expect(await screen.findByText('Log Entry #101')).toBeInTheDocument()
    expect(screen.getAllByText('/mnt/primary/docs/report.pdf').length).toBeGreaterThan(0)
  })
})
