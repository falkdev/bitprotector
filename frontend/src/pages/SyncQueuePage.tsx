import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Play } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { syncApi } from '@/api/sync'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PageIntro } from '@/components/shared/PageIntro'
import { useSyncStore } from '@/stores/sync-store'
import { formatDate } from '@/lib/format'
import type { ResolveQueueItemRequest, SyncQueueItem, SyncResolution, SyncStatus } from '@/types/sync'

type QueueFilter = SyncStatus | 'all'

const FILTERS: QueueFilter[] = ['all', 'pending', 'in_progress', 'completed', 'failed']

const STATUS_STYLES: Record<SyncStatus, string> = {
  pending: 'bg-yellow-100 text-yellow-800',
  in_progress: 'bg-blue-100 text-blue-800',
  completed: 'bg-green-100 text-green-800',
  failed: 'bg-red-100 text-red-800',
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
    setResolution('keep_master')
    setNewFilePath('')
    setSubmitting(false)
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
  const items = useSyncStore((state) => state.items)
  const loading = useSyncStore((state) => state.loading)
  const filter = useSyncStore((state) => state.filter)
  const fetch = useSyncStore((state) => state.fetch)
  const setFilter = useSyncStore((state) => state.setFilter)
  const refreshItem = useSyncStore((state) => state.refreshItem)
  const [resolveTarget, setResolveTarget] = useState<SyncQueueItem | null>(null)
  const [processingQueue, setProcessingQueue] = useState(false)
  const [clearingCompleted, setClearingCompleted] = useState(false)
  const [hasDrivePairs, setHasDrivePairs] = useState<boolean | null>(null)

  useEffect(() => {
    void fetch()
    const timer = window.setInterval(() => {
      void fetch()
    }, 5000)

    return () => {
      window.clearInterval(timer)
    }
  }, [fetch])

  useEffect(() => {
    let active = true

    const loadDrives = async () => {
      try {
        const drives = await drivesApi.list()
        if (active) {
          setHasDrivePairs(drives.length > 0)
        }
      } catch {
        if (active) {
          setHasDrivePairs(null)
        }
      }
    }

    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  const visibleItems =
    filter === 'all' ? items : items.filter((item) => item.status === filter)
  const completedCount = items.filter((item) => item.status === 'completed').length
  const disableProcessQueue = hasDrivePairs === false

  const processQueue = async () => {
    if (disableProcessQueue) {
      return
    }

    setProcessingQueue(true)
    try {
      const result = await syncApi.processQueue()
      toast.success(`Processed ${result.processed} queue item(s)`)
      await fetch()
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
      await fetch()
    } catch {
      toast.error(`Failed to resolve queue item #${id}`)
    }
  }

  const clearCompleted = async () => {
    setClearingCompleted(true)
    try {
      const result = await syncApi.clearCompletedQueue()
      toast.success(`Cleared ${result.deleted} completed queue item(s)`)
      await fetch()
    } catch {
      toast.error('Failed to clear completed queue items')
    } finally {
      setClearingCompleted(false)
    }
  }

  return (
    <div className="space-y-6">
      <PageIntro
        title="Sync Queue"
        subtitle="Review pending sync actions, process the queue, and resolve conflicts."
        actions={
          <button
            onClick={() => void processQueue()}
            disabled={processingQueue || disableProcessQueue}
            className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            <Play className="h-4 w-4" />
            {processingQueue ? 'Processing…' : 'Process Queue'}
          </button>
        }
      />
      {disableProcessQueue ? (
        <p className="text-xs text-muted-foreground" data-testid="sync-queue-no-drives-hint">
          Add a drive pair first to process the sync queue.
        </p>
      ) : null}

      <div className="flex items-center gap-3 rounded-lg border border-border bg-card p-4">
        <label htmlFor="queue-filter" className="text-sm font-medium">
          Filter
        </label>
        <select
          id="queue-filter"
          value={filter}
          onChange={(event) => setFilter(event.target.value as QueueFilter)}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        >
          {FILTERS.map((option) => (
            <option key={option} value={option}>
              {option === 'all' ? 'All statuses' : option.replace('_', ' ')}
            </option>
          ))}
        </select>
        <span className="text-sm text-muted-foreground">{visibleItems.length} item(s)</span>
        <button
          onClick={() => void clearCompleted()}
          disabled={clearingCompleted || completedCount === 0}
          className="ml-auto rounded-md border border-border px-3 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
        >
          {clearingCompleted ? 'Clearing…' : 'Clear Completed'}
        </button>
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
              cell: (item) => item.action,
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
              key: 'error_message',
              header: 'Details',
              cell: (item) => item.error_message ?? '—',
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
          data={visibleItems}
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

      <ResolveDialog
        item={resolveTarget}
        onClose={() => setResolveTarget(null)}
        onResolve={resolveItem}
      />
    </div>
  )
}
