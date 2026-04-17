import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { LogsPage } from './LogsPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeLogEntry } from '@/test/factories'
import { renderWithApp } from '@/test/render'

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
})
