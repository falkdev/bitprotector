import type { ReactElement } from 'react'
import { render } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { Toaster } from 'sonner'

interface RenderWithAppOptions {
  route?: string
}

export function renderWithApp(ui: ReactElement, options: RenderWithAppOptions = {}) {
  const { route = '/' } = options

  return render(
    <MemoryRouter initialEntries={[route]}>
      <Toaster position="top-right" richColors />
      {ui}
    </MemoryRouter>
  )
}
