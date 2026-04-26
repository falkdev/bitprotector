import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Plus, RefreshCw, ShieldCheck } from 'lucide-react'
import { schedulerApi } from '@/api/scheduler'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PageIntro } from '@/components/shared/PageIntro'
import { formatDate } from '@/lib/format'
import type {
  CreateScheduleRequest,
  ScheduleConfig,
  ScheduleTaskType,
  UpdateScheduleRequest,
} from '@/types/scheduler'

/* ── Interval helpers ─────────────────────────────────────────────────── */

type IntervalUnit = 'minutes' | 'hours' | 'days'

function intervalToSeconds(value: number, unit: IntervalUnit): number {
  const multiplier = { minutes: 60, hours: 3600, days: 86400 }
  return value * multiplier[unit]
}

function secondsToInterval(seconds: number): { value: number; unit: IntervalUnit } {
  if (seconds % 86400 === 0) return { value: seconds / 86400, unit: 'days' }
  if (seconds % 3600 === 0) return { value: seconds / 3600, unit: 'hours' }
  return { value: Math.round(seconds / 60), unit: 'minutes' }
}

function humanizeInterval(seconds: number): string {
  const { value, unit } = secondsToInterval(seconds)
  if (value === 1) return `Every ${unit.slice(0, -1)}`
  return `Every ${value} ${unit}`
}

/* ── Cron helpers ─────────────────────────────────────────────────────── */

function formatHour(hour: number): string {
  const locale =
    typeof navigator !== 'undefined' && navigator.language ? navigator.language : undefined
  const date = new Date(2000, 0, 1, hour, 0)
  return date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
}

function buildCronPresets(): { label: string; cron: string }[] {
  return [
    { label: 'Every hour', cron: '0 * * * *' },
    { label: 'Every 6 hours', cron: '0 */6 * * *' },
    { label: `Daily at ${formatHour(0)}`, cron: '0 0 * * *' },
    { label: `Daily at ${formatHour(2)}`, cron: '0 2 * * *' },
    { label: 'Weekly on Sunday', cron: '0 0 * * 0' },
  ]
}

const CRON_PRESETS = buildCronPresets()

function describeCron(expr: string): string {
  const preset = CRON_PRESETS.find((p) => p.cron === expr)
  if (preset) return preset.label
  return `Cron: ${expr}`
}

/* ── Table display ────────────────────────────────────────────────────── */

function describeSchedule(schedule: Pick<ScheduleConfig, 'cron_expr' | 'interval_seconds'>) {
  if (schedule.cron_expr) return describeCron(schedule.cron_expr)
  if (schedule.interval_seconds) return humanizeInterval(schedule.interval_seconds)
  return 'No schedule configured'
}

const TASK_LABELS: Record<ScheduleTaskType, string> = {
  sync: 'File Sync',
  integrity_check: 'Integrity Check',
}

/* ── Timing method type ───────────────────────────────────────────────── */

type TimingMethod = 'interval' | 'cron'

