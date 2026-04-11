import { useEffect } from 'react'
import { toast } from 'sonner'
import { useStatusStore } from '@/stores/status-store'
import { useLogsStore } from '@/stores/logs-store'
import { integrityApi } from '@/api/integrity'
import { syncApi } from '@/api/sync'
import { databaseApi } from '@/api/database'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { PageIntro } from '@/components/shared/PageIntro'
import { StatusOverview } from '@/components/dashboard/StatusOverview'
import { QuickActions } from '@/components/dashboard/QuickActions'
import { RecentActivity } from '@/components/dashboard/RecentActivity'

export function DashboardPage() {
  const { status, loading: statusLoading, fetch: fetchStatus } = useStatusStore()
  const { entries, loading: logsLoading, fetch: fetchLogs } = useLogsStore()
  const integrityDisabled = status ? status.drive_pairs === 0 : false

  useEffect(() => {
    void fetchStatus()
    void fetchLogs({ per_page: 10 })
  }, [fetchStatus, fetchLogs])

  const handleIntegrityCheck = async () => {
    try {
      await integrityApi.startRun(undefined, false)
      toast.success('Integrity run started')
      void fetchStatus()
    } catch {
      toast.error('Failed to start integrity run')
    }
  }

  const handleProcessSync = async () => {
    try {
      const result = await syncApi.processQueue()
      toast.success(`Sync queue processed (${result.processed} item(s))`)
      void fetchStatus()
    } catch {
      toast.error('Failed to process sync queue')
    }
  }

  const handleRunBackup = async () => {
    const dbPath = import.meta.env.VITE_DB_PATH ?? '/var/lib/bitprotector/bitprotector.db'
    try {
      const results = await databaseApi.runBackup(dbPath)
      const failed = results.filter((r) => r.status === 'failed').length
      if (failed === 0) {
        toast.success(`Database backup completed (${results.length} destination(s))`)
      } else {
        toast.warning(`Backup completed with ${failed} failure(s)`)
      }
    } catch {
      toast.error('Database backup failed')
    }
  }

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

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
        <div className="lg:col-span-1">
          <QuickActions
            onIntegrityCheck={handleIntegrityCheck}
            onProcessSync={handleProcessSync}
            onRunBackup={handleRunBackup}
            integrityDisabled={integrityDisabled}
            integrityDisabledMessage={
              integrityDisabled ? 'Add a drive pair first to run integrity checks.' : undefined
            }
          />
        </div>
        <div className="lg:col-span-2">
          <RecentActivity entries={entries} loading={logsLoading} />
        </div>
      </div>
    </div>
  )
}
