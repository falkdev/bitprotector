import { useEffect } from 'react'
import { toast } from 'sonner'
import { useStatusStore } from '@/stores/status-store'
import { useLogsStore } from '@/stores/logs-store'
import { integrityApi } from '@/api/integrity'
import { syncApi } from '@/api/sync'
import { databaseApi } from '@/api/database'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { StatusOverview } from '@/components/dashboard/StatusOverview'
import { QuickActions } from '@/components/dashboard/QuickActions'
import { RecentActivity } from '@/components/dashboard/RecentActivity'

export function DashboardPage() {
  const { status, loading: statusLoading, fetch: fetchStatus } = useStatusStore()
  const { entries, loading: logsLoading, fetch: fetchLogs } = useLogsStore()

  useEffect(() => {
    void fetchStatus()
    void fetchLogs({ per_page: 10 })
  }, [fetchStatus, fetchLogs])

  const handleIntegrityCheck = async () => {
    try {
      const result = await integrityApi.checkAll()
      const issues = result.results.filter((r) => r.status !== 'ok').length
      if (issues === 0) {
        toast.success('All files passed integrity check')
      } else {
        toast.warning(`Integrity check complete - ${issues} issue(s) found`)
      }
      void fetchStatus()
    } catch {
      toast.error('Integrity check failed')
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
      <div className="flex items-center justify-center py-16">
        <LoadingSpinner />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Dashboard</h1>
        <p className="text-sm text-muted-foreground">System status overview</p>
      </div>

      {status && <StatusOverview status={status} />}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
        <div className="lg:col-span-1">
          <QuickActions
            onIntegrityCheck={handleIntegrityCheck}
            onProcessSync={handleProcessSync}
            onRunBackup={handleRunBackup}
          />
        </div>
        <div className="lg:col-span-2">
          <RecentActivity entries={entries} loading={logsLoading} />
        </div>
      </div>
    </div>
  )
}
