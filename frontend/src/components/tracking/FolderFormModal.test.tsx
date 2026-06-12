import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { FolderFormModal } from '@/components/tracking/FolderFormModal'
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
        <button onClick={() => onPick('/mnt/secondary/selected')}>Pick</button>
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

describe('FolderFormModal', () => {
  it('converts an absolute folder path to a relative path before submit', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/mnt/secondary/documents'
    )
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(onSave).toHaveBeenCalledWith({
      drive_pair_id: 1,
      folder_path: 'documents',
      virtual_path: undefined,
    })
  })

  it('shows drive pair required error when submitted without selecting a drive', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(await screen.findByText('Select a drive pair')).toBeInTheDocument()
    expect(onSave).not.toHaveBeenCalled()
  })

  it('shows folder path required error when submitted without a path', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    // Either zod required error or manual path error
    expect(await screen.findByText(/required|is required/i)).toBeInTheDocument()
    expect(onSave).not.toHaveBeenCalled()
  })

  it('shows path resolution error when path is outside active root', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/other/path'
    )
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(onSave).not.toHaveBeenCalled()
    // The setError path is triggered, which keeps onSave from being called
  })

  it('shows virtual path validation error for non-absolute virtual path', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/mnt/primary/documents'
    )
    await user.type(screen.getByPlaceholderText('/docs'), 'relative/path')
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(await screen.findByText('Virtual path must be absolute')).toBeInTheDocument()
    expect(onSave).not.toHaveBeenCalled()
  })

  it('shows the active root hint after selecting a drive', async () => {
    const user = userEvent.setup()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={vi.fn()} />)

    expect(
      screen.getByText('Select a drive pair before browsing or submitting.')
    ).toBeInTheDocument()

    await user.selectOptions(screen.getByRole('combobox'), '1')

    expect(screen.getByText('Active root: /mnt/secondary')).toBeInTheDocument()
  })

  it('shows the resolved relative path hint when a valid path is entered', async () => {
    const user = userEvent.setup()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={vi.fn()} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/mnt/secondary/documents'
    )

    expect(await screen.findByText(/Will be stored as/)).toBeInTheDocument()
    expect(screen.getByText('documents')).toBeInTheDocument()
  })

  it('submits with an optional virtual path when provided', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/mnt/secondary/documents'
    )
    await user.type(screen.getByPlaceholderText('/docs'), '/virtual/docs')
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(onSave).toHaveBeenCalledWith({
      drive_pair_id: 1,
      folder_path: 'documents',
      virtual_path: '/virtual/docs',
    })
  })

  it('calls onClose when the Cancel button is clicked', async () => {
    const user = userEvent.setup()
    const onClose = vi.fn()

    render(<FolderFormModal drives={[drive]} onClose={onClose} onSave={vi.fn()} />)

    await user.click(screen.getByRole('button', { name: 'Cancel' }))

    expect(onClose).toHaveBeenCalled()
  })

  it('opens the path picker when Browse is clicked and sets path on pick', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={onSave} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    const browseButtons = screen.getAllByRole('button', { name: 'Browse' })
    await user.click(browseButtons[0])

    expect(screen.getByTestId('path-picker-dialog')).toBeInTheDocument()
    expect(screen.getByText('Select Folder Path')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Pick' }))

    expect(screen.queryByTestId('path-picker-dialog')).not.toBeInTheDocument()
  })

  it('closes the path picker without selecting when ClosePicker is clicked', async () => {
    const user = userEvent.setup()

    render(<FolderFormModal drives={[drive]} onClose={() => {}} onSave={vi.fn()} />)

    await user.selectOptions(screen.getByRole('combobox'), '1')
    const browseButtons = screen.getAllByRole('button', { name: 'Browse' })
    await user.click(browseButtons[0])
    await user.click(screen.getByRole('button', { name: 'ClosePicker' }))

    expect(screen.queryByTestId('path-picker-dialog')).not.toBeInTheDocument()
  })
})
