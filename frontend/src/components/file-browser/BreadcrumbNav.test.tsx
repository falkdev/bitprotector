import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { BreadcrumbNav } from './BreadcrumbNav'

describe('BreadcrumbNav', () => {
  it('renders the home/All button', () => {
    render(<BreadcrumbNav path="" onNavigate={vi.fn()} />)
    expect(screen.getByRole('button', { name: /all/i })).toBeInTheDocument()
  })

  it('calls onNavigate with empty string when All is clicked', async () => {
    const onNavigate = vi.fn()
    const user = userEvent.setup()
    render(<BreadcrumbNav path="/documents/reports" onNavigate={onNavigate} />)

    await user.click(screen.getByRole('button', { name: /all/i }))
    expect(onNavigate).toHaveBeenCalledWith('')
  })

  it('renders breadcrumb parts for nested path', () => {
    render(<BreadcrumbNav path="/documents/reports" onNavigate={vi.fn()} />)
    expect(screen.getByText('documents')).toBeInTheDocument()
    expect(screen.getByText('reports')).toBeInTheDocument()
  })

  it('calls onNavigate with correct segment path when intermediate segment is clicked', async () => {
    const onNavigate = vi.fn()
    const user = userEvent.setup()
    render(<BreadcrumbNav path="/documents/reports/2026" onNavigate={onNavigate} />)

    await user.click(screen.getByRole('button', { name: 'documents' }))
    expect(onNavigate).toHaveBeenCalledWith('/documents')
  })

  it('the last segment button is disabled', () => {
    render(<BreadcrumbNav path="/documents/reports" onNavigate={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'reports' })).toBeDisabled()
  })

  it('renders nav with correct aria-label', () => {
    render(<BreadcrumbNav path="" onNavigate={vi.fn()} />)
    expect(screen.getByRole('navigation', { name: 'Breadcrumb' })).toBeInTheDocument()
  })

  it('handles empty path gracefully', () => {
    render(<BreadcrumbNav path="" onNavigate={vi.fn()} />)
    // Only the "All" button should appear — no separator chevrons
    expect(screen.queryByText('/')).not.toBeInTheDocument()
  })
})
