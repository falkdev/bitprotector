import { ChevronRight, Home } from 'lucide-react'
import { cn } from '@/lib/utils'

interface BreadcrumbNavProps {
  path: string
  onNavigate: (path: string) => void
}

export function BreadcrumbNav({ path, onNavigate }: BreadcrumbNavProps) {
  const normalized = path?.trim() || ''
  const parts = normalized ? normalized.split('/').filter(Boolean) : []

  return (
    <nav className="flex items-center gap-1 text-sm" aria-label="Breadcrumb" data-testid="breadcrumb-nav">
      <button
        className="flex items-center gap-1 text-blue-600 hover:text-blue-800"
        onClick={() => onNavigate('')}
      >
        <Home className="h-4 w-4" />
        <span>All</span>
      </button>
      {parts.map((part, idx) => {
        const segPath = `/${parts.slice(0, idx + 1).join('/')}`
        const isLast = idx === parts.length - 1
        return (
          <span key={segPath} className="flex items-center gap-1">
            <ChevronRight className="h-3 w-3 text-gray-400" />
            <button
              className={cn(
                'hover:text-blue-800',
                isLast ? 'font-medium text-gray-900' : 'text-blue-600'
              )}
              onClick={() => onNavigate(segPath)}
              disabled={isLast}
            >
              {part}
            </button>
          </span>
        )
      })}
    </nav>
  )
}
