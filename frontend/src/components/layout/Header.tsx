import { useNavigate, useLocation } from 'react-router-dom'
import { LogOut, User } from 'lucide-react'
import { useAuthStore } from '@/stores/auth-store'

/** Convert a route path to a readable breadcrumb label */
function pathToLabel(pathname: string): string {
  const segments = pathname.split('/').filter(Boolean)
  if (!segments.length) return 'Dashboard'
  return segments
    .map((s) => s.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()))
    .join(' › ')
}

export function Header() {
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const username = useAuthStore((s) => s.username)
  const logout = useAuthStore((s) => s.logout)

  const handleLogout = () => {
    logout()
    navigate('/login')
  }

  return (
    <header className="flex h-14 items-center justify-between border-b border-border bg-card px-6">
      <span className="text-sm text-muted-foreground">{pathToLabel(pathname)}</span>

      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1.5 text-sm text-muted-foreground">
          <User className="h-4 w-4" />
          <span>{username}</span>
        </div>
        <button
          onClick={handleLogout}
          title="Log out"
          className="flex items-center gap-1 rounded-md px-2 py-1 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground transition-colors"
          data-testid="logout-button"
        >
          <LogOut className="h-4 w-4" />
          <span>Logout</span>
        </button>
      </div>
    </header>
  )
}
