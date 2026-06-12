import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it, vi } from 'vitest'
import { LoginPage } from './LoginPage'
import { authNavigation } from '@/api/client'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { renderWithApp } from '@/test/render'

const mockLogin = vi.fn()
const mockNavigate = vi.fn()

vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => ({
    isAuthenticated: false,
    login: mockLogin,
  }),
}))

vi.mock('react-router-dom', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-router-dom')>()
  return { ...actual, useNavigate: () => mockNavigate }
})

describe('LoginPage', () => {
  it('renders the login controls for the web GUI', () => {
    renderWithApp(<LoginPage />, { route: '/login' })

    expect(screen.getByRole('heading', { name: 'BitProtector' })).toBeInTheDocument()
    expect(screen.getByTestId('login-form')).toBeInTheDocument()
    expect(screen.getByTestId('username-input')).toBeInTheDocument()
    expect(screen.getByTestId('password-input')).toBeInTheDocument()
    expect(screen.getByTestId('login-button')).toBeInTheDocument()
    expect(screen.queryByTestId('page-title')).not.toBeInTheDocument()
  })

  it('submits credentials and navigates on success', async () => {
    const user = userEvent.setup()
    server.use(
      api.post('/auth/login', () =>
        HttpResponse.json({
          token: 'jwt-token',
          username: 'alice',
          expires_at: new Date(Date.now() + 3_600_000).toISOString(),
        })
      )
    )

    renderWithApp(<LoginPage />, { route: '/login' })

    await user.type(screen.getByTestId('username-input'), 'alice')
    await user.type(screen.getByTestId('password-input'), 'secret')
    await user.click(screen.getByTestId('login-button'))

    await waitFor(() => {
      expect(mockLogin).toHaveBeenCalled()
      expect(mockNavigate).toHaveBeenCalledWith('/dashboard', { replace: true })
    })
  })

  it('shows inline error on login failure without triggering global redirect', async () => {
    const user = userEvent.setup()
    const redirectSpy = vi.spyOn(authNavigation, 'redirectToLogin').mockImplementation(() => {})
    server.use(
      api.post('/auth/login', () =>
        HttpResponse.json({ error: 'bad credentials' }, { status: 401 })
      )
    )

    renderWithApp(<LoginPage />, { route: '/login' })

    await user.type(screen.getByTestId('username-input'), 'alice')
    await user.type(screen.getByTestId('password-input'), 'wrong')
    await user.click(screen.getByTestId('login-button'))

    const errorEl = await screen.findByTestId('login-error')
    expect(errorEl).toBeInTheDocument()
    expect(errorEl).toHaveTextContent('Invalid username or password')

    await waitFor(() => {
      expect(screen.getByTestId('login-error')).toBeInTheDocument()
    })
    expect(redirectSpy).not.toHaveBeenCalled()

    redirectSpy.mockRestore()
  })

  it('clears inline error when user edits a field', async () => {
    const user = userEvent.setup()
    server.use(
      api.post('/auth/login', () =>
        HttpResponse.json({ error: 'bad credentials' }, { status: 401 })
      )
    )

    renderWithApp(<LoginPage />, { route: '/login' })

    await user.type(screen.getByTestId('username-input'), 'alice')
    await user.type(screen.getByTestId('password-input'), 'wrong')
    await user.click(screen.getByTestId('login-button'))

    expect(await screen.findByTestId('login-error')).toBeInTheDocument()

    await user.type(screen.getByTestId('password-input'), 'x')
    expect(screen.queryByTestId('login-error')).not.toBeInTheDocument()
  })

  it('shows validation errors when form is submitted empty', async () => {
    const user = userEvent.setup()
    renderWithApp(<LoginPage />, { route: '/login' })

    await user.click(screen.getByTestId('login-button'))

    expect(await screen.findByText('Username is required')).toBeInTheDocument()
    expect(await screen.findByText('Password is required')).toBeInTheDocument()
  })
})
