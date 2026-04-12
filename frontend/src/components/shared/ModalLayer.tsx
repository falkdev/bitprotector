import type { ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { cn } from '@/lib/utils'

interface ModalLayerProps {
  children: ReactNode
  className?: string
  backdropTestId?: string
}

export function ModalLayer({
  children,
  className,
  backdropTestId = 'modal-overlay',
}: ModalLayerProps) {
  if (typeof document === 'undefined') {
    return null
  }

  return createPortal(
    <div
      className={cn('fixed inset-0 z-[70] flex items-center justify-center bg-black/40', className)}
      data-testid={backdropTestId}
    >
      {children}
    </div>,
    document.body
  )
}
