import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { QuickActions } from './QuickActions'

describe('QuickActions', () => {
  const defaultProps = {
    onIntegrityCheck: vi.fn().mockResolvedValue(undefined),
    onProcessSync: vi.fn().mockResolvedValue(undefined),
    onRunBackup: vi.fn().mockResolvedValue(undefined),
  }

  it('renders all three action buttons', () => {
    render(<QuickActions {...defaultProps} />)
    expect(screen.getByTestId('quick-action-integrity')).toBeInTheDocument()
    expect(screen.getByTestId('quick-action-sync')).toBeInTheDocument()
    expect(screen.getByTestId('quick-action-backup')).toBeInTheDocument()
  })

  it('calls onIntegrityCheck when integrity button is clicked', async () => {
    const onIntegrityCheck = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    render(<QuickActions {...defaultProps} onIntegrityCheck={onIntegrityCheck} />)

    await user.click(screen.getByTestId('quick-action-integrity'))
    expect(onIntegrityCheck).toHaveBeenCalledOnce()
  })

  it('calls onProcessSync when sync button is clicked', async () => {
    const onProcessSync = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    render(<QuickActions {...defaultProps} onProcessSync={onProcessSync} />)

    await user.click(screen.getByTestId('quick-action-sync'))
    expect(onProcessSync).toHaveBeenCalledOnce()
  })

  it('calls onRunBackup when backup button is clicked', async () => {
    const onRunBackup = vi.fn().mockResolvedValue(undefined)
    const user = userEvent.setup()
    render(<QuickActions {...defaultProps} onRunBackup={onRunBackup} />)

    await user.click(screen.getByTestId('quick-action-backup'))
    expect(onRunBackup).toHaveBeenCalledOnce()
  })

  it('shows Running text while action is in progress', async () => {
    let resolve: () => void
    const onProcessSync = vi.fn(
      () =>
        new Promise<void>((res) => {
          resolve = res
        })
    )
    const user = userEvent.setup()
    render(<QuickActions {...defaultProps} onProcessSync={onProcessSync} />)

    await user.click(screen.getByTestId('quick-action-sync'))
    expect(screen.getByText('Running…')).toBeInTheDocument()

    resolve!()
    await waitFor(() => expect(screen.queryByText('Running…')).not.toBeInTheDocument())
  })

  it('disables all buttons while an action is running', async () => {
    let resolve: () => void
    const onRunBackup = vi.fn(
      () =>
        new Promise<void>((res) => {
          resolve = res
        })
    )
    const user = userEvent.setup()
    render(<QuickActions {...defaultProps} onRunBackup={onRunBackup} />)

    await user.click(screen.getByTestId('quick-action-backup'))
    expect(screen.getByTestId('quick-action-integrity')).toBeDisabled()
    expect(screen.getByTestId('quick-action-sync')).toBeDisabled()

    resolve!()
    await waitFor(() => expect(screen.getByTestId('quick-action-integrity')).not.toBeDisabled())
  })

  it('disables integrity button and shows hint when integrityDisabled=true', () => {
    render(
      <QuickActions
        {...defaultProps}
        integrityDisabled
        integrityDisabledMessage="An integrity run is already in progress"
      />
    )
    expect(screen.getByTestId('quick-action-integrity')).toBeDisabled()
    expect(screen.getByTestId('quick-action-integrity-hint')).toHaveTextContent(
      'An integrity run is already in progress'
    )
  })

  it('does not render hint when integrityDisabled is false', () => {
    render(<QuickActions {...defaultProps} integrityDisabled={false} />)
    expect(screen.queryByTestId('quick-action-integrity-hint')).not.toBeInTheDocument()
  })
})
