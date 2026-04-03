import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { VirtualPathManagerPage } from './VirtualPathManagerPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import {
  makeBulkAssignResult,
  makeDrivePair,
  makeTrackingItem,
  makeTrackingListResponse,
} from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('VirtualPathManagerPage', () => {
  it('generates publish paths from real paths with mocked backend responses', async () => {
    const user = userEvent.setup()
    let requestBody: unknown = null

    server.use(
      api.get('/tracking/items', () =>
        HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({ id: 5, kind: 'file', path: 'documents/report.pdf' }),
          ])
        )
      ),
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.post('/virtual-paths/bulk-from-real', async ({ request }) => {
        requestBody = await request.json()
        return HttpResponse.json(makeBulkAssignResult({ succeeded: [5] }))
      })
    )

    renderWithApp(<VirtualPathManagerPage />)

    await screen.findByTestId('virtual-path-row-5')
    await user.selectOptions(screen.getByLabelText('Drive Pair'), '1')
    await user.type(screen.getByLabelText('Folder Path'), 'documents')
    await user.type(screen.getByLabelText('Publish Root'), '/published/documents')
    await user.click(screen.getByRole('button', { name: 'Generate Publish Paths' }))

    expect(await screen.findByText('Generated 1 publish path(s) from real paths')).toBeInTheDocument()
    expect(requestBody).toEqual({
      drive_pair_id: 1,
      folder_path: 'documents',
      publish_root: '/published/documents',
    })
  })

  it('shows a validation toast for malformed bulk assignment input', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/tracking/items', () =>
        HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({ id: 5, kind: 'file', path: 'documents/report.pdf' }),
          ])
        )
      ),
      api.get('/drives', () => HttpResponse.json([makeDrivePair()]))
    )

    renderWithApp(<VirtualPathManagerPage />)

    await screen.findByTestId('virtual-path-row-5')
    await user.type(screen.getByLabelText('Bulk Assignments'), 'bad-line-without-separator')
    await user.click(screen.getByRole('button', { name: 'Apply Publish Paths' }))

    expect(await screen.findByText(/Invalid bulk assignment line:/)).toBeInTheDocument()
  })
})
