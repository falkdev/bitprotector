import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { DriveCard } from './DriveCard'
import { makeDrivePair } from '@/test/factories'

describe('DriveCard', () => {
  const defaultProps = {
    drive: makeDrivePair(),
    onEdit: vi.fn(),
    onDelete: vi.fn(),
    onManageReplacement: vi.fn(),
  }

  it('renders drive name', () => {
    render(<DriveCard {...defaultProps} />)
    expect(screen.getByText('Primary Mirror')).toBeInTheDocument()
  })

  it('renders primary and secondary paths', () => {
    render(<DriveCard {...defaultProps} />)
    expect(screen.getByText('/mnt/primary')).toBeInTheDocument()
    expect(screen.getByText('/mnt/mirror')).toBeInTheDocument()
  })

  it('renders state badges for both drives', () => {
    render(<DriveCard {...defaultProps} />)
    expect(screen.getByText('P: active')).toBeInTheDocument()
    expect(screen.getByText('S: active')).toBeInTheDocument()
  })

  it('renders media type badges', () => {
    render(<DriveCard {...defaultProps} />)
    expect(screen.getByText('P: hdd')).toBeInTheDocument()
    expect(screen.getByText('S: hdd')).toBeInTheDocument()
  })

  it('renders correct data-testid', () => {
    render(<DriveCard {...defaultProps} drive={makeDrivePair({ id: 7 })} />)
    expect(screen.getByTestId('drive-card-7')).toBeInTheDocument()
  })

  it('applies warning border when primary state is not active', () => {
    const drive = makeDrivePair({ primary_state: 'failed' })
    render(<DriveCard {...defaultProps} drive={drive} />)
    expect(screen.getByTestId('drive-card-1').className).toMatch(/orange/)
  })

  it('does not apply warning border when both drives are active', () => {
    render(<DriveCard {...defaultProps} />)
    expect(screen.getByTestId('drive-card-1').className).not.toMatch(/orange/)
  })

  it('calls onEdit when edit button is clicked', async () => {
    const onEdit = vi.fn()
    const user = userEvent.setup()
    render(<DriveCard {...defaultProps} onEdit={onEdit} />)

    await user.click(screen.getByTestId('edit-drive-1'))
    expect(onEdit).toHaveBeenCalledOnce()
  })

  it('calls onDelete when delete button is clicked', async () => {
    const onDelete = vi.fn()
    const user = userEvent.setup()
    render(<DriveCard {...defaultProps} onDelete={onDelete} />)

    await user.click(screen.getByTestId('delete-drive-1'))
    expect(onDelete).toHaveBeenCalledOnce()
  })

  it('calls onManageReplacement when replacement button is clicked', async () => {
    const onManageReplacement = vi.fn()
    const user = userEvent.setup()
    render(<DriveCard {...defaultProps} onManageReplacement={onManageReplacement} />)

    await user.click(screen.getByTestId('replace-drive-1'))
    expect(onManageReplacement).toHaveBeenCalledOnce()
  })
})
