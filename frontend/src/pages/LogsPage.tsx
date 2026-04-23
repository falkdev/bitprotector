import { useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Search, ChevronLeft, ChevronRight } from 'lucide-react'
import { logsApi } from '@/api/logs'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { PageIntro } from '@/components/shared/PageIntro'
import { formatDate } from '@/lib/format'
import type { EventLogEntry, EventType, LogsQueryParams } from '@/types/log'

const EVENT_TYPES: EventType[] = [
  'file_created',
  'file_edited',
  'file_mirrored',
  'file_untracked',
  'integrity_pass',
  'integrity_fail',
  'recovery_success',
  'recovery_fail',
  'both_corrupted',
  'change_detected',
  'sync_completed',
  'sync_failed',
  'folder_tracked',
  'folder_untracked',
  'integrity_run_started',
  'integrity_run_completed',
  'drive_created',
  'drive_updated',
  'drive_deleted',
  'drive_failover',
  'drive_quiescing',
  'drive_quiesce_cancelled',
  'drive_failure_confirmed',
  'drive_replacement_assigned',
  'drive_rebuild_completed',
]

const EVENT_STYLES: Record<EventType, string> = {
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

const PER_PAGE = 25

interface FilterState {
  event_type: 'all' | EventType
  file_id: string
  from: string
  to: string
}

function toIsoDate(value: string) {
  if (!value) return undefined
  const parsed = new Date(value)
  return Number.isNaN(parsed.getTime()) ? undefined : parsed.toISOString()
}

function formatEventType(eventType: EventType) {
  return eventType.replace(/_/g, ' ')
}

function StructuredDetails({ details }: { details: string | null }) {
  if (!details) {
    return <p className="mt-3 text-xs text-muted-foreground italic">No additional details</p>
  }

  try {
    const parsed = JSON.parse(details) as Record<string, unknown>
    if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
      return (
        <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-md bg-muted p-3 text-xs">
          {Object.entries(parsed).map(([key, value]) => (
            <div key={key} className="contents">
              <dt className="font-medium text-muted-foreground">{key.replace(/_/g, ' ')}</dt>
              <dd className="font-mono break-all">{value === null ? '—' : String(value)}</dd>
            </div>
          ))}
        </dl>
      )
    }
  } catch {
    // Not JSON — fall through to raw display
  }

  return <pre className="mt-3 overflow-x-auto rounded-md bg-muted p-3 text-xs">{details}</pre>
}

