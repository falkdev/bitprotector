import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { IntegrityStatusBadge } from './IntegrityStatus'
import type { IntegrityStatus } from '@/types/integrity'

describe('IntegrityStatusBadge', () => {
  it('renders OK status', () => {
    render(<IntegrityStatusBadge status="ok" />)
    expect(screen.getByText('OK')).toBeInTheDocument()
  })

  it('renders master_corrupted status', () => {
    render(<IntegrityStatusBadge status="master_corrupted" />)
    expect(screen.getByText('Primary corrupt')).toBeInTheDocument()
  })

  it('renders mirror_corrupted status', () => {
    render(<IntegrityStatusBadge status="mirror_corrupted" />)
    expect(screen.getByText('Mirror corrupt')).toBeInTheDocument()
  })

  it('renders both_corrupted status', () => {
    render(<IntegrityStatusBadge status="both_corrupted" />)
    expect(screen.getByText('Both corrupt')).toBeInTheDocument()
  })

  it('renders master_missing status', () => {
    render(<IntegrityStatusBadge status="master_missing" />)
    expect(screen.getByText('Primary missing')).toBeInTheDocument()
  })

  it('renders mirror_missing status', () => {
    render(<IntegrityStatusBadge status="mirror_missing" />)
    expect(screen.getByText('Mirror missing')).toBeInTheDocument()
  })

  it('renders primary_drive_unavailable status', () => {
    render(<IntegrityStatusBadge status="primary_drive_unavailable" />)
    expect(screen.getByText('Primary unavailable')).toBeInTheDocument()
  })

  it('renders secondary_drive_unavailable status', () => {
    render(<IntegrityStatusBadge status="secondary_drive_unavailable" />)
    expect(screen.getByText('Mirror unavailable')).toBeInTheDocument()
  })

  it('renders internal_error status', () => {
    render(<IntegrityStatusBadge status="internal_error" />)
    expect(screen.getByText('Internal error')).toBeInTheDocument()
  })

  it('renders Unknown for unrecognized status', () => {
    render(<IntegrityStatusBadge status={'unknown_status' as IntegrityStatus} />)
    expect(screen.getByText('Unknown')).toBeInTheDocument()
  })

  it('applies green color for ok status', () => {
    const { container } = render(<IntegrityStatusBadge status="ok" />)
    expect(container.firstChild).toHaveClass('text-green-700')
  })

  it('applies red color for corrupted status', () => {
    const { container } = render(<IntegrityStatusBadge status="both_corrupted" />)
    expect(container.firstChild).toHaveClass('text-red-700')
  })

  it('applies extra className when provided', () => {
    const { container } = render(<IntegrityStatusBadge status="ok" className="custom-class" />)
    expect(container.firstChild).toHaveClass('custom-class')
  })
})
