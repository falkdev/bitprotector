import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DrivesPage } from './DrivesPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { renderWithApp } from '@/test/render'

describe('DrivesPage', () => {
  it('renders one add button in empty state and opens create form from it', async () => {
    const user = userEvent.setup()

    server.use(api.get('/drives', () => HttpResponse.json([])))

    renderWithApp(<DrivesPage />)

    expect(await screen.findByText('No drive pairs configured')).toBeInTheDocument()

    const addButtons = screen.getAllByRole('button', { name: 'Add Drive Pair' })
    expect(addButtons).toHaveLength(1)

    await user.click(addButtons[0])
    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()
    expect(await screen.findByTestId('drive-name-input')).toBeInTheDocument()
  })
})
