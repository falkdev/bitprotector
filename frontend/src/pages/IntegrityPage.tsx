import { useCallback, useEffect, useMemo, useState } from 'react'
import { Clock3, Play, RotateCcw, Square } from 'lucide-react'
import { toast } from 'sonner'
import { integrityApi } from '@/api/integrity'
import { drivesApi } from '@/api/drives'
import { IntegrityStatusBadge } from '@/components/integrity/IntegrityStatus'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PageIntro } from '@/components/shared/PageIntro'
import { formatDate } from '@/lib/format'
import type { DrivePair } from '@/types/drive'
import type { IntegrityRun, IntegrityRunResult } from '@/types/integrity'

const PAGE_SIZE = 50

function getLastIntegrityCheckLabel(run: IntegrityRun | null): string {
  if (!run) {
    return 'No integrity checks yet'
  }

  if (run.ended_at) {
    return formatDate(run.ended_at)
  }

  if (run.status === 'running' || run.status === 'stopping') {
    return `In progress (started ${formatDate(run.started_at)})`
  }

  return formatDate(run.started_at)
}

function SummaryCard({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-2xl font-semibold">{value}</p>
      <p className="mt-1 text-sm text-muted-foreground">{label}</p>
    </div>
  )
}

function StartRunDialog({
  open,
  drives,
  loading,
  onClose,
  onStart,
}: {
  open: boolean
  drives: DrivePair[]
  loading: boolean
  onClose: () => void
  onStart: (driveId: number | undefined, recover: boolean) => Promise<void>
}) {
  const [selectedDrive, setSelectedDrive] = useState('all')
  const [recover, setRecover] = useState(true)

  useEffect(() => {
    if (!open) return
    setSelectedDrive('all')
    setRecover(true)
  }, [open])

  if (!open) return null

  return (
    <ModalLayer>
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">Start Integrity Run</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Select which drive pair to check. Results will stream in as files are processed.
        </p>

        <div className="mt-4 space-y-4">
          <div>
            <label htmlFor="integrity-run-drive" className="mb-1 block text-sm font-medium">
              Drive Pair
            </label>
            <select
              id="integrity-run-drive"
              value={selectedDrive}
              onChange={(event) => setSelectedDrive(event.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="all">All drive pairs</option>
              {drives.map((drive) => (
                <option key={drive.id} value={String(drive.id)}>
                  {drive.name}
                </option>
              ))}
            </select>
          </div>

          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" checked={recover} onChange={(event) => setRecover(event.target.checked)} />
            Attempt automatic recovery
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
            disabled={loading}
            onClick={() => void onStart(selectedDrive === 'all' ? undefined : Number(selectedDrive), recover)}
            className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            <Play className="h-4 w-4" />
            {loading ? 'Starting…' : 'Start'}
          </button>
        </div>
      </div>
    </ModalLayer>
  )
}

export function IntegrityPage() {
  const [drives, setDrives] = useState<DrivePair[]>([])
  const [run, setRun] = useState<IntegrityRun | null>(null)
  const [results, setResults] = useState<IntegrityRunResult[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [bootLoading, setBootLoading] = useState(true)
  const [tableLoading, setTableLoading] = useState(false)
  const [loadingMore, setLoadingMore] = useState(false)
  const [showStartDialog, setShowStartDialog] = useState(false)
  const [starting, setStarting] = useState(false)
  const [stopping, setStopping] = useState(false)
  const [checkingFileId, setCheckingFileId] = useState<number | null>(null)

  const hasMore = results.length < total
  const hasDrivePairs = drives.length > 0
  const isRunning = run?.status === 'running' || run?.status === 'stopping'
  const disableStartRun = !isRunning && !hasDrivePairs
  const lastIntegrityCheckLabel = getLastIntegrityCheckLabel(run)

  const loadRunResults = useCallback(
    async (runId: number, nextPage = 1, append = false) => {
      if (append) {
        setLoadingMore(true)
      } else {
        setTableLoading(true)
      }
      try {
        const response = await integrityApi.runResults(runId, {
          issues_only: true,
          page: nextPage,
          per_page: PAGE_SIZE,
        })
        setRun(response.run)
        setTotal(response.total)
        setPage(response.page)
        setResults((current) => (append ? [...current, ...response.results] : response.results))
      } catch {
        toast.error('Failed to load integrity results')
      } finally {
        setTableLoading(false)
        setLoadingMore(false)
      }
    },
    []
  )

  useEffect(() => {
    let active = true
    const bootstrap = async () => {
      setBootLoading(true)
      try {
        const [nextDrives, activeRunResponse, latestResponse] = await Promise.all([
          drivesApi.list(),
          integrityApi.activeRun(),
          integrityApi.latestResults({
            issues_only: true,
            page: 1,
            per_page: PAGE_SIZE,
          }),
        ])
        if (!active) return
        setDrives(nextDrives)

        if (activeRunResponse.run) {
          setRun(activeRunResponse.run)
          const runResponse = await integrityApi.runResults(activeRunResponse.run.id, {
            issues_only: true,
            page: 1,
            per_page: PAGE_SIZE,
          })
          if (!active) return
          setRun(runResponse.run)
          setResults(runResponse.results)
          setTotal(runResponse.total)
          setPage(runResponse.page)
          return
        }

        setRun(latestResponse.run)
        setResults(latestResponse.results)
        setTotal(latestResponse.total)
        setPage(latestResponse.page)
      } catch {
        toast.error('Failed to load integrity page data')
      } finally {
        if (active) {
          setBootLoading(false)
        }
      }
    }

    void bootstrap()
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    if (!run || !isRunning) return

    const timer = window.setInterval(() => {
      void (async () => {
        try {
          const activeResponse = await integrityApi.activeRun()
          if (activeResponse.run?.id === run.id) {
            setRun(activeResponse.run)
          }
          await loadRunResults(run.id, 1, false)
        } catch {
          // no-op; the table loader already reports failures
        }
      })()
    }, 2000)

    return () => {
      window.clearInterval(timer)
    }
  }, [isRunning, loadRunResults, run])

  const startRun = async (driveId: number | undefined, recover: boolean) => {
    setStarting(true)
    try {
      const nextRun = await integrityApi.startRun(driveId, recover)
      setRun(nextRun)
      setResults([])
      setTotal(0)
      setPage(1)
      setShowStartDialog(false)
      toast.success('Integrity run started')
      await loadRunResults(nextRun.id, 1, false)
    } catch {
      toast.error('Failed to start integrity run')
    } finally {
      setStarting(false)
    }
  }

  const stopRun = async () => {
    if (!run) return
    setStopping(true)
    try {
      const nextRun = await integrityApi.stopRun(run.id)
      setRun(nextRun)
      toast.success(`Stop requested for run #${run.id}`)
    } catch {
      toast.error(`Failed to stop run #${run.id}`)
    } finally {
      setStopping(false)
    }
  }

  const recheckFile = async (fileId: number) => {
    setCheckingFileId(fileId)
    try {
      const result = await integrityApi.checkFile(fileId, true)
      if (result.status === 'ok' || result.recovered) {
        setResults((current) => current.filter((entry) => entry.file_id !== fileId))
        setTotal((current) => Math.max(0, current - 1))
      } else {
        setResults((current) =>
          current.map((entry) =>
            entry.file_id === fileId
              ? {
                  ...entry,
                  status: result.status,
                  recovered: result.recovered,
                  needs_attention: true,
                }
              : entry
          )
        )
      }
      toast.success(`File #${fileId} rechecked`)
    } catch {
      toast.error(`Failed to recheck file #${fileId}`)
    } finally {
      setCheckingFileId(null)
    }
  }

  const attentionRows = useMemo(
    () => results.filter((result) => result.needs_attention),
    [results]
  )

  if (bootLoading) {
    return (
      <div className="space-y-4">
        <PageIntro
          title="Integrity"
          subtitle="Run integrity checks, monitor progress, and review files that need attention."
        />
        <div className="flex items-center justify-center gap-3 py-20 text-sm text-muted-foreground">
          <LoadingSpinner />
          <span>Loading latest integrity results…</span>
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <PageIntro
        title="Integrity"
        subtitle="Run integrity checks, monitor progress, and review files that need attention."
        actions={
          <button
            type="button"
            onClick={() => {
              if (isRunning) {
                void stopRun()
                return
              }
              if (disableStartRun) {
                return
              }
              setShowStartDialog(true)
            }}
            disabled={starting || stopping || disableStartRun}
            className={`inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md px-4 py-2 text-sm font-medium text-white disabled:cursor-not-allowed disabled:opacity-60 ${
              isRunning ? 'bg-red-600 hover:bg-red-700' : 'bg-primary hover:bg-primary/90'
            }`}
          >
            {isRunning ? <Square className="h-4 w-4" /> : <Play className="h-4 w-4" />}
            {isRunning ? (stopping ? 'Stopping…' : 'Stop') : starting ? 'Starting…' : 'Run Check'}
          </button>
        }
      />
      {disableStartRun ? (
        <p className="text-xs text-muted-foreground" data-testid="integrity-no-drives-hint">
          Add a drive pair first to run integrity checks.
        </p>
      ) : null}
      <div
        className="inline-flex w-fit items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2"
        data-testid="integrity-last-check"
      >
        <Clock3 className="h-4 w-4 text-muted-foreground" />
        <p className="text-sm">
          <span className="font-medium">Last integrity check:</span>{' '}
          <span className="text-muted-foreground">{lastIntegrityCheckLabel}</span>
        </p>
      </div>

      {run && (
        <div className="grid gap-3 md:grid-cols-4">
          <SummaryCard label="Files Processed" value={run.processed_files} />
          <SummaryCard label="Total Files" value={run.total_files} />
          <SummaryCard label="Need Attention" value={run.attention_files} />
          <SummaryCard label="Recovered" value={run.recovered_files} />
        </div>
      )}

      {run && isRunning && (
        <div className="rounded-lg border border-blue-200 bg-blue-50 p-3 text-sm text-blue-900">
          Integrity check running ({run.processed_files}/{run.total_files}). Results appear as files
          are processed.
        </div>
      )}

      {run && run.error_message && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-800">
          Run failed: {run.error_message}
        </div>
      )}

      {tableLoading && (
        <div className="flex items-center justify-center gap-3 py-8 text-sm text-muted-foreground">
          <LoadingSpinner />
          <span>Loading run results…</span>
        </div>
      )}

      {!tableLoading && !run ? (
        <EmptyState
          title="No integrity runs yet"
          description="Run an integrity check to populate results."
        />
      ) : null}

      {!tableLoading && run && attentionRows.length === 0 && isRunning && (
        <div className="flex items-center justify-center gap-3 py-8 text-sm text-muted-foreground">
          <LoadingSpinner />
          <span>Checking files… no issues found yet.</span>
        </div>
      )}

      {!tableLoading && run && attentionRows.length === 0 && !isRunning ? (
        <EmptyState title="No files need attention" description="The latest run did not find actionable integrity issues." />
      ) : null}

      {!tableLoading && attentionRows.length > 0 && (
        <div className="space-y-3">
          <DataTable
            tableTestId="integrity-results-table"
            columns={[
              {
                key: 'file_id',
                header: 'File ID',
                cell: (result) => <span className="font-mono text-xs">{result.file_id}</span>,
              },
              {
                key: 'relative_path',
                header: 'Path',
                cell: (result) => <span className="font-mono text-xs">{result.relative_path}</span>,
              },
              {
                key: 'status',
                header: 'Status',
                cell: (result) => <IntegrityStatusBadge status={result.status} />,
              },
              {
                key: 'actions',
                header: '',
                cell: (result) => (
                  <button
                    onClick={() => void recheckFile(result.file_id)}
                    disabled={checkingFileId === result.file_id}
                    className="inline-flex items-center gap-1 rounded-md border border-border px-3 py-1.5 text-xs font-medium hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <RotateCcw className="h-3.5 w-3.5" />
                    {checkingFileId === result.file_id ? 'Checking…' : 'Recheck'}
                  </button>
                ),
              },
            ]}
            data={attentionRows}
            rowKey={(result) => result.id}
            rowTestId={(result) => `integrity-row-${result.file_id}`}
            emptyState={<EmptyState title="No files need attention" description="No issue rows on this page." />}
          />

          {hasMore && (
            <div className="flex justify-center">
              <button
                type="button"
                disabled={loadingMore || !run}
                onClick={() => {
                  if (!run) return
                  void loadRunResults(run.id, page + 1, true)
                }}
                className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
              >
                {loadingMore ? 'Loading…' : 'Load More'}
              </button>
            </div>
          )}
        </div>
      )}

      <StartRunDialog
        open={showStartDialog}
        drives={drives}
        loading={starting}
        onClose={() => setShowStartDialog(false)}
        onStart={startRun}
      />
    </div>
  )
}
