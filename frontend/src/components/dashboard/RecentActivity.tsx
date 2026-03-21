import type { EventLogEntry } from '@/types/log'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { EmptyState } from '@/components/shared/EmptyState'
import { formatRelative } from '@/lib/format'
import { cn } from '@/lib/utils'

const eventColors: Record<string, string> = {
  file_created: 'text-blue-600',
  file_edited: 'text-blue-600',
  file_mirrored: 'text-green-600',
  integrity_pass: 'text-green-600',
  integrity_fail: 'text-red-600',
  recovery_success: 'text-green-600',
  recovery_fail: 'text-red-600',
  both_corrupted: 'text-red-700 font-semibold',
  change_detected: 'text-yellow-600',
  sync_completed: 'text-green-600',
  sync_failed: 'text-red-600',
}

interface RecentActivityProps {
  entries: EventLogEntry[]
  loading: boolean
}

export function RecentActivity({ entries, loading }: RecentActivityProps) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <h2 className="mb-3 text-sm font-semibold">Recent Activity</h2>
      {loading && entries.length === 0 ? (
        <div className="flex justify-center py-8">
          <LoadingSpinner size="sm" />
        </div>
      ) : entries.length === 0 ? (
        <EmptyState title="No recent activity" description="Events will appear here" />
      ) : (
        <ul className="space-y-1.5 text-sm">
          {entries.map((entry) => (
            <li key={entry.id} className="flex items-start justify-between gap-4">
              <span className={cn('flex-1', eventColors[entry.event_type] ?? 'text-foreground')}>
                {entry.message}
              </span>
              <span className="flex-shrink-0 text-xs text-muted-foreground">
                {formatRelative(entry.created_at)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
