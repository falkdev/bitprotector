import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { FileDetails } from './FileDetails'
import { makeTrackedFile } from '@/test/factories'

describe('FileDetails', () => {
  const onClose = vi.fn()

  it('renders the filename from relative_path', () => {
    const file = makeTrackedFile({ relative_path: 'documents/report.pdf' })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('report.pdf')).toBeInTheDocument()
  })

  it('renders the relative path', () => {
    const file = makeTrackedFile({ relative_path: 'docs/report.pdf' })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('docs/report.pdf')).toBeInTheDocument()
  })

  it('renders virtual path when present', () => {
    const file = makeTrackedFile({ virtual_path: '/virtual/report.pdf' })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('/virtual/report.pdf')).toBeInTheDocument()
  })

  it('does not render virtual path section when absent', () => {
    const file = makeTrackedFile({ virtual_path: null })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.queryByText('Virtual path')).not.toBeInTheDocument()
  })

  it('renders formatted file size', () => {
    const file = makeTrackedFile({ file_size: 1024 })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('1.0 KB')).toBeInTheDocument()
  })

  it('renders drive pair name when provided', () => {
    const file = makeTrackedFile()
    render(<FileDetails file={file} drivePairName="Mirror A" onClose={onClose} />)
    expect(screen.getByText('Mirror A')).toBeInTheDocument()
  })

  it('falls back to drive_pair_id when drivePairName not provided', () => {
    const file = makeTrackedFile({ drive_pair_id: 7 })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('ID 7')).toBeInTheDocument()
  })

  it('renders mirrored status', () => {
    const file = makeTrackedFile({ is_mirrored: true })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('Yes')).toBeInTheDocument()
  })

  it('renders checksum when present', () => {
    const file = makeTrackedFile({ checksum: 'abc123def456' })
    render(<FileDetails file={file} onClose={onClose} />)
    expect(screen.getByText('abc123def456')).toBeInTheDocument()
  })

  it('calls onClose when close button is clicked', async () => {
    const onCloseFn = vi.fn()
    const user = userEvent.setup()
    const file = makeTrackedFile()
    render(<FileDetails file={file} onClose={onCloseFn} />)

    await user.click(screen.getByTestId('close-details'))
    expect(onCloseFn).toHaveBeenCalledOnce()
  })
})
