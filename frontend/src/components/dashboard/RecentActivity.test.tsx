import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { RecentActivity } from './RecentActivity'
import type { EventLogEntry } from '@/types/log'

function makeEntry(overrides: Partial<EventLogEntry> = {}): EventLogEntry {
  return {
    id: 1,
    event_type: 'file_created',
    tracked_file_id: null,
    file_path: null,
    message: 'File added',
    details: null,
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

describe('RecentActivity', () => {
  it('shows spinner when loading with no entries', () => {
    render(<RecentActivity entries={[]} loading={true} />)
    // LoadingSpinner renders — verify no entries shown and no empty state
    expect(screen.queryByText('No recent activity')).not.toBeInTheDocument()
  })

  it('shows empty state when not loading and no entries', () => {
    render(<RecentActivity entries={[]} loading={false} />)
    expect(screen.getByText('No recent activity')).toBeInTheDocument()
  })

  it('renders entries', () => {
    const entries = [
      makeEntry({ id: 1, message: 'File foo added', event_type: 'file_created' }),
      makeEntry({ id: 2, message: 'Integrity passed', event_type: 'integrity_pass' }),
    ]
    render(<RecentActivity entries={entries} loading={false} />)
    expect(screen.getByText('File foo added')).toBeInTheDocument()
    expect(screen.getByText('Integrity passed')).toBeInTheDocument()
  })

  it('renders file path when present', () => {
    const entry = makeEntry({ file_path: '/mnt/primary/doc.txt' })
    render(<RecentActivity entries={[entry]} loading={false} />)
    expect(screen.getByText('/mnt/primary/doc.txt')).toBeInTheDocument()
  })

  it('formats event type as badge text', () => {
    const entry = makeEntry({ event_type: 'recovery_success', message: 'Recovered ok' })
    render(<RecentActivity entries={[entry]} loading={false} />)
    expect(screen.getByText('recovery success')).toBeInTheDocument()
  })

  it('renders entries even while loading', () => {
    const entries = [makeEntry({ id: 1, message: 'Still showing' })]
    render(<RecentActivity entries={entries} loading={true} />)
    expect(screen.getByText('Still showing')).toBeInTheDocument()
  })
})
