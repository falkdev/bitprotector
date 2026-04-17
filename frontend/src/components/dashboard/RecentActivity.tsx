import type { EventLogEntry } from '@/types/log'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { EmptyState } from '@/components/shared/EmptyState'
import { formatRelative } from '@/lib/format'
import { cn } from '@/lib/utils'

const eventColors: Record<string, string> = {
  file_created: 'text-blue-600',
  file_edited: 'text-blue-600',
  file_mirrored: 'text-green-600',
  file_untracked: 'text-gray-600',
  integrity_pass: 'text-green-600',
  integrity_fail: 'text-red-600',
  recovery_success: 'text-green-600',
  recovery_fail: 'text-red-600',
  both_corrupted: 'text-red-700 font-semibold',
  change_detected: 'text-yellow-600',
  sync_completed: 'text-green-600',
  sync_failed: 'text-red-600',
  folder_tracked: 'text-blue-600',
  folder_untracked: 'text-gray-600',
  integrity_run_started: 'text-purple-600',
  integrity_run_completed: 'text-purple-600',
  drive_created: 'text-blue-600',
  drive_updated: 'text-blue-600',
  drive_deleted: 'text-gray-600',
  drive_failover: 'text-red-600',
  drive_quiescing: 'text-yellow-600',
  drive_quiesce_cancelled: 'text-gray-600',
  drive_failure_confirmed: 'text-red-600',
  drive_replacement_assigned: 'text-yellow-600',
  drive_rebuild_completed: 'text-green-600',
}

const eventBadgeStyles: Record<string, string> = {
  file_created: 'bg-blue-100 text-blue-800',
  file_edited: 'bg-blue-100 text-blue-800',
  file_mirrored: 'bg-green-100 text-green-800',
  file_untracked: 'bg-gray-100 text-gray-800',
  integrity_pass: 'bg-green-100 text-green-800',
  integrity_fail: 'bg-red-100 text-red-800',
  recovery_success: 'bg-green-100 text-green-800',
  recovery_fail: 'bg-red-100 text-red-800',
  both_corrupted: 'bg-red-100 text-red-900',
  change_detected: 'bg-yellow-100 text-yellow-800',
  sync_completed: 'bg-green-100 text-green-800',
  sync_failed: 'bg-red-100 text-red-800',
  folder_tracked: 'bg-blue-100 text-blue-800',
  folder_untracked: 'bg-gray-100 text-gray-800',
  integrity_run_started: 'bg-purple-100 text-purple-800',
  integrity_run_completed: 'bg-purple-100 text-purple-800',
  drive_created: 'bg-blue-100 text-blue-800',
  drive_updated: 'bg-blue-100 text-blue-800',
  drive_deleted: 'bg-gray-100 text-gray-800',
  drive_failover: 'bg-red-100 text-red-800',
  drive_quiescing: 'bg-yellow-100 text-yellow-800',
  drive_quiesce_cancelled: 'bg-gray-100 text-gray-800',
  drive_failure_confirmed: 'bg-red-100 text-red-800',
  drive_replacement_assigned: 'bg-yellow-100 text-yellow-800',
  drive_rebuild_completed: 'bg-green-100 text-green-800',
}

function formatEventType(eventType: string) {
  return eventType.replace(/_/g, ' ')
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
        <ul className="space-y-2 text-sm">
          {entries.map((entry) => (
            <li key={entry.id} className="flex items-start gap-3">
              <span
                className={cn(
                  'mt-0.5 flex-shrink-0 rounded-full px-1.5 py-0.5 text-[10px] font-medium leading-tight',
                  eventBadgeStyles[entry.event_type] ?? 'bg-gray-100 text-gray-800'
                )}
              >
                {formatEventType(entry.event_type)}
              </span>
              <div className="flex-1 min-w-0">
                <span className={cn(eventColors[entry.event_type] ?? 'text-foreground')}>
                  {entry.message}
                </span>
                {entry.file_path && (
                  <span className="ml-1 font-mono text-xs text-muted-foreground truncate">
                    {entry.file_path}
                  </span>
                )}
              </div>
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
