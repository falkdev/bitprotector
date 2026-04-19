import type { SystemStatus } from '@/types/status'
import { cn } from '@/lib/utils'

interface MetricCardProps {
  label: string
  value: number
  variant?: 'default' | 'warning' | 'error' | 'info'
  testId: string
}

function MetricCard({ label, value, variant = 'default', testId }: MetricCardProps) {
  return (
    <div
      data-testid={testId}
      className={cn(
        'rounded-lg border p-4',
        variant === 'default' && 'border-border bg-card',
        variant === 'warning' && 'border-yellow-200 bg-yellow-50',
        variant === 'error' && 'border-red-200 bg-red-50',
        variant === 'info' && 'border-blue-200 bg-blue-50'
      )}
    >
      <p
        className={cn(
          'text-2xl font-bold',
          variant === 'default' && 'text-foreground',
          variant === 'warning' && 'text-yellow-700',
          variant === 'error' && 'text-red-700',
          variant === 'info' && 'text-blue-700'
        )}
      >
        {value.toLocaleString()}
      </p>
      <p className="mt-1 text-xs leading-snug text-muted-foreground">{label}</p>
    </div>
  )
}

interface StatusOverviewProps {
  status: SystemStatus
}

export function StatusOverview({ status }: StatusOverviewProps) {
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(9rem,1fr))] gap-4">
      <MetricCard
        label="Files Tracked"
        value={status.files_tracked}
        testId="status-metric-files-tracked"
      />
      <MetricCard
        label="Files Mirrored"
        value={status.files_mirrored}
        testId="status-metric-files-mirrored"
      />
      <MetricCard
        label="Pending Sync"
        value={status.pending_sync}
        variant={status.pending_sync > 0 ? 'warning' : 'default'}
        testId="status-metric-pending-sync"
      />
      <MetricCard
        label="Integrity Issues"
        value={status.integrity_issues}
        variant={status.integrity_issues > 0 ? 'error' : 'default'}
        testId="status-metric-integrity-issues"
      />
      <MetricCard
        label="Drive Pairs"
        value={status.drive_pairs}
        testId="status-metric-drive-pairs"
      />
      <MetricCard
        label="Degraded Pairs"
        value={status.degraded_pairs}
        variant={status.degraded_pairs > 0 ? 'error' : 'default'}
        testId="status-metric-degraded-pairs"
      />
      <MetricCard
        label="Active Secondary"
        value={status.active_secondary_pairs}
        variant={status.active_secondary_pairs > 0 ? 'warning' : 'default'}
        testId="status-metric-active-secondary"
      />
      <MetricCard
        label="Rebuilding"
        value={status.rebuilding_pairs}
        variant={status.rebuilding_pairs > 0 ? 'info' : 'default'}
        testId="status-metric-rebuilding"
      />
      <MetricCard
        label="Quiescing"
        value={status.quiescing_pairs}
        variant={status.quiescing_pairs > 0 ? 'warning' : 'default'}
        testId="status-metric-quiescing"
      />
    </div>
  )
}
