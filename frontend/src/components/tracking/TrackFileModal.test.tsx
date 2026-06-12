import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { TrackFileModal } from '@/components/tracking/TrackFileModal'
import type { DrivePair } from '@/types/drive'

vi.mock('@/components/shared/PathPickerDialog', () => ({
  PathPickerDialog: ({
    open,
    onPick,
    onClose,
    title,
  }: {
    open: boolean
    onPick: (path: string) => void
    onClose: () => void
    title: string
  }) => {
    if (!open) return null
    return (
      <div data-testid="path-picker-dialog">
        <span>{title}</span>
        <button onClick={() => onPick('/mnt/secondary/picked-file.txt')}>Pick</button>
        <button onClick={onClose}>ClosePicker</button>
      </div>
    )
  },
}))

const drive: DrivePair = {
  id: 1,
  name: 'Main Pair',
  primary_path: '/mnt/primary',
  secondary_path: '/mnt/secondary',
  primary_media_type: 'hdd',
  secondary_media_type: 'hdd',
  primary_state: 'active',
  secondary_state: 'active',
  active_role: 'secondary',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

describe('TrackFileModal', () => {
  it('renders nothing when open=false', () => {
    render(<TrackFileModal open={false} onClose={() => {}} onTrack={vi.fn()} drives={[drive]} />)
    expect(screen.queryByText('Track new file')).not.toBeInTheDocument()
  })

  it('converts an absolute file path to a relative path before submit', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn().mockResolvedValue(undefined)

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/mnt/secondary/docs/report.pdf'
    )
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(onTrack).toHaveBeenCalledWith({
      drive_pair_id: 1,
      relative_path: 'docs/report.pdf',
    })
  })

  it('submits an optional virtual path when provided', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn().mockResolvedValue(undefined)

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/mnt/secondary/docs/report.pdf'
    )
    await user.type(screen.getByPlaceholderText('/docs/report.pdf'), '/virtual/docs/report.pdf')
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(onTrack).toHaveBeenCalledWith({
      drive_pair_id: 1,
      relative_path: 'docs/report.pdf',
      virtual_path: '/virtual/docs/report.pdf',
    })
  })

  it('shows drive pair validation error when submitted without a drive', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn()

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(await screen.findByText('Drive pair ID is required')).toBeInTheDocument()
    expect(onTrack).not.toHaveBeenCalled()
  })

  it('shows path required error when submitted without a path but drive selected', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn()

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(await screen.findByText(/is required/i)).toBeInTheDocument()
    expect(onTrack).not.toHaveBeenCalled()
  })

  it('shows path resolution error when path is outside active root', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn()

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/other/path/file.txt'
    )
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(onTrack).not.toHaveBeenCalled()
  })

  it('shows virtual path error for non-absolute virtual path', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn()

    render(<TrackFileModal open onClose={() => {}} onTrack={onTrack} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/mnt/secondary/docs/report.pdf'
    )
    await user.type(screen.getByPlaceholderText('/docs/report.pdf'), 'relative/path')
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(await screen.findByText('Virtual path must be absolute')).toBeInTheDocument()
    expect(onTrack).not.toHaveBeenCalled()
  })

  it('shows the active root hint after selecting a drive', async () => {
    const user = userEvent.setup()

    render(<TrackFileModal open onClose={() => {}} onTrack={vi.fn()} drives={[drive]} />)

    expect(
      screen.getByText('Select a drive pair before browsing or submitting.')
    ).toBeInTheDocument()

    await user.selectOptions(screen.getByRole('combobox'), '1')

    expect(screen.getByText('Active root: /mnt/secondary')).toBeInTheDocument()
  })

  it('shows the resolved path hint when a valid path is entered', async () => {
    const user = userEvent.setup()

    render(<TrackFileModal open onClose={() => {}} onTrack={vi.fn()} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/mnt/secondary/docs/report.pdf'
    )

    expect(await screen.findByText(/Will be stored as/)).toBeInTheDocument()
  })

  it('calls onClose when the Cancel button is clicked', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()

    render(<TrackFileModal open onClose={onClose} onTrack={vi.fn()} drives={[drive]} />)

    await user.click(screen.getByRole('button', { name: 'Cancel' }))

    expect(onClose).toHaveBeenCalled()
  })

  it('opens the path picker when Browse is clicked and sets path on pick', async () => {
    const user = userEvent.setup()

    render(<TrackFileModal open onClose={() => {}} onTrack={vi.fn()} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')

    // The file path Browse button (first one)
    const browseButtons = screen.getAllByRole('button', { name: 'Browse' })
    await user.click(browseButtons[0])

    expect(screen.getByTestId('path-picker-dialog')).toBeInTheDocument()
    expect(screen.getByText('Select File Path')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Pick' }))

    expect(screen.queryByTestId('path-picker-dialog')).not.toBeInTheDocument()
  })

  it('closes the path picker without selecting when ClosePicker is clicked', async () => {
    const user = userEvent.setup()

    render(<TrackFileModal open onClose={() => {}} onTrack={vi.fn()} drives={[drive]} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    const browseButtons = screen.getAllByRole('button', { name: 'Browse' })
    await user.click(browseButtons[0])
    await user.click(screen.getByRole('button', { name: 'ClosePicker' }))

    expect(screen.queryByTestId('path-picker-dialog')).not.toBeInTheDocument()
  })
})
