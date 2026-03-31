import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { IntegrityPage } from './IntegrityPage'
import { api, apiError } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeDrivePair, makeIntegrityResult, makeSingleIntegrityResult } from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('IntegrityPage', () => {
  it('runs batch checks and rechecks individual files', async () => {
    const user = userEvent.setup()
    let recheckRequested = false

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/check-all', () =>
        HttpResponse.json({
          results: [makeIntegrityResult({ file_id: 11, status: 'mirror_corrupted' })],
        })
      ),
      api.post('/integrity/check/11', () => {
        recheckRequested = true
        return HttpResponse.json(
          makeSingleIntegrityResult({ file_id: 11, status: 'ok', recovered: true })
        )
      })
    )

    renderWithApp(<IntegrityPage />)

    await user.click(await screen.findByRole('button', { name: 'Run Check' }))

    expect(await screen.findByTestId('integrity-row-11')).toHaveTextContent('11')

    await user.click(screen.getByRole('button', { name: 'Recheck' }))

    expect(await screen.findByText('File #11 rechecked')).toBeInTheDocument()
    expect(recheckRequested).toBe(true)
  })

  it('shows an error toast when the batch check fails', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/integrity/check-all', () => apiError(500, 'Integrity service unavailable'))
    )

    renderWithApp(<IntegrityPage />)

    await user.click(await screen.findByRole('button', { name: 'Run Check' }))

    expect(await screen.findByText('Integrity check failed')).toBeInTheDocument()
  })
})
