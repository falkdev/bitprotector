import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { DrivesPage } from './DrivesPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeDrivePair } from '@/test/factories'
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

  it('renders drive cards when drives are loaded', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair({ id: 1, name: 'Main Mirror' })]))
    )

    renderWithApp(<DrivesPage />)

    expect(await screen.findByText('Main Mirror')).toBeInTheDocument()
  })

  it('opens edit form when edit button is clicked', async () => {
    const user = userEvent.setup()
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair({ id: 1, name: 'My Drive' })]))
    )

    renderWithApp(<DrivesPage />)

    await screen.findByText('My Drive')
    await user.click(screen.getByTestId('edit-drive-1'))

    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()
    expect(await screen.findByTestId('drive-name-input')).toBeInTheDocument()
  })

  it('opens delete confirm dialog when delete button is clicked', async () => {
    const user = userEvent.setup()
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair({ id: 1, name: 'My Drive' })]))
    )

    renderWithApp(<DrivesPage />)

    await screen.findByText('My Drive')
    await user.click(screen.getByTestId('delete-drive-1'))

    expect(await screen.findByText(/Delete "My Drive"\?/)).toBeInTheDocument()
  })

  it('deletes drive and shows success toast', async () => {
    const user = userEvent.setup()
    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair({ id: 1, name: 'Old Drive' })])),
      api.delete('/drives/1', () => new HttpResponse(null, { status: 204 }))
    )

    renderWithApp(<DrivesPage />)

    await screen.findByText('Old Drive')
    await user.click(screen.getByTestId('delete-drive-1'))

    await user.click(await screen.findByRole('button', { name: 'Delete' }))

    await waitFor(() => {
      expect(screen.queryByText('Old Drive')).not.toBeInTheDocument()
    })
  })

  it('shows header add drive button when drives are present', async () => {
    server.use(api.get('/drives', () => HttpResponse.json([makeDrivePair({ id: 1 })])))

    renderWithApp(<DrivesPage />)

    await screen.findByTestId('add-drive-button')
  })

  it('creates a new drive pair and shows success toast', async () => {
    const user = userEvent.setup()
    const newDrive = makeDrivePair({ id: 2, name: 'New Pair' })

    server.use(
      api.get('/drives', () => HttpResponse.json([])),
      api.post('/drives', () => HttpResponse.json(newDrive, { status: 201 }))
    )

    renderWithApp(<DrivesPage />)

    await screen.findByText('No drive pairs configured')
    await user.click(screen.getByRole('button', { name: 'Add Drive Pair' }))

    await user.type(screen.getByTestId('drive-name-input'), 'New Pair')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/a')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/b')

    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(await screen.findByText('Drive pair "New Pair" created')).toBeInTheDocument()
  })

  it('updates an existing drive pair and shows success toast', async () => {
    const user = userEvent.setup()
    const drive = makeDrivePair({ id: 1, name: 'Old Name' })
    const updated = { ...drive, name: 'New Name' }

    server.use(
      api.get('/drives', () => HttpResponse.json([drive])),
      api.put('/drives/1', () => HttpResponse.json(updated))
    )

    renderWithApp(<DrivesPage />)

    await screen.findByText('Old Name')
    await user.click(screen.getByTestId('edit-drive-1'))

    const nameInput = await screen.findByTestId('drive-name-input')
    await user.clear(nameInput)
    await user.type(nameInput, 'New Name')

    await user.click(screen.getByRole('button', { name: 'Update' }))

    expect(await screen.findByText('Drive pair "New Name" updated')).toBeInTheDocument()
  })
})
