import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Plus } from 'lucide-react'
import { schedulerApi } from '@/api/scheduler'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { formatDate } from '@/lib/format'
import type {
  CreateScheduleRequest,
  ScheduleConfig,
  ScheduleTaskType,
  UpdateScheduleRequest,
} from '@/types/scheduler'

function describeSchedule(schedule: Pick<ScheduleConfig, 'cron_expr' | 'interval_seconds'>) {
  if (schedule.cron_expr) {
    return `Cron: ${schedule.cron_expr}`
  }
  if (schedule.interval_seconds) {
    return `Every ${schedule.interval_seconds}s`
  }
  return 'No schedule configured'
}

function ScheduleFormModal({
  schedule,
  onClose,
  onSave,
}: {
  schedule: ScheduleConfig | null
  onClose: () => void
  onSave: (data: CreateScheduleRequest | UpdateScheduleRequest) => Promise<void>
}) {
  const [taskType, setTaskType] = useState<ScheduleTaskType>('sync')
  const [cronExpr, setCronExpr] = useState('')
  const [intervalSeconds, setIntervalSeconds] = useState('')
  const [enabled, setEnabled] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setTaskType(schedule?.task_type ?? 'sync')
    setCronExpr(schedule?.cron_expr ?? '')
    setIntervalSeconds(schedule?.interval_seconds ? String(schedule.interval_seconds) : '')
    setEnabled(schedule?.enabled ?? true)
    setError(null)
    setSaving(false)
  }, [schedule])

  const submit = async () => {
    const normalizedCron = cronExpr.trim() || null
    const normalizedInterval = intervalSeconds.trim() ? Number(intervalSeconds.trim()) : null

    if (!normalizedCron && !normalizedInterval) {
      setError('Provide either a cron expression or an interval in seconds.')
      return
    }

    if (normalizedInterval !== null && (!Number.isFinite(normalizedInterval) || normalizedInterval <= 0)) {
      setError('Interval must be a positive number.')
      return
    }

    setSaving(true)
    setError(null)

    try {
      if (schedule) {
        await onSave({
          cron_expr: normalizedCron,
          interval_seconds: normalizedInterval,
          enabled,
        })
      } else {
        await onSave({
          task_type: taskType,
          cron_expr: normalizedCron ?? undefined,
          interval_seconds: normalizedInterval ?? undefined,
          enabled,
        })
      }
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">
          {schedule ? 'Edit Schedule' : 'Add Schedule'}
        </h2>

        <div className="mt-4 space-y-4">
          <div>
            <label htmlFor="task-type" className="mb-1 block text-sm font-medium">
              Task Type
            </label>
            <select
              id="task-type"
              value={taskType}
              onChange={(event) => setTaskType(event.target.value as ScheduleTaskType)}
              disabled={!!schedule}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm disabled:opacity-70"
            >
              <option value="sync">sync</option>
              <option value="integrity_check">integrity_check</option>
            </select>
          </div>

          <div>
            <label htmlFor="cron-expr" className="mb-1 block text-sm font-medium">
              Cron Expression
            </label>
            <input
              id="cron-expr"
              value={cronExpr}
              onChange={(event) => setCronExpr(event.target.value)}
              placeholder="0 2 * * *"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>

          <div>
            <label htmlFor="interval-seconds" className="mb-1 block text-sm font-medium">
              Interval Seconds
            </label>
            <input
              id="interval-seconds"
              type="number"
              min={1}
              value={intervalSeconds}
              onChange={(event) => setIntervalSeconds(event.target.value)}
              placeholder="3600"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>

          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(event) => setEnabled(event.target.checked)}
            />
            Enabled
          </label>

          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void submit()}
            disabled={saving}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {saving ? 'Saving…' : schedule ? 'Save Changes' : 'Create Schedule'}
          </button>
        </div>
      </div>
    </div>
  )
}

