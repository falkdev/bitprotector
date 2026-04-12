import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { FolderFormModal } from '@/components/tracking/FolderFormModal'
import type { DrivePair } from '@/types/drive'

vi.mock('@/components/shared/PathPickerDialog', () => ({
  PathPickerDialog: () => null,
}))

const drive: DrivePair = {
  id: 1,
  name: 'Main Pair',
  primary_path: '/mnt/primary',
  secondary_path: '/mnt/secondary',
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

    render(
      <FolderFormModal
        drives={[drive]}
        onClose={() => {}}
        onSave={onSave}
      />
    )

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('documents or /mnt/drive-a/documents'),
      '/mnt/primary/documents'
    )
    await user.click(screen.getByRole('button', { name: 'Add Folder' }))

    expect(onSave).toHaveBeenCalledWith({
      drive_pair_id: 1,
      folder_path: 'documents',
      virtual_path: undefined,
    })
  })
})
