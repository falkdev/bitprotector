import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ReplacementWorkflow } from './ReplacementWorkflow'
import { makeDrivePair } from '@/test/factories'
import { renderWithApp } from '@/test/render'

vi.mock('@/api/drives', () => ({
  drivesApi: {
    markReplacement: vi.fn().mockResolvedValue({}),
    confirmFailure: vi.fn().mockResolvedValue({}),
    cancelReplacement: vi.fn().mockResolvedValue({}),
    assignReplacement: vi.fn().mockResolvedValue({ drive_pair: {}, queued_rebuild_items: 0 }),
  },
}))

vi.mock('@/components/shared/PathPickerDialog', () => ({
  PathPickerDialog: () => null,
}))

describe('ReplacementWorkflow', () => {
  const drive = makeDrivePair({ id: 3, name: 'Backup Mirror' })
  const defaultProps = {
    drive,
    onClose: vi.fn(),
    onUpdate: vi.fn().mockResolvedValue(undefined),
  }

  it('renders the workflow modal with drive name', () => {
    renderWithApp(<ReplacementWorkflow {...defaultProps} />)
    expect(screen.getByText('Replacement Workflow — Backup Mirror')).toBeInTheDocument()
  })

  it('calls onClose when close button is clicked', async () => {
    const onClose = vi.fn()
    const user = userEvent.setup()
    renderWithApp(<ReplacementWorkflow {...defaultProps} onClose={onClose} />)

    await user.click(screen.getByTestId('close-replacement-workflow'))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('mark button calls markReplacement and onUpdate', async () => {
    const { drivesApi } = await import('@/api/drives')
    const onUpdate = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    renderWithApp(<ReplacementWorkflow {...defaultProps} onUpdate={onUpdate} />)

    await user.click(screen.getByTestId('mark-replacement-button'))
    await waitFor(() =>
      expect(drivesApi.markReplacement).toHaveBeenCalledWith(3, { role: 'primary' })
    )
    await waitFor(() => expect(onUpdate).toHaveBeenCalledWith(3))
  })

  it('confirm failure button calls confirmFailure and onUpdate', async () => {
    const { drivesApi } = await import('@/api/drives')
    const onUpdate = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    renderWithApp(<ReplacementWorkflow {...defaultProps} onUpdate={onUpdate} />)

    await user.click(screen.getByTestId('confirm-failure-button'))
    await waitFor(() =>
      expect(drivesApi.confirmFailure).toHaveBeenCalledWith(3, { role: 'primary' })
    )
    await waitFor(() => expect(onUpdate).toHaveBeenCalledWith(3))
  })

  it('assign button is disabled when path is empty', () => {
    renderWithApp(<ReplacementWorkflow {...defaultProps} />)
    expect(screen.getByTestId('assign-replacement-button')).toBeDisabled()
  })

  it('assign button is enabled when path is entered', async () => {
    const user = userEvent.setup()
    renderWithApp(<ReplacementWorkflow {...defaultProps} />)

    await user.type(screen.getByTestId('assign-path-input'), '/mnt/new')
    expect(screen.getByTestId('assign-replacement-button')).not.toBeDisabled()
  })

  it('assign calls assignReplacement with path and role', async () => {
    const { drivesApi } = await import('@/api/drives')
    const onUpdate = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    renderWithApp(<ReplacementWorkflow {...defaultProps} onUpdate={onUpdate} />)

    await user.type(screen.getByTestId('assign-path-input'), '/mnt/new-drive')
    await user.click(screen.getByTestId('assign-replacement-button'))

    await waitFor(() =>
      expect(drivesApi.assignReplacement).toHaveBeenCalledWith(3, {
        role: 'primary',
        new_path: '/mnt/new-drive',
        skip_validation: false,
      })
    )
  })
})
