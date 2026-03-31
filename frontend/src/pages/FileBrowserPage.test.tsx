import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { TrackFileModal } from './FileBrowserPage'
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
  active_role: 'primary',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
}

describe('TrackFileModal', () => {
  it('converts an absolute file path to a relative path before submit', async () => {
    const user = userEvent.setup()
    const onTrack = vi.fn().mockResolvedValue(undefined)

    render(
      <TrackFileModal
        open
        onClose={() => {}}
        onTrack={onTrack}
        drives={[drive]}
      />
    )

    await user.selectOptions(screen.getByRole('combobox'), '1')
    await user.type(
      screen.getByPlaceholderText('docs/report.pdf or /mnt/drive-a/docs/report.pdf'),
      '/mnt/primary/docs/report.pdf'
    )
    await user.click(screen.getByRole('button', { name: 'Track file' }))

    expect(onTrack).toHaveBeenCalledWith({
      drive_pair_id: 1,
      relative_path: 'docs/report.pdf',
    })
  })
})
