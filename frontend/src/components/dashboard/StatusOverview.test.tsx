import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { StatusOverview } from './StatusOverview'
import type { SystemStatus } from '@/types/status'

function makeStatus(overrides: Partial<SystemStatus> = {}): SystemStatus {
  return {
    files_tracked: 100,
    files_mirrored: 95,
    pending_sync: 0,
    integrity_issues: 0,
    drive_pairs: 2,
    degraded_pairs: 0,
    active_secondary_pairs: 0,
    rebuilding_pairs: 0,
    quiescing_pairs: 0,
    ...overrides,
  }
}

describe('StatusOverview', () => {
  it('renders all metric cards', () => {
    render(<StatusOverview status={makeStatus()} />)
    expect(screen.getByTestId('status-metric-files-tracked')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-files-mirrored')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-pending-sync')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-integrity-issues')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-drive-pairs')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-degraded-pairs')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-active-secondary')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-rebuilding')).toBeInTheDocument()
    expect(screen.getByTestId('status-metric-quiescing')).toBeInTheDocument()
  })

  it('displays the correct values', () => {
    render(<StatusOverview status={makeStatus({ files_tracked: 42, drive_pairs: 3 })} />)
    expect(screen.getByTestId('status-metric-files-tracked')).toHaveTextContent('42')
    expect(screen.getByTestId('status-metric-drive-pairs')).toHaveTextContent('3')
  })

  it('applies warning variant to pending sync when > 0', () => {
    render(<StatusOverview status={makeStatus({ pending_sync: 5 })} />)
    const card = screen.getByTestId('status-metric-pending-sync')
    expect(card.className).toMatch(/yellow/)
  })

  it('applies default variant to pending sync when 0', () => {
    render(<StatusOverview status={makeStatus({ pending_sync: 0 })} />)
    const card = screen.getByTestId('status-metric-pending-sync')
    expect(card.className).not.toMatch(/yellow/)
  })

  it('applies error variant to integrity issues when > 0', () => {
    render(<StatusOverview status={makeStatus({ integrity_issues: 3 })} />)
    const card = screen.getByTestId('status-metric-integrity-issues')
    expect(card.className).toMatch(/red/)
  })

  it('applies error variant to degraded pairs when > 0', () => {
    render(<StatusOverview status={makeStatus({ degraded_pairs: 1 })} />)
    const card = screen.getByTestId('status-metric-degraded-pairs')
    expect(card.className).toMatch(/red/)
  })

  it('applies info variant to rebuilding pairs when > 0', () => {
    render(<StatusOverview status={makeStatus({ rebuilding_pairs: 1 })} />)
    const card = screen.getByTestId('status-metric-rebuilding')
    expect(card.className).toMatch(/blue/)
  })
})
