import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Plus, ScanLine, Trash2 } from 'lucide-react'
import { foldersApi } from '@/api/folders'
import { drivesApi } from '@/api/drives'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { EmptyState } from '@/components/shared/EmptyState'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import type { TrackedFolder, CreateFolderRequest, ScanFolderResult } from '@/types/folder'
import type { DrivePair } from '@/types/drive'
import { formatDate } from '@/lib/format'
import { getActiveDrivePath, resolveAbsolutePathForPicker, resolveTrackedPathInput } from '@/lib/path'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'

const schema = z.object({
  drive_pair_id: z.coerce.number().min(1, 'Select a drive pair'),
  folder_path: z.string().min(1, 'Folder path is required'),
  virtual_path: z
    .string()
    .optional()
    .refine((value) => !value || value.trim().startsWith('/'), 'Publish path must be absolute'),
})

type FormData = z.infer<typeof schema>

export function FoldersPage() {
  const [folders, setFolders] = useState<TrackedFolder[]>([])
  const [drives, setDrives] = useState<DrivePair[]>([])
  const [loading, setLoading] = useState(true)
  const [showForm, setShowForm] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<TrackedFolder | null>(null)
  const [scanResult, setScanResult] = useState<{ folder: TrackedFolder; result: ScanFolderResult } | null>(null)

  const load = async () => {
    setLoading(true)
    try {
      const [f, d] = await Promise.all([foldersApi.list(), drivesApi.list()])
      setFolders(f)
      setDrives(d)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { void load() }, [])

  const handleScan = async (folder: TrackedFolder) => {
    try {
      const result = await foldersApi.scan(folder.id)
      setScanResult({ folder, result })
      toast.success(`Scan complete: ${result.new_files} new, ${result.changed_files} changed`)
    } catch {
      toast.error('Scan failed')
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget) return
    try {
      await foldersApi.delete(deleteTarget.id)
      setFolders((f) => f.filter((x) => x.id !== deleteTarget.id))
      toast.success('Folder removed')
    } catch {
      toast.error('Failed to remove folder')
    } finally {
      setDeleteTarget(null)
    }
  }

  const driveName = (id: number) => drives.find((d) => d.id === id)?.name ?? String(id)

  if (loading) return <div className="flex items-center justify-center py-16"><LoadingSpinner /></div>

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">Tracked Folders</h1>
          <p className="text-sm text-muted-foreground">Track folders for scanning and optionally publish them at an exact path</p>
        </div>
        <button
          onClick={() => setShowForm(true)}
          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
          data-testid="add-folder-button"
        >
          <Plus className="h-4 w-4" /> Add Folder
        </button>
      </div>

      {scanResult && (
        <div className="rounded-lg border border-green-200 bg-green-50 p-4 text-sm">
          <strong>Scan results for {scanResult.folder.folder_path}:</strong>{' '}
          {scanResult.result.new_files} new files, {scanResult.result.changed_files} changed files.
          <button onClick={() => setScanResult(null)} className="ml-3 text-xs underline">Dismiss</button>
        </div>
      )}

      <DataTable
        tableTestId="folders-table"
        columns={[
          { key: 'path', header: 'Path', cell: (f) => <span className="font-mono text-xs">{f.folder_path}</span> },
          { key: 'drive', header: 'Drive Pair', cell: (f) => driveName(f.drive_pair_id) },
          { key: 'publish', header: 'Publish Path', cell: (f) => f.virtual_path ? <span className="font-mono text-xs">{f.virtual_path}</span> : '—' },
          { key: 'created', header: 'Created', cell: (f) => formatDate(f.created_at) },
          {
            key: 'actions',
            header: '',
            cell: (f) => (
              <div className="flex gap-1">
                <button
                  onClick={() => handleScan(f)}
                  className="flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-accent transition-colors"
                  data-testid={`scan-folder-${f.id}`}
                >
                  <ScanLine className="h-3 w-3" /> Scan
                </button>
                <button
                  onClick={() => setDeleteTarget(f)}
                  className="flex items-center gap-1 rounded px-2 py-1 text-xs text-destructive hover:bg-destructive/10 transition-colors"
                  data-testid={`delete-folder-${f.id}`}
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
            ),
          },
        ]}
        data={folders}
        rowKey={(f) => f.id}
        rowTestId={(f) => `folder-row-${f.id}`}
        emptyState={<EmptyState title="No tracked folders" description="Add a folder to auto-discover files" />}
      />

      {showForm && (
        <FolderFormModal
          drives={drives}
          onClose={() => setShowForm(false)}
          onSave={async (data) => {
            const folder = await foldersApi.create(data)
            setFolders((f) => [...f, folder])
            toast.success('Folder added')
            setShowForm(false)
          }}
        />
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
        title="Remove tracked folder?"
        description="Files already tracked will remain. Only the folder scan association is removed."
        confirmLabel="Remove"
        destructive
        onConfirm={handleDelete}
      />
    </div>
  )
}

export function FolderFormModal({
  drives,
  onClose,
  onSave,
}: {
  drives: DrivePair[]
  onClose: () => void
  onSave: (data: CreateFolderRequest) => Promise<void>
}) {
  const {
    register,
    handleSubmit,
    setError,
    clearErrors,
    setValue,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    resolver: zodResolver(schema) as never,
  })
  const [showPicker, setShowPicker] = useState(false)
  const drivePairId = watch('drive_pair_id')
  const rawPath = watch('folder_path') ?? ''
  const selectedDrive = drives.find((drive) => drive.id === Number(drivePairId))
  const activeRoot = selectedDrive
    ? getActiveDrivePath(
        selectedDrive.primary_path,
        selectedDrive.secondary_path,
        selectedDrive.active_role
      )
    : null
  const pathResolution = resolveTrackedPathInput(activeRoot, rawPath)

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-2xl rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="mb-4 font-semibold">Add Tracked Folder</h2>
          <form onSubmit={handleSubmit(async (d) => {
            const resolution = resolveTrackedPathInput(activeRoot, d.folder_path)
            if (resolution.error || !resolution.relativePath) {
              setError('folder_path', {
                type: 'manual',
                message: resolution.error ?? 'Folder path is required',
              })
              return
            }

            clearErrors('folder_path')
            await onSave({
              drive_pair_id: Number(d.drive_pair_id),
              folder_path: resolution.relativePath,
              virtual_path: d.virtual_path?.trim() || undefined,
            })
          })} className="space-y-4">
          <div>
            <label htmlFor="folder-drive-pair" className="mb-1 block text-sm font-medium">Drive Pair</label>
            <select
              id="folder-drive-pair"
              {...register('drive_pair_id')}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="">Select…</option>
              {drives.map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
            </select>
            {errors.drive_pair_id && <p className="mt-1 text-xs text-destructive">{errors.drive_pair_id.message}</p>}
          </div>
          <div>
            <label htmlFor="folder-path" className="mb-1 block text-sm font-medium">Folder Path</label>
            <div className="flex gap-2">
              <input
                id="folder-path"
                {...register('folder_path')}
                placeholder="documents or /mnt/drive-a/documents"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                disabled={!selectedDrive}
                className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent transition-colors disabled:cursor-not-allowed disabled:opacity-60"
              >
                Browse
              </button>
            </div>
            <p className="mt-1 text-xs text-muted-foreground">
              {selectedDrive
                ? `Active root: ${activeRoot}`
                : 'Select a drive pair before browsing or submitting.'}
            </p>
            {selectedDrive && rawPath.trim() && !pathResolution.error && pathResolution.relativePath ? (
              <p className="mt-1 text-xs text-muted-foreground">
                Will be stored as <span className="font-mono">{pathResolution.relativePath}</span>
              </p>
            ) : null}
            {errors.folder_path && <p className="mt-1 text-xs text-destructive">{errors.folder_path.message}</p>}
          </div>
          <div>
            <label htmlFor="folder-virtual-path" className="mb-1 block text-sm font-medium">Publish Path (optional)</label>
            <input
              id="folder-virtual-path"
              {...register('virtual_path')}
              placeholder="/docs"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
            />
            <p className="mt-1 text-xs text-muted-foreground">
              If set, BitProtector will create a symlink exactly at this path to the tracked folder.
            </p>
            {errors.virtual_path && <p className="mt-1 text-xs text-destructive">{errors.virtual_path.message}</p>}
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent transition-colors">Cancel</button>
            <button type="submit" disabled={isSubmitting} className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-60">
              {isSubmitting ? 'Adding…' : 'Add Folder'}
            </button>
          </div>
        </form>
      </div>
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select Folder Path"
        description="Browse the BitProtector host filesystem and choose a directory under the selected drive pair’s active root."
        mode="directory"
        value={rawPath}
        startPath={resolveAbsolutePathForPicker(activeRoot, rawPath)}
        confirmLabel="Use Folder Path"
        validatePath={(path) => resolveTrackedPathInput(activeRoot, path).error}
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue('folder_path', path, { shouldDirty: true, shouldValidate: true })
          clearErrors('folder_path')
          setShowPicker(false)
        }}
      />
    </>
  )
}
