import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { ShieldCheck, RotateCcw } from 'lucide-react'
import { integrityApi } from '@/api/integrity'
import { drivesApi } from '@/api/drives'
import { IntegrityStatusBadge } from '@/components/integrity/IntegrityStatus'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import type { DrivePair } from '@/types/drive'
import type { BatchIntegrityResult } from '@/types/integrity'

const ISSUE_STATUSES = new Set([
  'master_corrupted',
  'mirror_corrupted',
  'both_corrupted',
  'master_missing',
  'mirror_missing',
  'primary_drive_unavailable',
  'secondary_drive_unavailable',
])

function SummaryCard({
  label,
  value,
  tone = 'default',
}: {
  label: string
  value: number
  tone?: 'default' | 'warning' | 'danger' | 'info'
}) {
  return (
    <div
      className={[
        'rounded-lg border p-4',
        tone === 'default' && 'border-border bg-card',
        tone === 'warning' && 'border-yellow-200 bg-yellow-50',
        tone === 'danger' && 'border-red-200 bg-red-50',
        tone === 'info' && 'border-blue-200 bg-blue-50',
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <p className="text-2xl font-semibold">{value}</p>
      <p className="mt-1 text-sm text-muted-foreground">{label}</p>
    </div>
  )
}

export function IntegrityPage() {
  const [drives, setDrives] = useState<DrivePair[]>([])
  const [selectedDrive, setSelectedDrive] = useState('all')
  const [recover, setRecover] = useState(true)
  const [results, setResults] = useState<BatchIntegrityResult[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [drivesLoading, setDrivesLoading] = useState(true)
  const [checkingFileId, setCheckingFileId] = useState<number | null>(null)

  useEffect(() => {
    let active = true

    const loadDrives = async () => {
      setDrivesLoading(true)
      try {
        const nextDrives = await drivesApi.list()
        if (active) {
          setDrives(nextDrives)
        }
      } catch {
        toast.error('Failed to load drive pairs')
      } finally {
        if (active) {
          setDrivesLoading(false)
        }
      }
    }

    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  const runCheckAll = async () => {
    setLoading(true)
    try {
      const response = await integrityApi.checkAll(
        selectedDrive === 'all' ? undefined : Number(selectedDrive),
        recover
      )
      setResults(response.results)
      const issues = response.results.filter((result) => ISSUE_STATUSES.has(result.status)).length
      if (issues === 0) {
        toast.success('All tracked files passed integrity checks')
      } else {
        toast.warning(`Integrity check completed with ${issues} issue(s)`)
      }
    } catch {
      toast.error('Integrity check failed')
    } finally {
      setLoading(false)
    }
  }

  const recheckFile = async (fileId: number) => {
    setCheckingFileId(fileId)
    try {
      const result = await integrityApi.checkFile(fileId, recover)
      setResults((current) => {
        const nextResult: BatchIntegrityResult = {
          file_id: result.file_id,
          status: result.status,
          recovered: result.recovered,
        }

        if (!current) {
          return [nextResult]
        }

        const exists = current.some((entry) => entry.file_id === fileId)
        if (!exists) {
          return [nextResult, ...current]
        }

        return current.map((entry) => (entry.file_id === fileId ? nextResult : entry))
      })
      toast.success(`File #${fileId} rechecked`)
    } catch {
      toast.error(`Failed to recheck file #${fileId}`)
    } finally {
      setCheckingFileId(null)
    }
  }

  const issueCount = results?.filter((result) => ISSUE_STATUSES.has(result.status)).length ?? 0
  const recoveredCount = results?.filter((result) => result.recovered).length ?? 0
  const cleanCount = results?.filter((result) => result.status === 'ok').length ?? 0

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Integrity Checks</h1>
          <p className="text-sm text-muted-foreground">
            Run batch integrity checks and inspect per-file outcomes.
          </p>
        </div>
        <button
          onClick={() => void runCheckAll()}
          disabled={loading}
          className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {loading ? 'Running…' : 'Run Check'}
        </button>
      </div>

      <div className="grid gap-4 rounded-lg border border-border bg-card p-4 lg:grid-cols-[1fr_auto] lg:items-end">
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          <div>
            <label htmlFor="drive-filter" className="mb-1 block text-sm font-medium">
              Drive Pair
            </label>
            <select
              id="drive-filter"
              value={selectedDrive}
              onChange={(event) => setSelectedDrive(event.target.value)}
              disabled={drivesLoading}
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
          <label className="flex items-center gap-2 rounded-md border border-border bg-background px-3 py-2 text-sm">
            <input
              type="checkbox"
              checked={recover}
              onChange={(event) => setRecover(event.target.checked)}
            />
            Attempt recovery automatically
          </label>
        </div>
        {drivesLoading && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <LoadingSpinner size="sm" />
            Loading drives…
          </div>
        )}
      </div>

      {results && (
        <div className="grid gap-4 md:grid-cols-3">
          <SummaryCard label="Files Checked" value={results.length} />
          <SummaryCard
            label="Healthy Files"
            value={cleanCount}
            tone={cleanCount === results.length ? 'default' : 'info'}
          />
          <SummaryCard
            label="Issues Found"
            value={issueCount}
            tone={issueCount > 0 ? 'danger' : 'default'}
          />
          <SummaryCard
            label="Recovered"
            value={recoveredCount}
            tone={recoveredCount > 0 ? 'warning' : 'default'}
          />
        </div>
      )}

      {results && issueCount > 0 && (
        <div className="rounded-lg border border-red-200 bg-red-50 p-4 text-sm text-red-800">
          {issueCount} file(s) need attention. Re-run individual rows if you want to confirm the
          latest state after recovery or follow-up work.
        </div>
      )}

      {results === null && !loading ? (
        <EmptyState
          icon={<ShieldCheck className="h-10 w-10 text-muted-foreground" />}
          title="No integrity results yet"
          description="Run a batch check to populate the latest per-file status table."
        />
      ) : loading && results === null ? (
        <div className="flex items-center justify-center py-16">
          <LoadingSpinner />
        </div>
      ) : (
        <DataTable
          columns={[
            {
              key: 'file_id',
              header: 'File ID',
              cell: (result) => <span className="font-mono text-xs">{result.file_id}</span>,
            },
            {
              key: 'status',
              header: 'Status',
              cell: (result) => <IntegrityStatusBadge status={result.status} />,
            },
            {
              key: 'recovered',
              header: 'Recovered',
              cell: (result) => (result.recovered ? 'Yes' : 'No'),
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
          data={results ?? []}
          rowKey={(result) => result.file_id}
          emptyState={
            <EmptyState
              title="No tracked files matched this integrity run"
              description="Adjust the drive filter or run the check again."
            />
          }
        />
      )}
    </div>
  )
}
