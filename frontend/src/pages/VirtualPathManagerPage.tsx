import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { RefreshCw, Link2, Wand2 } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { trackingApi } from '@/api/tracking'
import { virtualPathsApi } from '@/api/virtual-paths'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { Pagination } from '@/components/shared/Pagination'
import { formatPath } from '@/lib/format'
import type { DrivePair } from '@/types/drive'
import type { TrackedFile, TrackedFileListResponse } from '@/types/file'
import type { TrackingItem } from '@/types/tracking'
import type { BulkAssignResult } from '@/types/virtual-path'

function isAbsolutePath(value: string) {
  return value.trim().startsWith('/')
}

function toTrackedFile(item: TrackingItem): TrackedFile {
  return {
    id: item.id,
    drive_pair_id: item.drive_pair_id,
    relative_path: item.path,
    checksum: null,
    file_size: null,
    virtual_path: item.virtual_path,
    is_mirrored: item.is_mirrored ?? false,
    tracked_direct: item.tracked_direct ?? false,
    tracked_via_folder: item.tracked_via_folder ?? false,
    last_verified: null,
    created_at: item.created_at,
    updated_at: item.updated_at,
  }
}

function VirtualPathModal({
  file,
  onClose,
  onSave,
}: {
  file: TrackedFile | null
  onClose: () => void
  onSave: (fileId: number, virtualPath: string) => Promise<void>
}) {
  const [value, setValue] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    setValue(file?.virtual_path ?? '')
    setSaving(false)
  }, [file])

  if (!file) return null

  const submit = async () => {
    if (!value.trim()) {
      return
    }

    if (!isAbsolutePath(value)) {
      toast.error('Publish path must be absolute')
      return
    }

    setSaving(true)
    try {
      await onSave(file.id, value.trim())
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">Publish File At Path</h2>
        <p className="mt-1 text-sm text-muted-foreground font-mono">{file.relative_path}</p>

        <div className="mt-4 space-y-3">
          <div>
            <label htmlFor="virtual-path-value" className="mb-1 block text-sm font-medium">
              Publish Path
            </label>
            <input
              id="virtual-path-value"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder="/docs/report.pdf"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
            <p className="mt-1 text-xs text-muted-foreground">
              BitProtector will create a symlink exactly at this absolute path.
            </p>
          </div>
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
            disabled={saving || !value.trim()}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  )
}

