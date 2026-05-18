import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DriveForm } from './DriveForm'

vi.mock('@/components/shared/PathPickerDialog', () => ({
  PathPickerDialog: ({ open, onPick }: { open: boolean; onPick: (path: string) => void }) =>
    open ? (
      <button type="button" onClick={() => onPick('/mnt/picked-directory')}>
        Use Mock Directory
      </button>
    ) : null,
}))

describe('DriveForm', () => {
  it('fills the selected drive path from the picker', async () => {
    const user = userEvent.setup()

    render(<DriveForm onClose={() => {}} onSave={vi.fn().mockResolvedValue(undefined)} />)

    await user.click(screen.getAllByRole('button', { name: 'Browse' })[0])
    await user.click(screen.getByRole('button', { name: 'Use Mock Directory' }))

    expect(screen.getByTestId('drive-primary-path-input')).toHaveValue('/mnt/picked-directory')
  })

  it('submits skip validation for new drive pairs when selected', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)

    render(<DriveForm onClose={() => {}} onSave={onSave} />)

    await user.type(screen.getByTestId('drive-name-input'), 'Mirror A')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/primary')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/secondary')
    await user.click(
      screen.getByRole('checkbox', { name: 'Skip path validation when creating this drive pair' })
    )
    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(onSave).toHaveBeenCalledWith({
      name: 'Mirror A',
      primary_path: '/mnt/primary',
      secondary_path: '/mnt/secondary',
      primary_media_type: 'hdd',
      secondary_media_type: 'hdd',
      skip_validation: true,
    })
  })

  it('shows the backend error when save fails', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValue({
      isAxiosError: true,
      message: 'Request failed with status code 400',
      response: {
        data: {
          error: {
            message: 'Primary path does not exist: /mnt/primary',
          },
        },
      },
    })

    render(<DriveForm onClose={() => {}} onSave={onSave} />)

    await user.type(screen.getByTestId('drive-name-input'), 'Mirror A')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/primary')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/secondary')
    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(await screen.findByText('Primary path does not exist: /mnt/primary')).toBeInTheDocument()
  })

  it('shows axios message as fallback when API response has no error message', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValue({
      isAxiosError: true,
      message: 'Network Error',
      response: { data: {} },
    })

    render(<DriveForm onClose={() => {}} onSave={onSave} />)

    await user.type(screen.getByTestId('drive-name-input'), 'Mirror A')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/primary')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/secondary')
    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(await screen.findByText('Network Error')).toBeInTheDocument()
  })

  it('shows generic error message when save rejects with a plain Error', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValue(new Error('Unexpected failure'))

    render(<DriveForm onClose={() => {}} onSave={onSave} />)

    await user.type(screen.getByTestId('drive-name-input'), 'Mirror A')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/primary')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/secondary')
    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(await screen.findByText('Unexpected failure')).toBeInTheDocument()
  })

  it('fills the secondary drive path from the picker', async () => {
    const user = userEvent.setup()

    render(<DriveForm onClose={() => {}} onSave={vi.fn().mockResolvedValue(undefined)} />)

    await user.click(screen.getAllByRole('button', { name: 'Browse' })[1])
    await user.click(screen.getByRole('button', { name: 'Use Mock Directory' }))

    expect(screen.getByTestId('drive-secondary-path-input')).toHaveValue('/mnt/picked-directory')
  })

  it('renders in edit mode with Update button when initial drive is provided', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)
    const initial = {
      id: 5,
      name: 'Existing Drive',
      primary_path: '/mnt/primary',
      secondary_path: '/mnt/secondary',
      primary_media_type: 'hdd' as const,
      secondary_media_type: 'ssd' as const,
      primary_state: 'active' as const,
      secondary_state: 'active' as const,
      active_role: 'primary' as const,
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
    }

    render(<DriveForm initial={initial} onClose={() => {}} onSave={onSave} />)

    expect(screen.getByRole('button', { name: 'Update' })).toBeInTheDocument()
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()

    await user.clear(screen.getByTestId('drive-name-input'))
    await user.type(screen.getByTestId('drive-name-input'), 'Updated Drive')
    await user.click(screen.getByRole('button', { name: 'Update' }))

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'Updated Drive', secondary_media_type: 'ssd' })
    )
  })

  it('shows fallback error message when Error has an empty message', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockRejectedValue(new Error(''))

    render(<DriveForm onClose={() => {}} onSave={onSave} />)

    await user.type(screen.getByTestId('drive-name-input'), 'Mirror A')
    await user.type(screen.getByTestId('drive-primary-path-input'), '/mnt/primary')
    await user.type(screen.getByTestId('drive-secondary-path-input'), '/mnt/secondary')
    await user.click(screen.getByRole('button', { name: 'Create' }))

    expect(await screen.findByText('Failed to create drive pair')).toBeInTheDocument()
  })
})
