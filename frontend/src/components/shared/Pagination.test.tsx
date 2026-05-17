import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { Pagination } from './Pagination'

describe('Pagination', () => {
  it('renders nothing when total is 0', () => {
    const { container } = render(
      <Pagination page={1} perPage={10} total={0} onPageChange={vi.fn()} />
    )
    expect(container).toBeEmptyDOMElement()
  })

  it('shows range and page indicator', () => {
    render(<Pagination page={1} perPage={10} total={25} onPageChange={vi.fn()} />)
    expect(screen.getByText('1–10 of 25')).toBeInTheDocument()
    expect(screen.getByText('1 / 3')).toBeInTheDocument()
  })

  it('shows last page range correctly', () => {
    render(<Pagination page={3} perPage={10} total={25} onPageChange={vi.fn()} />)
    expect(screen.getByText('21–25 of 25')).toBeInTheDocument()
  })

  it('calls onPageChange with next page when next is clicked', async () => {
    const user = userEvent.setup()
    const onPageChange = vi.fn()
    render(<Pagination page={1} perPage={10} total={25} onPageChange={onPageChange} />)

    await user.click(screen.getByLabelText('Next page'))

    expect(onPageChange).toHaveBeenCalledWith(2)
  })

  it('calls onPageChange with previous page when prev is clicked', async () => {
    const user = userEvent.setup()
    const onPageChange = vi.fn()
    render(<Pagination page={2} perPage={10} total={25} onPageChange={onPageChange} />)

    await user.click(screen.getByLabelText('Previous page'))

    expect(onPageChange).toHaveBeenCalledWith(1)
  })

  it('disables previous button on first page', () => {
    render(<Pagination page={1} perPage={10} total={25} onPageChange={vi.fn()} />)
    expect(screen.getByLabelText('Previous page')).toBeDisabled()
  })

  it('disables next button on last page', () => {
    render(<Pagination page={3} perPage={10} total={25} onPageChange={vi.fn()} />)
    expect(screen.getByLabelText('Next page')).toBeDisabled()
  })
})
