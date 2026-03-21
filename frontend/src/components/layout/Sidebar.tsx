import { NavLink } from 'react-router-dom'
import {
  LayoutDashboard,
  Files,
  HardDrive,
  FolderOpen,
  ShieldCheck,
  RefreshCw,
  GitBranch,
  Clock,
  ScrollText,
  Database,
} from 'lucide-react'
import { cn } from '@/lib/utils'

const navItems = [
  { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { to: '/files', label: 'File Browser', icon: Files },
  { to: '/drives', label: 'Drives', icon: HardDrive },
  { to: '/folders', label: 'Folders', icon: FolderOpen },
  { to: '/integrity', label: 'Integrity', icon: ShieldCheck },
  { to: '/sync', label: 'Sync Queue', icon: RefreshCw },
  { to: '/virtual-paths', label: 'Virtual Paths', icon: GitBranch },
  { to: '/scheduler', label: 'Scheduler', icon: Clock },
  { to: '/logs', label: 'Logs', icon: ScrollText },
  { to: '/database', label: 'Database Backups', icon: Database },
]

export function Sidebar() {
  return (
    <aside className="flex w-56 flex-col border-r border-border bg-card">
      {/* Logo */}
      <div className="flex h-14 items-center gap-2 border-b border-border px-4">
        <ShieldCheck className="h-5 w-5 text-primary" />
        <span className="font-semibold text-sm tracking-wide">BitProtector</span>
      </div>

      {/* Navigation */}
      <nav className="flex-1 overflow-y-auto py-3">
        <ul className="space-y-0.5 px-2">
          {navItems.map(({ to, label, icon: Icon }) => (
            <li key={to}>
              <NavLink
                to={to}
                data-testid={`nav-${to.slice(1)}`}
                className={({ isActive }) =>
                  cn(
                    'flex items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors',
                    isActive
                      ? 'bg-primary/10 text-primary font-medium'
                      : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                  )
                }
              >
                <Icon className="h-4 w-4 flex-shrink-0" />
                {label}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
    </aside>
  )
}
