import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { RefreshCw, Link2, Wand2 } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { filesApi } from '@/api/files'
import { foldersApi } from '@/api/folders'
import { virtualPathsApi } from '@/api/virtual-paths'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { Pagination } from '@/components/shared/Pagination'
import { formatPath } from '@/lib/format'
import type { DrivePair } from '@/types/drive'
import type { TrackedFile, TrackedFileListResponse } from '@/types/file'
import type { TrackedFolder } from '@/types/folder'
import type { BulkAssignResult } from '@/types/virtual-path'

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
        <h2 className="text-lg font-semibold">Set Virtual Path</h2>
        <p className="mt-1 text-sm text-muted-foreground font-mono">{file.relative_path}</p>

        <div className="mt-4 space-y-3">
          <div>
            <label htmlFor="virtual-path-value" className="mb-1 block text-sm font-medium">
              Virtual Path
            </label>
            <input
              id="virtual-path-value"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder="/virtual/path/example"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
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
  const [folders, setFolders] = useState<TrackedFolder[]>([])
  const [loading, setLoading] = useState(true)
  const [page, setPage] = useState(1)
  const [editingFile, setEditingFile] = useState<TrackedFile | null>(null)
  const [removeTarget, setRemoveTarget] = useState<TrackedFile | null>(null)
  const [bulkInput, setBulkInput] = useState('')
  const [bulkResult, setBulkResult] = useState<BulkAssignResult | null>(null)
  const [bulkFromReal, setBulkFromReal] = useState({
    drive_pair_id: '',
    folder_path: '',
    virtual_base: '',
  })
  const [refreshSummary, setRefreshSummary] = useState<{
    created: number
    removed: number
    errors: string[]
  } | null>(null)

  const loadFiles = useCallback(async (nextPage = page) => {
    setLoading(true)
    try {
      const nextResponse = await filesApi.list({ page: nextPage, per_page: 50 })
      setResponse(nextResponse)
    } catch {
      toast.error('Failed to load tracked files')
    } finally {
      setLoading(false)
    }
  }, [page])

  useEffect(() => {
    let active = true

    const loadSupportData = async () => {
      try {
        const [nextDrives, nextFolders] = await Promise.all([drivesApi.list(), foldersApi.list()])
        if (active) {
          setDrives(nextDrives)
          setFolders(nextFolders)
        }
      } catch {
        toast.error('Failed to load drive and folder metadata')
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
      toast.success(`Virtual path updated for file #${fileId}`)
      await loadFiles(page)
    } catch {
      toast.error(`Failed to update virtual path for file #${fileId}`)
    }
  }

  const removeVirtualPath = async () => {
    if (!removeTarget) return

    try {
      await virtualPathsApi.remove(removeTarget.id)
      toast.success(`Removed virtual path from file #${removeTarget.id}`)
      setRemoveTarget(null)
      await loadFiles(page)
    } catch {
      toast.error(`Failed to remove virtual path from file #${removeTarget.id}`)
    }
  }

  const refreshSymlinks = async () => {
    try {
      const result = await virtualPathsApi.refresh()
      setRefreshSummary(result)
      toast.success(`Refreshed symlinks (${result.created} created, ${result.removed} removed)`)
    } catch {
      toast.error('Failed to refresh symlinks')
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

    try {
      const result = await virtualPathsApi.bulk({ entries })
      setBulkResult(result)
      if (result.failed.length === 0) {
        toast.success(`Assigned ${result.succeeded.length} virtual path(s)`)
      } else {
        toast.warning(
          `Assigned ${result.succeeded.length} path(s) with ${result.failed.length} failure(s)`
        )
      }
      await loadFiles(page)
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Bulk assignment failed'
      toast.error(message)
    }
  }

  const submitBulkFromReal = async () => {
    if (!bulkFromReal.drive_pair_id || !bulkFromReal.folder_path.trim() || !bulkFromReal.virtual_base.trim()) {
      toast.error('Drive, folder path, and virtual base are required')
      return
    }

    try {
      const result = await virtualPathsApi.bulkFromReal({
        drive_pair_id: Number(bulkFromReal.drive_pair_id),
        folder_path: bulkFromReal.folder_path.trim(),
        virtual_base: bulkFromReal.virtual_base.trim(),
      })
      setBulkResult(result)
      toast.success(
        `Generated ${result.succeeded.length} virtual path(s) from real paths`
      )
      await loadFiles(page)
    } catch {
      toast.error('Failed to assign virtual paths from real paths')
    }
  }

  const files = response?.files ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold">Virtual Paths</h1>
          <p className="text-sm text-muted-foreground">
            Manage individual and bulk symlink mappings for tracked files.
          </p>
        </div>
        <button
          onClick={() => void refreshSymlinks()}
          className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <RefreshCw className="h-4 w-4" />
          Refresh Symlinks
        </button>
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.2fr_1fr]">
        <div className="space-y-4 rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2">
            <Link2 className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-semibold">Bulk Assign</h2>
          </div>
          <p className="text-sm text-muted-foreground">
            Enter one mapping per line in the format <span className="font-mono">file_id|/virtual/path</span>.
          </p>
          <textarea
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
            Apply Bulk Assignments
          </button>
        </div>

        <div className="space-y-4 rounded-lg border border-border bg-card p-4">
          <div className="flex items-center gap-2">
            <Wand2 className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-semibold">Bulk From Real Paths</h2>
          </div>
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
                Virtual Base
              </label>
              <input
                id="bulk-base"
                value={bulkFromReal.virtual_base}
                onChange={(event) =>
                  setBulkFromReal((current) => ({ ...current, virtual_base: event.target.value }))
                }
                placeholder="/virtual/documents"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              />
            </div>
          </div>
          <button
            onClick={() => void submitBulkFromReal()}
            className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            Generate From Real Paths
          </button>
          {folders.length > 0 && (
            <div className="rounded-md border border-dashed border-border p-3 text-xs text-muted-foreground">
              Tracked folders: {folders.map((folder) => folder.folder_path).join(', ')}
            </div>
          )}
        </div>
      </div>

      {refreshSummary && (
        <div className="rounded-lg border border-blue-200 bg-blue-50 p-4 text-sm text-blue-900">
          Refreshed symlinks: {refreshSummary.created} created, {refreshSummary.removed} removed
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
                header: 'Virtual Path',
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
                      Set
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
            emptyState={
              <EmptyState
                title="No tracked files"
                description="Track files first, then assign virtual paths from this page."
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
        title="Remove virtual path?"
        description={`Remove the virtual path for "${removeTarget?.relative_path}"?`}
        confirmLabel="Remove"
        destructive
        onConfirm={removeVirtualPath}
      />
    </div>
  )
}
