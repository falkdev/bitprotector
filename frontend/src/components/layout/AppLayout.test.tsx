import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { beforeEach, describe, expect, it } from 'vitest'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { AppLayout } from './AppLayout'
import { SIDEBAR_COLLAPSED_STORAGE_KEY } from './Sidebar'
import { useAuthStore } from '@/stores/auth-store'
import { useThemeStore } from '@/stores/theme-store'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { makeDrivePair } from '@/test/factories'

function renderLayout(initialRoute = '/dashboard') {
  return (
    <MemoryRouter initialEntries={[initialRoute]}>
      <Routes>
        <Route path="/" element={<AppLayout />}>
          <Route path="dashboard" element={<div>Dashboard content</div>} />
          <Route path="integrity" element={<div>Integrity content</div>} />
          <Route path="drives" element={<div>Drives content</div>} />
          <Route path="scheduler" element={<div>Scheduler content</div>} />
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
    // Default: no drives — individual tests override as needed
    server.use(api.get('/drives', () => HttpResponse.json([])))
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

  describe('no-drives banner', () => {
    it('shows the banner when no drives exist', async () => {
      render(renderLayout())

      expect(await screen.findByText(/Add a drive pair/i)).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Drives page' })).toBeInTheDocument()
    })

    it('does not show the banner when drives exist', async () => {
      server.use(api.get('/drives', () => HttpResponse.json([makeDrivePair()])))

      render(renderLayout())

      // Wait for fetch to complete, then assert banner is absent
      await screen.findByText('Dashboard content')
      // Give the async fetch time to resolve
      await new Promise((r) => setTimeout(r, 0))
      expect(screen.queryByText(/Add a drive pair/i)).not.toBeInTheDocument()
    })

    it('hides the banner when the dismiss button is clicked', async () => {
      const user = userEvent.setup()
      render(renderLayout())

      await screen.findByText(/Add a drive pair/i)
      await user.click(screen.getByRole('button', { name: 'Dismiss' }))

      expect(screen.queryByText(/Add a drive pair/i)).not.toBeInTheDocument()
    })

    it('banner stays dismissed after navigating to another page', async () => {
      const user = userEvent.setup()
      render(renderLayout())

      await screen.findByText(/Add a drive pair/i)
      await user.click(screen.getByRole('button', { name: 'Dismiss' }))

      // Navigate to Drives page via the sidebar
      await user.click(screen.getByTestId('nav-drives'))
      expect(await screen.findByText('Drives content')).toBeInTheDocument()

      expect(screen.queryByText(/Add a drive pair/i)).not.toBeInTheDocument()
    })
  })

  describe('sidebar nav disabled state', () => {
    const disabledRoutes = ['files', 'integrity', 'sync', 'scheduler']
    const alwaysEnabledRoutes = ['dashboard', 'drives', 'logs', 'database']

    it('disables nav items that require drives when no drives exist', async () => {
      render(renderLayout())

      // Wait for the drives fetch to complete
      await screen.findByText(/Add a drive pair/i)

      for (const route of disabledRoutes) {
        const item = screen.getByTestId(`nav-${route}`)
        expect(item.tagName).toBe('SPAN')
        expect(item).toHaveAttribute('aria-disabled', 'true')
      }
    })

    it('keeps always-enabled nav items as links regardless of drives', async () => {
      render(renderLayout())

      await screen.findByText(/Add a drive pair/i)

      for (const route of alwaysEnabledRoutes) {
        const item = screen.getByTestId(`nav-${route}`)
        expect(item.tagName).toBe('A')
        expect(item).not.toHaveAttribute('aria-disabled')
      }
    })

    it('enables all nav items when drives exist', async () => {
      server.use(api.get('/drives', () => HttpResponse.json([makeDrivePair()])))

      render(renderLayout())

      // Wait for the fetch to resolve — banner should not appear
      await screen.findByText('Dashboard content')
      await new Promise((r) => setTimeout(r, 0))

      for (const route of disabledRoutes) {
        const item = screen.getByTestId(`nav-${route}`)
        expect(item.tagName).toBe('A')
        expect(item).not.toHaveAttribute('aria-disabled')
      }
    })
  })
})
