import { useState } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { toast, Toaster } from 'sonner'
import { describe, expect, it, vi } from 'vitest'
import { ModalLayer } from './ModalLayer'

if (!HTMLElement.prototype.setPointerCapture) {
  HTMLElement.prototype.setPointerCapture = () => {}
}

if (!HTMLElement.prototype.releasePointerCapture) {
  HTMLElement.prototype.releasePointerCapture = () => {}
}

describe('ModalLayer', () => {
  it('renders a modal overlay through a portal when mounted', () => {
    render(
      <ModalLayer>
        <div>Modal Body</div>
      </ModalLayer>
    )

    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()
    expect(screen.getByText('Modal Body')).toBeInTheDocument()
  })

  it('does not render anything when the parent does not mount it', () => {
    function Example({ open }: { open: boolean }) {
      if (!open) return null
      return (
        <ModalLayer>
          <div>Shown</div>
        </ModalLayer>
      )
    }

    const { rerender } = render(<Example open={false} />)
    expect(screen.queryByTestId('modal-overlay')).not.toBeInTheDocument()

    rerender(<Example open />)
    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()
  })

  it('allows alert/info toast actions while the modal layer is visible', async () => {
    const user = userEvent.setup()
    const onToastAction = vi.fn()

    function ModalWithToastTrigger() {
      const [open] = useState(true)

      return (
        <>
          <Toaster position="top-right" richColors />
          <button
            type="button"
            onClick={() =>
              toast.info('Connection warning', {
                action: {
                  label: 'Acknowledge',
                  onClick: onToastAction,
                },
              })
            }
          >
            Show Toast
          </button>
          {open ? (
            <ModalLayer>
              <div>Blocking Modal</div>
            </ModalLayer>
          ) : null}
        </>
      )
    }

    render(<ModalWithToastTrigger />)

    expect(screen.getByTestId('modal-overlay')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Show Toast' }))

    const actionButton = await screen.findByRole('button', { name: 'Acknowledge' })
    await user.click(actionButton)

    expect(onToastAction).toHaveBeenCalledTimes(1)
  })
})
