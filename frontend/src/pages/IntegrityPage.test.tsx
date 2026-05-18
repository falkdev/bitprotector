import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { IntegrityPage } from './IntegrityPage'
import { formatDate } from '@/lib/format'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import {
  makeDrivePair,
  makeIntegrityRun,
  makeIntegrityRunResult,
  makeIntegrityRunResultsResponse,
  makeSingleIntegrityResult,
} from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('IntegrityPage', () => {
  it('loads latest persisted issue rows on initial render', async () => {
    const lastCheck = '2026-03-04T15:22:00Z'

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({
              id: 77,
              status: 'completed',
              attention_files: 1,
              ended_at: lastCheck,
            }),
            results: [
              makeIntegrityRunResult({
                id: 11,
                run_id: 77,
                file_id: 11,
                relative_path: 'docs/broken.txt',
                status: 'mirror_corrupted',
                needs_attention: true,
              }),
            ],
            total: 1,
          })
        )
      )
    )

    renderWithApp(<IntegrityPage />)

    expect(await screen.findByTestId('page-title')).toHaveTextContent('Integrity')
    expect(screen.getByTestId('page-subtitle')).toHaveTextContent(
      'Run integrity checks, monitor progress, and review files that need attention.'
    )
    expect(await screen.findByTestId('integrity-last-check')).toHaveTextContent(
      'Last integrity check:'
    )
    expect(screen.getByTestId('integrity-last-check')).toHaveTextContent(formatDate(lastCheck))
    expect(await screen.findByTestId('integrity-row-11')).toHaveTextContent('docs/broken.txt')
  })

  it('disables starting a run and shows helper text when no drive pairs exist', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/drives', () => HttpResponse.json([])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: null,
            results: [],
            total: 0,
          })
        )
      )
    )

    renderWithApp(<IntegrityPage />)

    const runCheckButton = await screen.findByRole('button', { name: 'Run Check' })
    expect(runCheckButton).toBeDisabled()
    expect(await screen.findByTestId('integrity-no-drives-hint')).toHaveTextContent(
      'Add a drive pair first to run integrity checks.'
    )

    await user.click(runCheckButton)
    expect(screen.queryByText('Start Integrity Run')).not.toBeInTheDocument()
  })

  it('starts and stops a run through the dialog and action button', async () => {
    const user = userEvent.setup()
    const startedRun = makeIntegrityRun({
      id: 301,
      status: 'running',
      total_files: 20,
      processed_files: 2,
      attention_files: 0,
      trigger: 'api',
    })
    let activeRun: typeof startedRun | null = null

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () =>
        HttpResponse.json({ run: activeRun?.status === 'running' ? activeRun : null })
      ),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: null,
            results: [],
            total: 0,
          })
        )
      ),
      api.post('/integrity/runs', () => {
        activeRun = startedRun
        return HttpResponse.json(startedRun, { status: 202 })
      }),
      api.get('/integrity/runs/:id/results', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: activeRun ?? startedRun,
            results: [],
            total: 0,
          })
        )
      ),
      api.post('/integrity/runs/:id/stop', () => {
        activeRun = { ...(activeRun ?? startedRun), status: 'stopping', stop_requested: true }
        return HttpResponse.json(activeRun)
      })
    )

    renderWithApp(<IntegrityPage />)

    await user.click(await screen.findByRole('button', { name: 'Run Check' }))
    await screen.findByText('Start Integrity Run')
    await user.click(screen.getByRole('button', { name: 'Start' }))

    expect(await screen.findByText('Integrity run started')).toBeInTheDocument()
    expect(await screen.findByRole('button', { name: 'Stop' })).toBeInTheDocument()
    expect(await screen.findByText(/Integrity check running/)).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Stop' }))
    await waitFor(() => {
      expect(screen.getByText('Stop requested for run #301')).toBeInTheDocument()
    })
  })

  it('shows active workers card for running runs', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () =>
        HttpResponse.json({
          run: makeIntegrityRun({
            id: 502,
            status: 'running',
            active_workers: 4,
          }),
        })
      ),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: null,
            results: [],
            total: 0,
          })
        )
      ),
      api.get('/integrity/runs/:id/results', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({
              id: 502,
              status: 'running',
              active_workers: 4,
            }),
            results: [],
            total: 0,
          })
        )
      )
    )

    renderWithApp(<IntegrityPage />)
    expect(await screen.findByText('Files checking in parallel')).toBeInTheDocument()
    expect(screen.getByText('4')).toBeInTheDocument()
  })

  it('rechecks a file and removes it from the list when result is ok', async () => {
    const user = userEvent.setup()
    const result = makeIntegrityRunResult({
      id: 10,
      run_id: 77,
      file_id: 55,
      needs_attention: true,
    })

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({ id: 77, status: 'completed' }),
            results: [result],
            total: 1,
          })
        )
      ),
      api.post('/integrity/check/:id', () =>
        HttpResponse.json(
          makeSingleIntegrityResult({ file_id: 55, status: 'ok', recovered: false })
        )
      )
    )

    renderWithApp(<IntegrityPage />)
    expect(await screen.findByTestId('integrity-row-55')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Recheck' }))

    await waitFor(() => {
      expect(screen.queryByTestId('integrity-row-55')).not.toBeInTheDocument()
    })
    expect(await screen.findByText('File #55 rechecked')).toBeInTheDocument()
  })

  it('rechecks a file and updates status in place when still failing', async () => {
    const user = userEvent.setup()
    const result = makeIntegrityRunResult({
      id: 10,
      run_id: 77,
      file_id: 55,
      needs_attention: true,
    })

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({ id: 77, status: 'completed' }),
            results: [result],
            total: 1,
          })
        )
      ),
      api.post('/integrity/check/:id', () =>
        HttpResponse.json(
          makeSingleIntegrityResult({ file_id: 55, status: 'mirror_corrupted', recovered: false })
        )
      )
    )

    renderWithApp(<IntegrityPage />)
    expect(await screen.findByTestId('integrity-row-55')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Recheck' }))

    await waitFor(() => {
      expect(screen.getByText('File #55 rechecked')).toBeInTheDocument()
    })
    expect(screen.getByTestId('integrity-row-55')).toBeInTheDocument()
  })

  it('shows error toast when recheck fails', async () => {
    const user = userEvent.setup()
    const result = makeIntegrityRunResult({
      id: 10,
      run_id: 77,
      file_id: 55,
      needs_attention: true,
    })

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({ id: 77, status: 'completed' }),
            results: [result],
            total: 1,
          })
        )
      ),
      api.post('/integrity/check/:id', () => HttpResponse.json({}, { status: 500 }))
    )

    renderWithApp(<IntegrityPage />)
    expect(await screen.findByTestId('integrity-row-55')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Recheck' }))

    await waitFor(() => {
      expect(screen.getByText('Failed to recheck file #55')).toBeInTheDocument()
    })
  })

  it('shows Load More button when hasMore is true and loads more on click', async () => {
    const user = userEvent.setup()
    const firstResult = makeIntegrityRunResult({
      id: 20,
      run_id: 77,
      file_id: 100,
      needs_attention: true,
    })
    const secondResult = makeIntegrityRunResult({
      id: 21,
      run_id: 77,
      file_id: 101,
      needs_attention: true,
    })

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/runs/active', () => HttpResponse.json({ run: null })),
      api.get('/integrity/runs/latest', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({ id: 77, status: 'completed' }),
            results: [firstResult],
            total: 2,
            per_page: 1,
          })
        )
      ),
      api.get('/integrity/runs/:id/results', () =>
        HttpResponse.json(
          makeIntegrityRunResultsResponse({
            run: makeIntegrityRun({ id: 77, status: 'completed' }),
            results: [secondResult],
            total: 2,
            per_page: 1,
          })
        )
      )
    )

    renderWithApp(<IntegrityPage />)
    expect(await screen.findByTestId('integrity-row-100')).toBeInTheDocument()

    const loadMoreBtn = screen.getByRole('button', { name: 'Load More' })
    expect(loadMoreBtn).toBeInTheDocument()

    await user.click(loadMoreBtn)
    expect(await screen.findByTestId('integrity-row-101')).toBeInTheDocument()
  })
})
