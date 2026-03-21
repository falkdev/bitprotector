import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'

interface PaginationProps {
  page: number
  perPage: number
  total: number
  onPageChange: (page: number) => void
  className?: string
}

export function Pagination({ page, perPage, total, onPageChange, className }: PaginationProps) {
  const totalPages = Math.max(1, Math.ceil(total / perPage))
  const from = Math.min((page - 1) * perPage + 1, total)
  const to = Math.min(page * perPage, total)

  if (total === 0) return null

  return (
    <div className={cn('flex items-center justify-between text-sm text-muted-foreground', className)}>
      <span>
        {from}–{to} of {total}
      </span>
      <div className="flex items-center gap-1">
        <button
          onClick={() => onPageChange(page - 1)}
          disabled={page <= 1}
          className="rounded p-1 hover:bg-accent disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          aria-label="Previous page"
        >
          <ChevronLeft className="h-4 w-4" />
        </button>
        <span className="px-2">
          {page} / {totalPages}
        </span>
        <button
          onClick={() => onPageChange(page + 1)}
          disabled={page >= totalPages}
          className="rounded p-1 hover:bg-accent disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          aria-label="Next page"
        >
          <ChevronRight className="h-4 w-4" />
        </button>
      </div>
    </div>
  )
}