export function VirtualPathManagerPage() {
  const [response, setResponse] = useState<TrackedFileListResponse | null>(null)
  const [drives, setDrives] = useState<DrivePair[]>([])
  const [loading, setLoading] = useState(true)
  const [page, setPage] = useState(1)
  const [filters, setFilters] = useState({
    drive_id: '',
    q: '',
    publish_prefix: '',
    published: 'all',
  })
  const [editingFile, setEditingFile] = useState<TrackedFile | null>(null)
  const [removeTarget, setRemoveTarget] = useState<TrackedFile | null>(null)
  const [bulkInput, setBulkInput] = useState('')
  const [bulkResult, setBulkResult] = useState<BulkAssignResult | null>(null)
  const [bulkFromReal, setBulkFromReal] = useState({
    drive_pair_id: '',
    folder_path: '',
    publish_root: '',
  })
  const [refreshSummary, setRefreshSummary] = useState<{
    created: number
    removed: number
    errors: string[]
  } | null>(null)

  const loadFiles = useCallback(async (nextPage = page) => {
    setLoading(true)
    try {
      const tracking = await trackingApi.list({
        item_kind: 'file',
        source: 'all',
        page: nextPage,
        per_page: 50,
        drive_id: filters.drive_id ? Number(filters.drive_id) : undefined,
        q: filters.q.trim() || undefined,
        publish_prefix: filters.publish_prefix.trim() || undefined,
        published:
          filters.published === 'all'
            ? undefined
            : filters.published === 'published',
      })

      const files = tracking.items
        .filter((item) => item.kind === 'file')
        .map(toTrackedFile)

      setResponse({
        files,
        total: tracking.total,
        page: tracking.page,
        per_page: tracking.per_page,
      })
    } catch {
      toast.error('Failed to load tracked files')
    } finally {
      setLoading(false)
    }
  }, [filters.drive_id, filters.publish_prefix, filters.published, filters.q, page])

  useEffect(() => {
    let active = true

    const loadSupportData = async () => {
      try {
        const nextDrives = await drivesApi.list()
        if (active) {
          setDrives(nextDrives)
        }
      } catch {
        toast.error('Failed to load drive metadata')
      }
    }

    void loadSupportData()
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    void loadFiles(page)
  }, [loadFiles, page])

  const driveName = (driveId: number) =>
    drives.find((drive) => drive.id === driveId)?.name ?? `Drive #${driveId}`

  const saveVirtualPath = async (fileId: number, virtualPath: string) => {
    try {
      await virtualPathsApi.set(fileId, { virtual_path: virtualPath })
      setEditingFile(null)
      toast.success(`Publish path updated for file #${fileId}`)
      await loadFiles(page)
    } catch {
      toast.error(`Failed to update publish path for file #${fileId}`)
    }
  }

  const removeVirtualPath = async () => {
    if (!removeTarget) return

    try {
      await virtualPathsApi.remove(removeTarget.id)
      toast.success(`Removed publish path from file #${removeTarget.id}`)
      setRemoveTarget(null)
      await loadFiles(page)
    } catch {
      toast.error(`Failed to remove publish path from file #${removeTarget.id}`)
    }
  }

  const refreshSymlinks = async () => {
    try {
      const result = await virtualPathsApi.refresh()
      setRefreshSummary(result)
      toast.success(`Refreshed published symlinks (${result.created} created, ${result.removed} removed)`)
    } catch {
      toast.error('Failed to refresh published symlinks')
    }
  }

  const submitBulkAssignments = async () => {
    const lines = bulkInput
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)

    if (lines.length === 0) {
      toast.error('Provide at least one bulk assignment entry')
      return
    }

    try {
      const entries = lines.map((line) => {
        const [fileIdText, ...rest] = line.split('|')
        const fileId = Number(fileIdText?.trim())
        const virtualPath = rest.join('|').trim()

        if (!Number.isFinite(fileId) || fileId <= 0 || !virtualPath) {
          throw new Error(`Invalid bulk assignment line: ${line}`)
        }

        return {
          file_id: fileId,
          virtual_path: virtualPath,
        }
      })

      const result = await virtualPathsApi.bulk({ entries })
      setBulkResult(result)
      if (result.failed.length === 0) {
        toast.success(`Assigned ${result.succeeded.length} publish path(s)`)
      } else {
        toast.warning(
          `Assigned ${result.succeeded.length} publish path(s) with ${result.failed.length} failure(s)`
        )
      }
      await loadFiles(page)
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Bulk assignment failed'
      toast.error(message)
    }
  }

  const submitBulkFromReal = async () => {
    if (!bulkFromReal.drive_pair_id || !bulkFromReal.folder_path.trim() || !bulkFromReal.publish_root.trim()) {
      toast.error('Drive, folder path, and publish root are required')
      return
    }

    if (!isAbsolutePath(bulkFromReal.publish_root)) {
      toast.error('Publish root must be absolute')
      return
    }

    try {
      const result = await virtualPathsApi.bulkFromReal({
        drive_pair_id: Number(bulkFromReal.drive_pair_id),
        folder_path: bulkFromReal.folder_path.trim(),
        publish_root: bulkFromReal.publish_root.trim(),
      })
      setBulkResult(result)
      toast.success(
        `Generated ${result.succeeded.length} publish path(s) from real paths`
      )
      await loadFiles(page)
    } catch {
      toast.error('Failed to assign publish paths from real paths')
    }
  }

  const files = response?.files ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Publish Paths</h1>
          <p className="text-sm text-muted-foreground">
            Manage the exact filesystem paths where tracked files are published as symlinks.
          </p>
        </div>
        <button
          onClick={() => void refreshSymlinks()}
          className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <RefreshCw className="h-4 w-4" />
          Refresh Published Symlinks
        </button>
      </div>

      <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-4">
        <select
          value={filters.drive_id}
          onChange={(event) => {
            setPage(1)
            setFilters((current) => ({ ...current, drive_id: event.target.value }))
          }}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        >
          <option value="">All drives</option>
          {drives.map((drive) => (
            <option key={drive.id} value={String(drive.id)}>
              {drive.name}
            </option>
          ))}
        </select>
        <input
          value={filters.q}
          onChange={(event) => {
            setPage(1)
            setFilters((current) => ({ ...current, q: event.target.value }))
          }}
          placeholder="Search relative path"
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        />
        <input
          value={filters.publish_prefix}
          onChange={(event) => {
            setPage(1)
            setFilters((current) => ({ ...current, publish_prefix: event.target.value }))
          }}
          placeholder="Publish prefix (/docs)"
          className="rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
        />
        <select
          value={filters.published}
          onChange={(event) => {
            setPage(1)
            setFilters((current) => ({ ...current, published: event.target.value }))
          }}
          className="rounded-md border border-input bg-background px-3 py-2 text-sm"
        >
          <option value="all">Published + Unpublished</option>
          <option value="published">Published only</option>
          <option value="unpublished">Unpublished only</option>
        </select>
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.2fr_1fr]">
        <div className="space-y-4 rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2">
            <Link2 className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-semibold">Bulk Assign Publish Paths</h2>
          </div>
          <p className="text-sm text-muted-foreground">
            Enter one mapping per line in the format <span className="font-mono">file_id|/exact/publish/path</span>.
          </p>
          <textarea
            aria-label="Bulk Assignments"
            value={bulkInput}
            onChange={(event) => setBulkInput(event.target.value)}
            rows={6}
            placeholder={'12|/docs/report.pdf\n15|/media/photo.jpg'}
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
          />
          <button
            onClick={() => void submitBulkAssignments()}
            className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            Apply Publish Paths
          </button>
        </div>

        <div className="space-y-4 rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2">
            <Wand2 className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-semibold">Generate Publish Paths From Folder Files</h2>
          </div>
          <p className="text-sm text-muted-foreground">
            BitProtector will append each tracked file path under the selected folder to this absolute publish root.
          </p>
          <div className="space-y-3">
            <div>
              <label htmlFor="bulk-drive" className="mb-1 block text-sm font-medium">
                Drive Pair
              </label>
              <select
                id="bulk-drive"
                value={bulkFromReal.drive_pair_id}
                onChange={(event) =>
                  setBulkFromReal((current) => ({ ...current, drive_pair_id: event.target.value }))
                }
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="">Select a drive pair</option>
                {drives.map((drive) => (
                  <option key={drive.id} value={String(drive.id)}>
                    {drive.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label htmlFor="bulk-folder" className="mb-1 block text-sm font-medium">
                Folder Path
              </label>
              <input
                id="bulk-folder"
                value={bulkFromReal.folder_path}
                onChange={(event) =>
                  setBulkFromReal((current) => ({ ...current, folder_path: event.target.value }))
                }
                placeholder="documents"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label htmlFor="bulk-base" className="mb-1 block text-sm font-medium">
                Publish Root
              </label>
              <input
                id="bulk-base"
                value={bulkFromReal.publish_root}
                onChange={(event) =>
                  setBulkFromReal((current) => ({ ...current, publish_root: event.target.value }))
                }
                placeholder="/published/documents"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              />
            </div>
          </div>
          <button
            onClick={() => void submitBulkFromReal()}
            className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            Generate Publish Paths
          </button>
        </div>
      </div>

      {refreshSummary && (
        <div className="rounded-lg border border-blue-200 bg-blue-50 p-4 text-sm text-blue-900">
          Refreshed published symlinks: {refreshSummary.created} created, {refreshSummary.removed} removed
          {refreshSummary.errors.length > 0 &&
            `, ${refreshSummary.errors.length} error(s)`}
        </div>
      )}

      {bulkResult && bulkResult.failed.length > 0 && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 p-4 text-sm text-yellow-900">
          Bulk operation completed with {bulkResult.failed.length} failure(s):{' '}
          {bulkResult.failed.map((failure) => `#${failure.file_id}`).join(', ')}
        </div>
      )}

      {loading && !response ? (
        <div className="flex items-center justify-center py-16">
          <LoadingSpinner />
        </div>
      ) : (
        <div className="space-y-3">
          <DataTable
            tableTestId="virtual-paths-table"
            columns={[
              {
                key: 'id',
                header: 'File ID',
                cell: (file) => <span className="font-mono text-xs">{file.id}</span>,
              },
              {
                key: 'drive',
                header: 'Drive Pair',
                cell: (file) => driveName(file.drive_pair_id),
              },
              {
                key: 'relative_path',
                header: 'Relative Path',
                cell: (file) => <span className="font-mono text-xs">{file.relative_path}</span>,
              },
              {
                key: 'virtual_path',
                header: 'Publish Path',
                cell: (file) => (
                  <span className="font-mono text-xs">{formatPath(file.virtual_path)}</span>
                ),
              },
              {
                key: 'actions',
                header: '',
                cell: (file) => (
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => setEditingFile(file)}
                      className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
                    >
                      Set Path
                    </button>
                    <button
                      onClick={() => setRemoveTarget(file)}
                      disabled={!file.virtual_path}
                      className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      Remove
                    </button>
                  </div>
                ),
              },
            ]}
            data={files}
            rowKey={(file) => file.id}
            rowTestId={(file) => `virtual-path-row-${file.id}`}
            emptyState={
              <EmptyState
                title="No tracked files"
                description="Track files first, then assign exact publish paths from this page."
              />
            }
          />
          {response && (
            <Pagination
              page={response.page}
              perPage={response.per_page}
              total={response.total}
              onPageChange={setPage}
            />
          )}
        </div>
      )}

      <VirtualPathModal
        file={editingFile}
        onClose={() => setEditingFile(null)}
        onSave={saveVirtualPath}
      />

      <ConfirmDialog
        open={!!removeTarget}
        onOpenChange={(open) => {
          if (!open) setRemoveTarget(null)
        }}
        title="Remove publish path?"
        description={`Remove the publish path for "${removeTarget?.relative_path}"?`}
        confirmLabel="Remove"
        destructive
        onConfirm={removeVirtualPath}
      />
    </div>
  )
}
