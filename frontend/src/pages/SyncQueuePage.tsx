import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { LoaderCircle, Pause, Play } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { syncApi } from '@/api/sync'
import { openSyncStream } from '@/api/sync-stream'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PageIntro } from '@/components/shared/PageIntro'
import { useSyncStore } from '@/stores/sync-store'
import { formatDate } from '@/lib/format'
import type {
  ResolveQueueItemRequest,
  SyncAction,
  SyncQueueItem,
  SyncResolution,
  SyncStatus,
} from '@/types/sync'

type QueueFilter = SyncStatus | 'all'

const FILTERS: QueueFilter[] = ['all', 'pending', 'in_progress', 'completed', 'failed']

const FILTER_LABELS: Record<QueueFilter, string> = {
  all: 'All',
  pending: 'Pending',
  in_progress: 'In Progress',
  completed: 'Completed',
  failed: 'Failed',
}

const STATUS_STYLES: Record<SyncStatus, string> = {
  pending: 'bg-yellow-100 text-yellow-800',
  in_progress: 'bg-blue-100 text-blue-800',
  completed: 'bg-green-100 text-green-800',
  failed: 'bg-red-100 text-red-800',
}

const ACTION_LABELS: Record<SyncAction, string> = {
  mirror: 'Mirror',
  restore_master: 'Restore Master',
  restore_mirror: 'Restore Mirror',
  verify: 'Verify',
  adopt_mirror: 'Adopt Mirror',
  user_action_required: 'Action Required',
}

const ACTION_DESCRIPTIONS: Record<SyncAction, string> = {
  mirror: 'Copy master files to the mirror drive',
  restore_master: 'Recover the master drive using the mirror',
  restore_mirror: 'Repair the mirror using the master drive',
  verify: 'Verify file checksums for integrity',
  adopt_mirror: 'Promote the mirror as the new authoritative source',
  user_action_required: 'Manual conflict resolution needed',
}

function ResolveDialog({
  item,
  onClose,
  onResolve,
}: {
  item: SyncQueueItem | null
  onClose: () => void
  onResolve: (id: number, data: ResolveQueueItemRequest) => Promise<void>
}) {
  const [resolution, setResolution] = useState<SyncResolution>('keep_master')
  const [newFilePath, setNewFilePath] = useState('')
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setResolution('keep_master')
      setNewFilePath('')
      setSubmitting(false)
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [item])

  if (!item) return null

  const submit = async () => {
    setSubmitting(true)
    try {
      await onResolve(item.id, {
        resolution,
        new_file_path: resolution === 'provide_new' ? newFilePath : undefined,
      })
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <ModalLayer>
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">Resolve Queue Item</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Queue item #{item.id} for tracked file #{item.tracked_file_id}
        </p>

        <div className="mt-4 space-y-3">
          <label className="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
            <input
              type="radio"
              name="resolution"
              value="keep_master"
              checked={resolution === 'keep_master'}
              onChange={() => setResolution('keep_master')}
            />
            Keep the primary copy
          </label>
          <label className="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
            <input
              type="radio"
              name="resolution"
              value="keep_mirror"
              checked={resolution === 'keep_mirror'}
              onChange={() => setResolution('keep_mirror')}
            />
            Keep the mirror copy
          </label>
          <label className="flex items-start gap-2 rounded-md border border-border px-3 py-2 text-sm">
            <input
              type="radio"
              name="resolution"
              value="provide_new"
              checked={resolution === 'provide_new'}
              onChange={() => setResolution('provide_new')}
              className="mt-0.5"
            />
            <span className="flex-1">
              Provide a replacement file path
              {resolution === 'provide_new' && (
                <input
                  value={newFilePath}
                  onChange={(event) => setNewFilePath(event.target.value)}
                  placeholder="/path/to/replacement/file"
                  className="mt-2 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                />
              )}
            </span>
          </label>
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
            disabled={submitting || (resolution === 'provide_new' && !newFilePath.trim())}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {submitting ? 'Resolving…' : 'Resolve'}
          </button>
        </div>
      </div>
    </ModalLayer>
  )
}

