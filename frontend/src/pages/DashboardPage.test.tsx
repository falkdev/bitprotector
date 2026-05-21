import { screen } from '@testing-library/react'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DashboardPage } from './DashboardPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeSystemStatus } from '@/test/factories'
import { renderWithApp } from '@/test/render'

const defaultHandlers = () => [
  api.get('/status', () => HttpResponse.json(makeSystemStatus({ drive_pairs: 2 }))),
  api.get('/logs', () => HttpResponse.json({ logs: [], total: 0, page: 1, per_page: 10 })),
]

describe('DashboardPage', () => {
  it('renders the status overview after loading', async () => {
    server.use(...defaultHandlers())

    renderWithApp(<DashboardPage />)

    expect(await screen.findByTestId('status-metric-files-tracked')).toBeInTheDocument()
  })

  it('shows loading spinner while status is loading then renders content', async () => {
    server.use(...defaultHandlers())

    renderWithApp(<DashboardPage />)

    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(await screen.findByTestId('status-metric-files-tracked')).toBeInTheDocument()
  })
})
