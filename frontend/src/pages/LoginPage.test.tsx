import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { LoginPage } from './LoginPage'
import { renderWithApp } from '@/test/render'

vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => ({
    isAuthenticated: false,
    login: vi.fn(),
  }),
}))

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
})
