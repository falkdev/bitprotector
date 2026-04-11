import { screen } from '@testing-library/react'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DashboardPage } from './DashboardPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeSystemStatus } from '@/test/factories'
import { renderWithApp } from '@/test/render'

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
})