export function LogsPage() {
  const [entries, setEntries] = useState<EventLogEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [page, setPage] = useState(1)
  const [hasNext, setHasNext] = useState(false)
  const [filters, setFilters] = useState<FilterState>({
    event_type: 'all',
    file_id: '',
    from: '',
    to: '',
  })
  const [appliedFilters, setAppliedFilters] = useState<FilterState>({
    event_type: 'all',
    file_id: '',
    from: '',
    to: '',
  })
  const [expandedLogId, setExpandedLogId] = useState<number | null>(null)

  useEffect(() => {
    let active = true

    const loadEntries = async () => {
      setLoading(true)
      const params: LogsQueryParams = {
        page,
        per_page: PER_PAGE,
        event_type: appliedFilters.event_type === 'all' ? undefined : appliedFilters.event_type,
        file_id: appliedFilters.file_id ? Number(appliedFilters.file_id) : undefined,
        from: toIsoDate(appliedFilters.from),
        to: toIsoDate(appliedFilters.to),
      }

      try {
        const nextEntries = await logsApi.list(params)
        if (active) {
          setEntries(nextEntries)
          setHasNext(nextEntries.length === PER_PAGE)
          if (expandedLogId && !nextEntries.some((entry) => entry.id === expandedLogId)) {
            setExpandedLogId(null)
          }
        }
      } catch {
        if (active) {
          toast.error('Failed to load event logs')
        }
      } finally {
        if (active) {
          setLoading(false)
        }
      }
    }

    void loadEntries()
    return () => {
      active = false
    }
  }, [page, appliedFilters, expandedLogId])

  const expandedEntry = useMemo(
    () => entries.find((entry) => entry.id === expandedLogId) ?? null,
    [entries, expandedLogId]
  )

  const applyFilters = () => {
    setPage(1)
    setAppliedFilters(filters)
  }

  const resetFilters = () => {
    const nextFilters: FilterState = {
      event_type: 'all',
      file_id: '',
      from: '',
      to: '',
    }
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    setPage(1)
  }

  return (
    <div className="flex min-h-full flex-col gap-6">
      <PageIntro
        title="Logs"
        subtitle="Browse system events, filter by type/date, and investigate recent activity."
      />

      <div className="rounded-lg border border-border bg-card p-4">
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-5 [&>*]:min-w-0">
          <div className="min-w-0">
            <label htmlFor="log-event-type" className="mb-1 block text-sm font-medium">
              Event Type
            </label>
            <select
              id="log-event-type"
              value={filters.event_type}
              onChange={(event) =>
                setFilters((current) => ({
                  ...current,
                  event_type: event.target.value as FilterState['event_type'],
                }))
              }
              className="w-full min-w-0 rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="all">All event types</option>
              {EVENT_TYPES.map((eventType) => (
                <option key={eventType} value={eventType}>
                  {formatEventType(eventType)}
                </option>
              ))}
            </select>
          </div>
          <div className="min-w-0">
            <label htmlFor="log-file-id" className="mb-1 block text-sm font-medium">
              File ID
            </label>
            <input
              id="log-file-id"
              type="number"
              min={1}
              value={filters.file_id}
              onChange={(event) =>
                setFilters((current) => ({ ...current, file_id: event.target.value }))
              }
              placeholder="123"
              className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <div className="min-w-0">
            <label htmlFor="log-from" className="mb-1 block text-sm font-medium">
              From
            </label>
            <input
              id="log-from"
              type="datetime-local"
              value={filters.from}
              onChange={(event) =>
                setFilters((current) => ({ ...current, from: event.target.value }))
              }
              className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <div className="min-w-0">
            <label htmlFor="log-to" className="mb-1 block text-sm font-medium">
              To
            </label>
            <input
              id="log-to"
              type="datetime-local"
              value={filters.to}
              onChange={(event) =>
                setFilters((current) => ({ ...current, to: event.target.value }))
              }
              className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <div className="flex flex-col items-start gap-2 sm:flex-row sm:flex-wrap sm:items-end">
            <button
              onClick={applyFilters}
              className="inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              <Search className="h-4 w-4" />
              Apply
            </button>
            <button
              onClick={resetFilters}
              className="whitespace-nowrap rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
            >
              Reset
            </button>
          </div>
        </div>
      </div>

      <div className="flex flex-1 flex-col gap-6">
        {loading && entries.length === 0 ? (
          <div className="flex items-center justify-center py-16">
            <LoadingSpinner />
          </div>
        ) : (
          <DataTable
            tableTestId="logs-table"
            columns={[
              {
                key: 'event_type',
                header: 'Event Type',
                cell: (entry) => (
                  <span
                    className={`rounded-full px-2 py-0.5 text-xs font-medium ${EVENT_STYLES[entry.event_type]}`}
                  >
                    {formatEventType(entry.event_type)}
                  </span>
                ),
              },
              {
                key: 'tracked_file_id',
                header: 'File',
                cell: (entry) =>
                  entry.file_path ? (
                    <span className="font-mono text-xs" title={`File #${entry.tracked_file_id}`}>
                      {entry.file_path}
                    </span>
                  ) : entry.tracked_file_id ? (
                    <span className="font-mono text-xs">#{entry.tracked_file_id}</span>
                  ) : (
                    '—'
                  ),
              },
              {
                key: 'message',
                header: 'Message',
                cell: (entry) => entry.message,
              },
              {
                key: 'created_at',
                header: 'Created',
                cell: (entry) => formatDate(entry.created_at),
              },
              {
                key: 'actions',
                header: '',
                cell: (entry) => (
                  <button
                    onClick={() =>
                      setExpandedLogId((current) => (current === entry.id ? null : entry.id))
                    }
                    className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
                  >
                    {expandedLogId === entry.id ? 'Hide' : 'View'}
                  </button>
                ),
              },
            ]}
            data={entries}
            rowKey={(entry) => entry.id}
            rowTestId={(entry) => `log-row-${entry.id}`}
            emptyState={
              <EmptyState
                title="No matching log entries"
                description="Try broadening the current filters or moving to an earlier page."
              />
            }
          />
        )}

        {expandedEntry && (
          <div className="rounded-lg border border-border bg-card p-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <h2 className="text-sm font-semibold">Log Entry #{expandedEntry.id}</h2>
                <p className="mt-1 text-sm text-muted-foreground">
                  {formatDate(expandedEntry.created_at)}
                </p>
              </div>
              <span
                className={`rounded-full px-2 py-0.5 text-xs font-medium ${EVENT_STYLES[expandedEntry.event_type]}`}
              >
                {formatEventType(expandedEntry.event_type)}
              </span>
            </div>
            {expandedEntry.file_path && (
              <p className="mt-2 text-sm">
                <span className="font-medium text-muted-foreground">File:</span>{' '}
                <span className="font-mono text-xs">{expandedEntry.file_path}</span>
                {expandedEntry.tracked_file_id && (
                  <span className="ml-1 text-muted-foreground">
                    (#{expandedEntry.tracked_file_id})
                  </span>
                )}
              </p>
            )}
            <p className="mt-2 text-sm">{expandedEntry.message}</p>
            <StructuredDetails details={expandedEntry.details} />
          </div>
        )}

        {entries.length > 0 && (
          <div className="mt-auto flex items-center justify-between rounded-lg border border-border bg-card p-4 text-sm">
            <span className="text-muted-foreground">Page {page}</span>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setPage((current) => Math.max(1, current - 1))}
                disabled={page === 1}
                className="inline-flex items-center gap-1 rounded-md border border-border px-3 py-1.5 hover:bg-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                <ChevronLeft className="h-4 w-4" />
                Previous
              </button>
              <button
                onClick={() => setPage((current) => current + 1)}
                disabled={!hasNext}
                className="inline-flex items-center gap-1 rounded-md border border-border px-3 py-1.5 hover:bg-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                Next
                <ChevronRight className="h-4 w-4" />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
