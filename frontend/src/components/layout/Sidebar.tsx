import { useEffect, useRef, useState } from 'react'
import { NavLink, useNavigate } from 'react-router-dom'
import {
  LayoutDashboard,
  Files,
  HardDrive,
  ShieldCheck,
  RefreshCw,
  Clock,
  ScrollText,
  Database,
  LogOut,
  PanelLeftClose,
  PanelLeftOpen,
  User,
  Sun,
  Moon,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuthStore } from '@/stores/auth-store'
import { useTheme } from '@/lib/use-theme'

export const SIDEBAR_COLLAPSED_STORAGE_KEY = 'bitprotector.sidebar.collapsed'

const navItems = [
  { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/files', label: 'Tracking Workspace', icon: Files },
  { to: '/drives', label: 'Drives', icon: HardDrive },
  { to: '/integrity', label: 'Integrity', icon: ShieldCheck },
  { to: '/sync', label: 'Sync Queue', icon: RefreshCw },
  { to: '/scheduler', label: 'Scheduler', icon: Clock },
  { to: '/logs', label: 'Logs', icon: ScrollText },
  { to: '/database', label: 'Database Backups', icon: Database },
]

function loadInitialCollapsedState() {
  if (typeof window === 'undefined') return false
  return window.localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY) === '1'
}

export function Sidebar() {
  const navigate = useNavigate()
  const username = useAuthStore((state) => state.username)
  const logout = useAuthStore((state) => state.logout)
  const [collapsed, setCollapsed] = useState(loadInitialCollapsedState)
  const [userMenuOpen, setUserMenuOpen] = useState(false)
  const userMenuRef = useRef<HTMLDivElement | null>(null)
  const { theme, toggle } = useTheme()

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, collapsed ? '1' : '0')
    setUserMenuOpen(false)
  }, [collapsed])

  useEffect(() => {
    if (!userMenuOpen) return

    const handlePointerDown = (event: MouseEvent) => {
      if (!userMenuRef.current?.contains(event.target as Node)) {
        setUserMenuOpen(false)
      }
    }

    window.addEventListener('mousedown', handlePointerDown)
    return () => {
      window.removeEventListener('mousedown', handlePointerDown)
    }
  }, [userMenuOpen])

  const handleLogout = () => {
    setUserMenuOpen(false)
    logout()
    navigate('/login')
  }

  return (
    <aside
      className={cn(
        'flex flex-col border-r border-border bg-card transition-[width] duration-200 ease-out',
        collapsed ? 'w-16' : 'w-56'
      )}
      data-testid="app-sidebar"
    >
      {/* Logo */}
      <div
        className={cn(
          'flex items-center border-b border-border',
          collapsed ? 'h-16 justify-center px-2' : 'h-14 gap-2 px-4'
        )}
      >
        <div className="flex min-w-0 items-center gap-2">
          <ShieldCheck className="h-6 w-6 shrink-0 text-primary" />
          {!collapsed ? (
            <span className="truncate text-sm font-semibold tracking-wide">BitProtector</span>
          ) : null}
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto py-3">
        <ul className={cn('space-y-0.5', collapsed ? 'px-1.5' : 'px-2')}>
          {navItems.map(({ to, label, icon: Icon }) => (
            <li key={to}>
              <NavLink
                to={to}
                data-testid={`nav-${to.slice(1)}`}
                aria-label={label}
                title={collapsed ? label : undefined}
                onClick={() => setUserMenuOpen(false)}
                className={({ isActive }) =>
                  cn(
                    'flex overflow-hidden whitespace-nowrap rounded-md py-2 text-sm transition-colors',
                    collapsed ? 'justify-center px-2' : 'items-center gap-2.5 px-3',
                    isActive
                      ? 'bg-primary/10 text-primary font-medium'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                  )
                }
              >
                <Icon className="h-5 w-5 flex-shrink-0" />
                {!collapsed ? <span className="truncate">{label}</span> : null}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>

      <div className={cn('flex px-2 pb-1', collapsed ? 'justify-center' : 'justify-end')}>
        <button
          type="button"
          onClick={() => setCollapsed((current) => !current)}
          className="flex p-1.5 text-muted-foreground transition-colors hover:text-accent-foreground"
          title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          data-testid="sidebar-toggle"
        >
          {collapsed ? (
            <PanelLeftOpen className="h-5 w-5" />
          ) : (
            <PanelLeftClose className="h-5 w-5" />
          )}
        </button>
      </div>

      <div className="border-t border-border p-2">
        <div className="relative" ref={userMenuRef}>
          <button
            type="button"
            onClick={() => setUserMenuOpen((current) => !current)}
            className={cn(
              'flex w-full items-center rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground',
              collapsed ? 'justify-center' : 'gap-1.5'
            )}
            data-testid="user-menu-trigger"
            aria-haspopup="menu"
            aria-expanded={userMenuOpen}
            title={collapsed ? (username ?? 'Unknown') : undefined}
          >
            <User className="h-5 w-5" />
            {!collapsed ? <span className="truncate">{username ?? 'Unknown'}</span> : null}
          </button>
          {userMenuOpen ? (
            <div
              role="menu"
              className={cn(
                'absolute z-50 w-40 rounded-md border border-border bg-popover p-1 shadow-md',
                collapsed ? 'bottom-0 left-full ml-2' : 'bottom-full left-0 mb-2'
              )}
            >
              <button
                type="button"
                onClick={() => toggle()}
                className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-popover-foreground transition-colors hover:bg-accent"
                data-testid="user-menu-theme-toggle"
                role="menuitem"
                aria-pressed={theme === 'dark'}
              >
                {theme === 'dark' ? <Sun className="h-5 w-5" /> : <Moon className="h-5 w-5" />}
                <span>{theme === 'dark' ? 'Light mode' : 'Dark mode'}</span>
              </button>
              <button
                type="button"
                onClick={handleLogout}
                className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm text-popover-foreground transition-colors hover:bg-accent"
                data-testid="user-menu-logout"
                role="menuitem"
              >
                <LogOut className="h-5 w-5" />
                <span>Logout</span>
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </aside>
  )
}