export function SyncQueuePage() {
  const summary = useSyncStore((state) => state.summary)
  const setSummary = useSyncStore((state) => state.setSummary)
  const items = useSyncStore((state) => state.items)
  const filteredTotal = useSyncStore((state) => state.filteredTotal)
  const page = useSyncStore((state) => state.page)
  const perPage = useSyncStore((state) => state.perPage)
  const loading = useSyncStore((state) => state.loading)
  const filter = useSyncStore((state) => state.filter)
  const fetchItems = useSyncStore((state) => state.fetchItems)
  const setFilter = useSyncStore((state) => state.setFilter)
  const setPage = useSyncStore((state) => state.setPage)
  const refreshItem = useSyncStore((state) => state.refreshItem)
  const [resolveTarget, setResolveTarget] = useState<SyncQueueItem | null>(null)
  const [processingQueue, setProcessingQueue] = useState(false)
  const [clearingCompleted, setClearingCompleted] = useState(false)
  const [togglingPause, setTogglingPause] = useState(false)
  const [hasDrivePairs, setHasDrivePairs] = useState<boolean | null>(null)

  // Track the last revision for which we issued an item fetch, so that each
  // SSE summary event triggers at most one REST request.
  const lastFetchedRevision = useRef(-1)

  // ── Drive-pair check (one-shot) ──────────────────────────────────────────
  useEffect(() => {
    let active = true
    const loadDrives = async () => {
      try {
        const drives = await drivesApi.list()
        if (active) setHasDrivePairs(drives.length > 0)
      } catch {
        if (active) setHasDrivePairs(null)
      }
    }
    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  // ── Initial item fetch ───────────────────────────────────────────────────
  useEffect(() => {
    void fetchItems()
  }, [fetchItems])

  // ── SSE stream ───────────────────────────────────────────────────────────
  useEffect(() => {
    const stream = openSyncStream((incoming) => {
      setSummary(incoming)
      // Refetch item list whenever the server signals a new revision.
      if (incoming.revision !== lastFetchedRevision.current) {
        lastFetchedRevision.current = incoming.revision
        void fetchItems()
      }
    })

    return () => stream.abort()
  }, [setSummary, fetchItems])

  // ── Derived values from SSE summary ─────────────────────────────────────
  const queuePaused = summary?.queue_paused ?? false
  const processingActive = summary?.processing_active ?? false
  const scanning = summary?.scanning ?? false
  const scanTotalFiles = summary?.scan_total_files ?? 0
  const scanScannedFiles = summary?.scan_scanned_files ?? 0
  const scanActiveFolders = summary?.scan_active_folders ?? 0
  const activeItems = summary?.active_items ?? 0

  const statusCounts: Record<QueueFilter, number> = {
    all: summary?.total ?? 0,
    pending: summary?.pending_items ?? 0,
    in_progress: summary?.in_progress_items ?? 0,
    completed: summary?.completed_items ?? 0,
    failed: summary?.failed_items ?? 0,
  }

  const showProcessingIndicator = processingActive || processingQueue
  const controlsDisabledByScan = scanning

  const noDrivePairs = hasDrivePairs === false
  const disableProcessQueue =
    noDrivePairs || processingActive || processingQueue || controlsDisabledByScan
  const disablePauseQueue =
    controlsDisabledByScan || togglingPause || (!queuePaused && activeItems === 0)
  const totalPages = Math.max(1, Math.ceil(filteredTotal / perPage))
  const hasPreviousPage = page > 1
  const hasNextPage = page * perPage < filteredTotal

  // ── Actions ──────────────────────────────────────────────────────────────
  const processQueue = async () => {
    if (noDrivePairs || controlsDisabledByScan) return
    setProcessingQueue(true)
    try {
      await syncApi.processQueue()
      toast.success('Processing started')
    } catch {
      toast.error('Failed to start processing')
    } finally {
      setProcessingQueue(false)
    }
  }

  const resolveItem = async (id: number, data: ResolveQueueItemRequest) => {
    try {
      const updated = await syncApi.resolveQueueItem(id, data)
      refreshItem(updated)
      setResolveTarget(null)
      toast.success(`Queue item #${id} resolved`)
      await fetchItems()
    } catch {
      toast.error(`Failed to resolve queue item #${id}`)
    }
  }

  const clearCompleted = async () => {
    setClearingCompleted(true)
    try {
      const result = await syncApi.clearCompletedQueue()
      toast.success(`Cleared ${result.deleted} completed queue item(s)`)
      await fetchItems()
    } catch {
      toast.error('Failed to clear completed queue items')
    } finally {
      setClearingCompleted(false)
    }
  }

  const togglePause = async () => {
    if (controlsDisabledByScan) return
    setTogglingPause(true)
    try {
      if (queuePaused) {
        await syncApi.resumeQueue()
        toast.success('Sync queue processing resumed')
      } else {
        await syncApi.pauseQueue()
        toast.success('Sync queue processing paused')
      }
    } catch {
      toast.error('Failed to toggle queue pause state')
    } finally {
      setTogglingPause(false)
    }
  }

  return (
    <div className="space-y-6">
      <PageIntro
        title="Sync Queue"
        subtitle="Review pending sync actions, process the queue, and resolve conflicts."
        actions={
          <div className="flex items-center gap-2">
            <button
              onClick={() => void togglePause()}
              disabled={disablePauseQueue}
              data-testid="toggle-pause-button"
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
            >
              {queuePaused ? (
                <>
                  <Play className="h-4 w-4" />
                  {togglingPause ? 'Resuming…' : 'Resume Queue'}
                </>
              ) : (
                <>
                  <Pause className="h-4 w-4" />
                  {togglingPause ? 'Pausing…' : 'Pause Queue'}
                </>
              )}
            </button>
            <button
              onClick={() => void processQueue()}
              disabled={disableProcessQueue}
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              <Play className="h-4 w-4" />
              {showProcessingIndicator ? 'Processing...' : 'Process Queue'}
            </button>
          </div>
        }
      />
      {queuePaused && (
        <div
          data-testid="queue-paused-banner"
          className="flex items-center gap-2 rounded-lg border border-yellow-300 bg-yellow-50 px-4 py-3 text-sm text-yellow-800"
        >
          <Pause className="h-4 w-4 shrink-0" />
          <span>
            <strong>Queue paused.</strong> Automatic processing is suspended. Existing items will
            not be processed until you resume the queue.
          </span>
        </div>
      )}

      {noDrivePairs ? (
        <p className="text-xs text-muted-foreground" data-testid="sync-queue-no-drives-hint">
          Add a drive pair first to process the sync queue.
        </p>
      ) : null}

      <div className="space-y-3 rounded-lg border border-border bg-card p-4">
        <div role="tablist" aria-label="Queue filter" className="flex flex-wrap gap-2">
          {FILTERS.map((option) => (
            <button
              key={option}
              type="button"
              role="tab"
              aria-selected={filter === option}
              onClick={() => void setFilter(option)}
              disabled={controlsDisabledByScan}
              className={`rounded-md border px-3 py-1.5 text-sm disabled:cursor-not-allowed disabled:opacity-60 ${
                filter === option
                  ? 'border-primary bg-primary/10 text-primary'
                  : 'border-border hover:bg-accent'
              }`}
            >
              {`${FILTER_LABELS[option]} ${statusCounts[option]}`}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-3 text-sm text-muted-foreground">
          {scanning ? (
            <span className="inline-flex items-center gap-1.5">
              <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
              Updating… {scanScannedFiles} / {scanTotalFiles} files across {scanActiveFolders}{' '}
              folder(s)
            </span>
          ) : null}
          {showProcessingIndicator && !queuePaused ? (
            <span className="inline-flex items-center gap-1.5" data-testid="processing-indicator">
              <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
              Processing queue…
            </span>
          ) : null}
          {!scanning ? (
            <span>
              Showing {items.length} of {filteredTotal} (filter: {FILTER_LABELS[filter]})
            </span>
          ) : null}
          <button
            onClick={() => void clearCompleted()}
            disabled={clearingCompleted || statusCounts.completed === 0}
            className="ml-auto rounded-md border border-border px-3 py-2 text-sm text-foreground hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
          >
            {clearingCompleted ? 'Clearing…' : 'Clear Completed'}
          </button>
        </div>
      </div>

      {loading && items.length === 0 ? (
        <div className="flex items-center justify-center py-16">
          <LoadingSpinner />
        </div>
      ) : (
        <DataTable
          tableTestId="sync-queue-table"
          columns={[
            {
              key: 'id',
              header: 'Queue ID',
              cell: (item) => <span className="font-mono text-xs">{item.id}</span>,
            },
            {
              key: 'tracked_file_id',
              header: 'File ID',
              cell: (item) => <span className="font-mono text-xs">{item.tracked_file_id}</span>,
            },
            {
              key: 'action',
              header: 'Action',
              cell: (item) => (
                <span
                  className="rounded bg-muted px-2 py-0.5 text-xs font-medium"
                  title={ACTION_DESCRIPTIONS[item.action]}
                >
                  {ACTION_LABELS[item.action]}
                </span>
              ),
            },
            {
              key: 'status',
              header: 'Status',
              cell: (item) => (
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-medium ${STATUS_STYLES[item.status]}`}
                >
                  {item.status.replace('_', ' ')}
                </span>
              ),
            },
            {
              key: 'created_at',
              header: 'Created',
              cell: (item) => formatDate(item.created_at),
            },
            {
              key: 'relative_path',
              header: 'File',
              cell: (item) => (
                <div>
                  <div className="font-mono text-xs text-foreground">
                    {item.relative_path || '—'}
                  </div>
                  {item.error_message ? (
                    <div className="mt-1 text-xs text-red-600">{item.error_message}</div>
                  ) : null}
                </div>
              ),
            },
            {
              key: 'actions',
              header: '',
              cell: (item) =>
                item.action === 'user_action_required' && item.status === 'pending' ? (
                  <button
                    onClick={() => setResolveTarget(item)}
                    className="rounded-md border border-border px-3 py-1.5 text-xs font-medium hover:bg-accent"
                  >
                    Resolve
                  </button>
                ) : (
                  '—'
                ),
            },
          ]}
          data={items}
          rowKey={(item) => item.id}
          rowTestId={(item) => `sync-queue-row-${item.id}`}
          emptyState={
            <EmptyState
              title="No queue items"
              description="The sync queue is empty for the current filter."
            />
          }
        />
      )}

      {filteredTotal > 0 ? (
        <div className="flex items-center justify-end gap-3 text-sm">
          {scanning && (
            <span className="inline-flex items-center gap-1.5 text-muted-foreground">
              <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
              Page count updating…
            </span>
          )}
          <button
            type="button"
            onClick={() => void setPage(page - 1)}
            disabled={!hasPreviousPage}
            className="rounded-md border border-border px-3 py-2 hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
          >
            Previous
          </button>
          <span className="text-muted-foreground">
            Page {page} of {totalPages}
          </span>
          <button
            type="button"
            onClick={() => void setPage(page + 1)}
            disabled={!hasNextPage || controlsDisabledByScan}
            className="rounded-md border border-border px-3 py-2 hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
          >
            Next
          </button>
        </div>
      ) : null}

      <ResolveDialog
        item={resolveTarget}
        onClose={() => setResolveTarget(null)}
        onResolve={resolveItem}
      />
    </div>
  )
}
