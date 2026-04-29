import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AppLayout } from './AppLayout'
import { SIDEBAR_COLLAPSED_STORAGE_KEY } from './Sidebar'
import { useAuthStore } from '@/stores/auth-store'
import { useThemeStore } from '@/stores/theme-store'

function renderLayout(initialRoute = '/dashboard') {
  return (
    <MemoryRouter initialEntries={[initialRoute]}>
      <Routes>
        <Route path="/" element={<AppLayout />}>
          <Route path="dashboard" element={<div>Dashboard content</div>} />
          <Route path="integrity" element={<div>Integrity content</div>} />
        </Route>
        <Route path="/login" element={<div>Login page</div>} />
      </Routes>
    </MemoryRouter>
  )
}

describe('AppLayout', () => {
  beforeEach(() => {
    localStorage.clear()
    useAuthStore.setState({
      token: 'test-token',
      username: 'testuser',
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      isAuthenticated: true,
    })
  })

  it('renders authenticated pages without a top header bar', () => {
    render(renderLayout())

    expect(screen.getByText('Dashboard content')).toBeInTheDocument()
    expect(screen.queryByRole('banner')).not.toBeInTheDocument()
  })

  it('opens the sidebar user menu and logs out from the footer', async () => {
    const user = userEvent.setup()
    render(renderLayout('/integrity'))

    const sidebar = screen.getByRole('complementary')
    const footer = sidebar.lastElementChild as HTMLElement
    const userMenuTrigger = screen.getByTestId('user-menu-trigger')

    expect(within(footer).getByText('testuser')).toBeInTheDocument()
    expect(footer).toContainElement(userMenuTrigger)
    await user.click(userMenuTrigger)

    const logoutButton = screen.getByTestId('user-menu-logout')
    expect(logoutButton).toBeInTheDocument()

    await user.click(logoutButton)
    expect(screen.getByText('Login page')).toBeInTheDocument()
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
  })

  it('collapses sidebar navigation and persists collapsed state', async () => {
    const user = userEvent.setup()
    const firstRender = render(renderLayout('/dashboard'))

    const sidebar = screen.getByTestId('app-sidebar')
    const toggle = screen.getByTestId('sidebar-toggle')
    expect(sidebar).toHaveClass('w-56')

    await user.click(toggle)
    expect(sidebar).toHaveClass('w-16')
    expect(localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY)).toBe('1')
    expect(screen.getByTestId('nav-dashboard')).toHaveAttribute('title', 'Dashboard')

    firstRender.unmount()
    render(renderLayout('/dashboard'))

    expect(screen.getByTestId('app-sidebar')).toHaveClass('w-16')
    expect(screen.getByTestId('nav-dashboard')).toHaveAttribute('title', 'Dashboard')
  })

  it('toggles dark mode via the user menu theme toggle', async () => {
    const user = userEvent.setup()
    render(renderLayout())

    await user.click(screen.getByTestId('user-menu-trigger'))
    const toggleBtn = screen.getByTestId('user-menu-theme-toggle')
    expect(toggleBtn).toBeInTheDocument()

    // First click → dark (menu stays open)
    await user.click(toggleBtn)
    expect(document.documentElement.classList.contains('dark')).toBe(true)

    // Click again without re-opening (menu is still open) → light
    await user.click(screen.getByTestId('user-menu-theme-toggle'))
    expect(document.documentElement.classList.contains('dark')).toBe(false)

    // Cleanup
    useThemeStore.setState({ override: null })
  })
})
