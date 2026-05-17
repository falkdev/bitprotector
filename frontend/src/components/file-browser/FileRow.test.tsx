import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { FileRow } from './FileRow'
import { makeTrackedFile } from '@/test/factories'

function renderRow(props: Partial<Parameters<typeof FileRow>[0]> = {}) {
  const defaults = {
    file: makeTrackedFile({ id: 5, relative_path: 'docs/report.pdf' }),
    isSelected: false,
    onClick: vi.fn(),
    onMirror: vi.fn(),
    onDelete: vi.fn(),
    onSetVirtualPath: vi.fn(),
  }
  return render(
    <table>
      <tbody>
        <FileRow {...defaults} {...props} />
      </tbody>
    </table>
  )
}

describe('FileRow', () => {
  it('renders filename from relative_path', () => {
    renderRow()
    expect(screen.getByText('report.pdf')).toBeInTheDocument()
  })

  it('applies selected styles when isSelected=true', () => {
    renderRow({ isSelected: true })
    expect(screen.getByTestId('file-row-5').className).toMatch(/blue-50/)
  })

  it('does not apply selected styles when isSelected=false', () => {
    renderRow({ isSelected: false })
    expect(screen.getByTestId('file-row-5').className).not.toMatch(/blue-50/)
  })

  it('calls onClick when row is clicked', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    renderRow({ onClick })

    await user.click(screen.getByTestId('file-row-5'))
    expect(onClick).toHaveBeenCalledOnce()
  })

  it('shows Mirrored badge when file is mirrored', () => {
    renderRow({ file: makeTrackedFile({ id: 5, is_mirrored: true }) })
    expect(screen.getByText('Mirrored')).toBeInTheDocument()
  })

  it('does not show Mirrored badge when file is not mirrored', () => {
    renderRow({ file: makeTrackedFile({ id: 5, is_mirrored: false }) })
    expect(screen.queryByText('Mirrored')).not.toBeInTheDocument()
  })

  it('shows virtual path when present', () => {
    renderRow({
      file: makeTrackedFile({ id: 5, virtual_path: '/virtual/doc.pdf' }),
    })
    expect(screen.getByText('/virtual/doc.pdf')).toBeInTheDocument()
  })

  it('shows "none" when virtual_path is null', () => {
    renderRow({ file: makeTrackedFile({ id: 5, virtual_path: null }) })
    expect(screen.getByText('none')).toBeInTheDocument()
  })

  it('shows "—" when file_size is null', () => {
    renderRow({ file: makeTrackedFile({ id: 5, file_size: null }) })
    expect(screen.getByText('—')).toBeInTheDocument()
  })

  it('uses relative_path as filename when it contains no slashes', () => {
    renderRow({ file: makeTrackedFile({ id: 5, relative_path: 'rootfile.txt' }) })
    expect(screen.getByText('rootfile.txt')).toBeInTheDocument()
  })
})