/* ── Form modal ───────────────────────────────────────────────────────── */

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
  const [timingMethod, setTimingMethod] = useState<TimingMethod>('interval')
  const [intervalValue, setIntervalValue] = useState('1')
  const [intervalUnit, setIntervalUnit] = useState<IntervalUnit>('hours')
  const [cronExpr, setCronExpr] = useState('')
  const [cronMode, setCronMode] = useState<'preset' | 'custom'>('preset')
  const [selectedPreset, setSelectedPreset] = useState('')
  const [maxDurationValue, setMaxDurationValue] = useState('')
  const [enabled, setEnabled] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setTaskType(schedule?.task_type ?? 'sync')
    setEnabled(schedule?.enabled ?? true)
    setError(null)
    setSaving(false)
    setMaxDurationValue(
      schedule?.max_duration_seconds != null ? String(schedule.max_duration_seconds) : '',
    )

    if (schedule?.cron_expr) {
      setTimingMethod('cron')
      setCronExpr(schedule.cron_expr)
      const preset = CRON_PRESETS.find((p) => p.cron === schedule.cron_expr)
      if (preset) {
        setCronMode('preset')
        setSelectedPreset(preset.cron)
      } else {
        setCronMode('custom')
        setSelectedPreset('')
      }
      setIntervalValue('1')
      setIntervalUnit('hours')
    } else if (schedule?.interval_seconds) {
      setTimingMethod('interval')
      const { value, unit } = secondsToInterval(schedule.interval_seconds)
      setIntervalValue(String(value))
      setIntervalUnit(unit)
      setCronExpr('')
      setCronMode('preset')
      setSelectedPreset('')
    } else {
      setTimingMethod('interval')
      setIntervalValue('1')
      setIntervalUnit('hours')
      setCronExpr('')
      setCronMode('preset')
      setSelectedPreset('')
    }
  }, [schedule])

  const selectPreset = (cron: string) => {
    setSelectedPreset(cron)
    setCronExpr(cron)
    setCronMode('preset')
  }

  const switchToCustomCron = () => {
    setCronMode('custom')
    setSelectedPreset('')
    setCronExpr('')
  }

  const submit = async () => {
    let finalCron: string | null = null
    let finalInterval: number | null = null

    if (timingMethod === 'cron') {
      const trimmed = cronExpr.trim()
      if (!trimmed) {
        setError('Select a preset or enter a custom cron expression.')
        return
      }
      finalCron = trimmed
    } else {
      const num = Number(intervalValue)
      if (!Number.isFinite(num) || num <= 0) {
        setError('Interval must be a positive number.')
        return
      }
      finalInterval = intervalToSeconds(num, intervalUnit)
    }

    setSaving(true)
    setError(null)

    // Parse optional max duration
    const maxDurationTrimmed = maxDurationValue.trim()
    let maxDurationSeconds: number | null | undefined = undefined
    if (maxDurationTrimmed !== '') {
      const parsed = Number(maxDurationTrimmed)
      if (!Number.isInteger(parsed) || parsed < 0) {
        setError('Max duration must be a non-negative whole number of seconds.')
        return
      }
      maxDurationSeconds = parsed > 0 ? parsed : null
    }

    try {
      if (schedule) {
        await onSave({
          cron_expr: finalCron,
          interval_seconds: finalInterval,
          enabled,
          max_duration_seconds: maxDurationSeconds,
        })
      } else {
        await onSave({
          task_type: taskType,
          cron_expr: finalCron ?? undefined,
          interval_seconds: finalInterval ?? undefined,
          enabled,
          max_duration_seconds: maxDurationSeconds ?? undefined,
        })
      }
    } finally {
      setSaving(false)
    }
  }

  const isEditing = !!schedule

  return (
    <ModalLayer>
      <div className="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">{isEditing ? 'Edit Schedule' : 'Add Schedule'}</h2>

        <div className="mt-5 space-y-6">
          {/* ── Section 1: Task type ───────────────────────────────── */}
          <fieldset disabled={isEditing}>
            <legend className="mb-2 text-sm font-medium">What do you want to schedule?</legend>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                aria-pressed={taskType === 'sync'}
                onClick={() => setTaskType('sync')}
                className={`flex flex-col items-center gap-2 rounded-lg border-2 p-4 text-center transition-colors ${
                  taskType === 'sync'
                    ? 'border-primary bg-primary/5'
                    : 'border-border hover:border-primary/40'
                } disabled:opacity-60`}
              >
                <RefreshCw className="h-6 w-6" />
                <span className="text-sm font-medium">File Sync</span>
                <span className="text-xs text-muted-foreground">
                  Sync tracked files to their backup destinations
                </span>
              </button>
              <button
                type="button"
                aria-pressed={taskType === 'integrity_check'}
                onClick={() => setTaskType('integrity_check')}
                className={`flex flex-col items-center gap-2 rounded-lg border-2 p-4 text-center transition-colors ${
                  taskType === 'integrity_check'
                    ? 'border-primary bg-primary/5'
                    : 'border-border hover:border-primary/40'
                } disabled:opacity-60`}
              >
                <ShieldCheck className="h-6 w-6" />
                <span className="text-sm font-medium">Integrity Check</span>
                <span className="text-xs text-muted-foreground">
                  Verify backup integrity by checking checksums
                </span>
              </button>
            </div>
          </fieldset>

          {/* ── Section 2: Timing ──────────────────────────────────── */}
          <fieldset>
            <legend className="mb-2 text-sm font-medium">How often should it run?</legend>

            {/* Timing method toggle */}
            <div className="mb-3 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => setTimingMethod('interval')}
                className={`shrink-0 whitespace-nowrap rounded-md border border-border px-3 py-2 text-sm font-medium transition-colors ${
                  timingMethod === 'interval'
                    ? 'border-primary bg-primary text-primary-foreground'
                    : 'hover:bg-accent'
                }`}
              >
                Recurring Interval
              </button>
              <button
                type="button"
                onClick={() => setTimingMethod('cron')}
                className={`shrink-0 whitespace-nowrap rounded-md border border-border px-3 py-2 text-sm font-medium transition-colors ${
                  timingMethod === 'cron'
                    ? 'border-primary bg-primary text-primary-foreground'
                    : 'hover:bg-accent'
                }`}
              >
                Cron Schedule
              </button>
            </div>

            {timingMethod === 'interval' ? (
              <div>
                <label className="mb-1 block text-xs text-muted-foreground">
                  Run this task every:
                </label>
                <div className="flex gap-2">
                  <input
                    aria-label="Interval value"
                    type="number"
                    min={1}
                    value={intervalValue}
                    onChange={(e) => setIntervalValue(e.target.value)}
                    className="w-24 rounded-md border border-input bg-background px-3 py-2 text-sm"
                  />
                  <select
                    aria-label="Interval unit"
                    value={intervalUnit}
                    onChange={(e) => setIntervalUnit(e.target.value as IntervalUnit)}
                    className="rounded-md border border-input bg-background px-3 py-2 text-sm"
                  >
                    <option value="minutes">Minutes</option>
                    <option value="hours">Hours</option>
                    <option value="days">Days</option>
                  </select>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <label className="mb-1 block text-xs text-muted-foreground">
                  Choose a preset or enter a custom cron expression:
                </label>
                <div className="flex flex-wrap gap-2">
                  {CRON_PRESETS.map((preset) => (
                    <button
                      key={preset.cron}
                      type="button"
                      onClick={() => selectPreset(preset.cron)}
                      className={`rounded-md border px-3 py-1.5 text-xs transition-colors ${
                        cronMode === 'preset' && selectedPreset === preset.cron
                          ? 'border-primary bg-primary/10 font-medium text-primary'
                          : 'border-border hover:border-primary/40'
                      }`}
                    >
                      {preset.label}
                    </button>
                  ))}
                  <button
                    type="button"
                    onClick={switchToCustomCron}
                    className={`rounded-md border px-3 py-1.5 text-xs transition-colors ${
                      cronMode === 'custom'
                        ? 'border-primary bg-primary/10 font-medium text-primary'
                        : 'border-border hover:border-primary/40'
                    }`}
                  >
                    Custom…
                  </button>
                </div>
                {cronMode === 'custom' && (
                  <div>
                    <input
                      aria-label="Custom cron expression"
                      value={cronExpr}
                      onChange={(e) => setCronExpr(e.target.value)}
                      placeholder="min hour dom month dow"
                      className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                    />
                    <p className="mt-1 text-xs text-muted-foreground">
                      5-field format: minute (0-59) hour (0-23) day-of-month (1-31) month (1-12)
                      day-of-week (0-6, Sun=0)
                    </p>
                  </div>
                )}
              </div>
            )}
          </fieldset>

          {/* ── Section 3: Max duration ────────────────────────────── */}
          <fieldset>
            <legend className="mb-2 text-sm font-medium">
              Maximum run duration{' '}
              <span className="font-normal text-muted-foreground">(optional)</span>
            </legend>
            <div className="flex items-center gap-2">
              <input
                aria-label="Max duration in seconds"
                type="number"
                min={0}
                step={1}
                value={maxDurationValue}
                onChange={(e) => setMaxDurationValue(e.target.value)}
                placeholder="Unlimited"
                className="w-36 rounded-md border border-input bg-background px-3 py-2 text-sm"
              />
              <span className="text-sm text-muted-foreground">seconds</span>
            </div>
            <p className="mt-1 text-xs text-muted-foreground">
              The job stops gracefully after this duration; remaining items resume at the next
              interval. Leave blank for unlimited.
            </p>
          </fieldset>

          {/* ── Section 4: Options ─────────────────────────────────── */}
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
            />
            Enable schedule immediately
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
            {saving ? 'Saving…' : isEditing ? 'Save Changes' : 'Create Schedule'}
          </button>
        </div>
      </div>
    </ModalLayer>
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
      <div className="space-y-6">
        <PageIntro
          title="Scheduler"
          subtitle="Configure recurring sync and integrity jobs for automated maintenance."
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
        title="Scheduler"
        subtitle="Configure recurring sync and integrity jobs for automated maintenance."
        actions={
          <button
            onClick={() => setShowCreate(true)}
            className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            <Plus className="h-4 w-4" />
            Add Schedule
          </button>
        }
      />

      <DataTable
        tableTestId="scheduler-table"
        columns={[
          {
            key: 'task_type',
            header: 'Task',
            cell: (schedule) => TASK_LABELS[schedule.task_type] ?? schedule.task_type,
          },
          {
            key: 'schedule',
            header: 'Schedule',
            cell: (schedule) => describeSchedule(schedule),
          },
          {
            key: 'max_duration',
            header: 'Max Duration',
            cell: (schedule) =>
              schedule.max_duration_seconds != null
                ? `${schedule.max_duration_seconds}s`
                : '—',
          },
          {
            key: 'enabled',
            header: 'Enabled',
            cell: (schedule) => (
              <button
                onClick={() => void toggleEnabled(schedule)}
                className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                  schedule.enabled ? 'bg-green-100 text-green-800' : 'bg-gray-100 text-gray-700'
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
        <ScheduleFormModal schedule={formTarget} onClose={closeForm} onSave={saveSchedule} />
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title="Delete schedule?"
        description={`Delete the ${deleteTarget ? TASK_LABELS[deleteTarget.task_type] : 'selected'} schedule?`}
        confirmLabel="Delete"
        destructive
        onConfirm={deleteSchedule}
      />
    </div>
  )
}