export function SchedulerPage() {
  const [schedules, setSchedules] = useState<ScheduleConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [formTarget, setFormTarget] = useState<ScheduleConfig | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<ScheduleConfig | null>(null)

  const loadSchedules = async () => {
    setLoading(true)
    try {
      const nextSchedules = await schedulerApi.list()
      setSchedules(nextSchedules)
    } catch {
      toast.error('Failed to load schedules')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadSchedules()
  }, [])

  const closeForm = () => {
    setShowCreate(false)
    setFormTarget(null)
  }

  const saveSchedule = async (data: CreateScheduleRequest | UpdateScheduleRequest) => {
    try {
      if (formTarget) {
        await schedulerApi.update(formTarget.id, data as UpdateScheduleRequest)
        toast.success('Schedule updated')
      } else {
        await schedulerApi.create(data as CreateScheduleRequest)
        toast.success('Schedule created')
      }
      closeForm()
      await loadSchedules()
    } catch {
      toast.error('Failed to save schedule')
    }
  }

  const toggleEnabled = async (schedule: ScheduleConfig) => {
    try {
      const updated = await schedulerApi.update(schedule.id, {
        enabled: !schedule.enabled,
      })
      setSchedules((current) =>
        current.map((entry) => (entry.id === schedule.id ? updated : entry))
      )
      toast.success(`Schedule ${updated.enabled ? 'enabled' : 'disabled'}`)
    } catch {
      toast.error('Failed to update schedule state')
    }
  }

  const deleteSchedule = async () => {
    if (!deleteTarget) return

    try {
      await schedulerApi.delete(deleteTarget.id)
      setSchedules((current) => current.filter((entry) => entry.id !== deleteTarget.id))
      setDeleteTarget(null)
      toast.success('Schedule deleted')
    } catch {
      toast.error('Failed to delete schedule')
    }
  }

  if (loading && schedules.length === 0) {
    return (
      <div className="flex items-center justify-center py-16">
        <LoadingSpinner />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Scheduler</h1>
          <p className="text-sm text-muted-foreground">
            Configure recurring sync and integrity tasks.
          </p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          Add Schedule
        </button>
      </div>

      <DataTable
        tableTestId="scheduler-table"
        columns={[
          {
            key: 'task_type',
            header: 'Task',
            cell: (schedule) => schedule.task_type,
          },
          {
            key: 'schedule',
            header: 'Schedule',
            cell: (schedule) => describeSchedule(schedule),
          },
          {
            key: 'enabled',
            header: 'Enabled',
            cell: (schedule) => (
              <button
                onClick={() => void toggleEnabled(schedule)}
                className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                  schedule.enabled
                    ? 'bg-green-100 text-green-800'
                    : 'bg-gray-100 text-gray-700'
                }`}
              >
                {schedule.enabled ? 'Enabled' : 'Disabled'}
              </button>
            ),
          },
          {
            key: 'last_run',
            header: 'Last Run',
            cell: (schedule) => formatDate(schedule.last_run),
          },
          {
            key: 'next_run',
            header: 'Next Run',
            cell: (schedule) => formatDate(schedule.next_run),
          },
          {
            key: 'actions',
            header: '',
            cell: (schedule) => (
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setFormTarget(schedule)}
                  className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
                >
                  Edit
                </button>
                <button
                  onClick={() => setDeleteTarget(schedule)}
                  className="rounded-md border border-border px-3 py-1.5 text-xs text-destructive hover:bg-destructive/10"
                >
                  Delete
                </button>
              </div>
            ),
          },
        ]}
        data={schedules}
        rowKey={(schedule) => schedule.id}
        rowTestId={(schedule) => `schedule-row-${schedule.id}`}
        emptyState={
          <EmptyState
            title="No schedules configured"
            description="Create a sync or integrity schedule to automate background work."
          />
        }
      />

      {(showCreate || formTarget) && (
        <ScheduleFormModal
          schedule={formTarget}
          onClose={closeForm}
          onSave={saveSchedule}
        />
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title="Delete schedule?"
        description={`Delete the ${deleteTarget?.task_type ?? 'selected'} schedule?`}
        confirmLabel="Delete"
        destructive
        onConfirm={deleteSchedule}
      />
    </div>
  )
}
