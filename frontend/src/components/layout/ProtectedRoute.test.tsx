import { render, screen } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ProtectedRoute } from './ProtectedRoute'

const mockUseAuth = vi.fn()

vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => mockUseAuth(),
}))

function renderProtectedRoute() {
  render(
    <MemoryRouter initialEntries={['/dashboard']}>
      <Routes>
        <Route path="/login" element={<div>Login Screen</div>} />
        <Route
          path="/dashboard"
          element={
            <ProtectedRoute>
              <div>Dashboard Content</div>
            </ProtectedRoute>
          }
        />
      </Routes>
    </MemoryRouter>
  )
}

describe('ProtectedRoute', () => {
  beforeEach(() => {
    mockUseAuth.mockReset()
  })

  it('redirects unauthenticated users to login', async () => {
    mockUseAuth.mockReturnValue({
      isAuthenticated: false,
      validate: vi.fn(),
    })

    renderProtectedRoute()
    expect(await screen.findByText('Login Screen')).toBeInTheDocument()
  })

  it('renders children when token validation succeeds', async () => {
    const validate = vi.fn().mockResolvedValue(true)
    mockUseAuth.mockReturnValue({
      isAuthenticated: true,
      validate,
    })

    renderProtectedRoute()
    expect(await screen.findByText('Dashboard Content')).toBeInTheDocument()
    expect(validate).toHaveBeenCalledTimes(1)
  })

  it('redirects to login when token validation fails', async () => {
    const validate = vi.fn().mockResolvedValue(false)
    mockUseAuth.mockReturnValue({
      isAuthenticated: true,
      validate,
    })

    renderProtectedRoute()
    expect(await screen.findByText('Login Screen')).toBeInTheDocument()
    expect(validate).toHaveBeenCalledTimes(1)
  })
})
