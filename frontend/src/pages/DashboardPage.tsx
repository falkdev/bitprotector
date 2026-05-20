import { useEffect } from 'react'
import { useStatusStore } from '@/stores/status-store'
import { useLogsStore } from '@/stores/logs-store'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { PageIntro } from '@/components/shared/PageIntro'
import { StatusOverview } from '@/components/dashboard/StatusOverview'
import { RecentActivity } from '@/components/dashboard/RecentActivity'

export function DashboardPage() {
  const { status, loading: statusLoading, fetch: fetchStatus } = useStatusStore()
  const { entries, loading: logsLoading, fetch: fetchLogs } = useLogsStore()

  useEffect(() => {
    void fetchStatus()
    void fetchLogs({ per_page: 10 })
  }, [fetchStatus, fetchLogs])

  if (statusLoading && !status) {
    return (
      <div className="space-y-6">
        <PageIntro
          title="Dashboard"
          subtitle="Live overview of system health, sync activity, and integrity status."
        />
        <div className="flex items-center justify-center py-16">
          <LoadingSpinner />
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <PageIntro
        title="Dashboard"
        subtitle="Live overview of system health, sync activity, and integrity status."
      />
      {status && <StatusOverview status={status} />}

      <div className="grid grid-cols-1 gap-6">
        <div>
          <RecentActivity entries={entries} loading={logsLoading} />
        </div>
      </div>
    </div>
  )
}
