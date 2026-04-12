import { useState } from 'react'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ConfirmDialog } from './ConfirmDialog'

describe('ConfirmDialog', () => {
  it('renders modal overlay on open and fires confirm action', async () => {
    const user = userEvent.setup()
    const onConfirm = vi.fn()

    function Harness() {
      const [open, setOpen] = useState(false)

      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Open Confirm
          </button>
          <ConfirmDialog
            open={open}
            onOpenChange={setOpen}
            title="Delete item?"
            description="This cannot be undone."
            onConfirm={onConfirm}
          />
        </>
      )
    }

    render(<Harness />)

    expect(screen.queryByTestId('modal-overlay')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Open Confirm' }))

    const dialog = await screen.findByRole('alertdialog')
    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: 'Confirm' }))
    expect(onConfirm).toHaveBeenCalledTimes(1)
  })
})
