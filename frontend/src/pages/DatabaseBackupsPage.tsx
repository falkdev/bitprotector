import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Plus, Database } from 'lucide-react'
import { databaseApi } from '@/api/database'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { formatDate } from '@/lib/format'
import type {
  CreateBackupConfigRequest,
  DbBackupConfig,
  RunBackupResult,
  UpdateBackupConfigRequest,
} from '@/types/database'

function BackupFormModal({
  backup,
  onClose,
  onSave,
}: {
  backup: DbBackupConfig | null
  onClose: () => void
  onSave: (data: CreateBackupConfigRequest | UpdateBackupConfigRequest) => Promise<void>
}) {
  const [backupPath, setBackupPath] = useState('')
  const [driveLabel, setDriveLabel] = useState('')
  const [maxCopies, setMaxCopies] = useState('5')
  const [enabled, setEnabled] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setBackupPath(backup?.backup_path ?? '')
    setDriveLabel(backup?.drive_label ?? '')
    setMaxCopies(String(backup?.max_copies ?? 5))
    setEnabled(backup?.enabled ?? true)
    setSaving(false)
    setError(null)
  }, [backup])

  const submit = async () => {
    const parsedMaxCopies = Number(maxCopies)
    if (!Number.isFinite(parsedMaxCopies) || parsedMaxCopies <= 0) {
      setError('Max copies must be a positive number.')
      return
    }
    if (!backup && !backupPath.trim()) {
      setError('Backup path is required.')
      return
    }

    setSaving(true)
    setError(null)

    try {
      if (backup) {
        await onSave({
          max_copies: parsedMaxCopies,
          enabled,
        })
      } else {
        await onSave({
          backup_path: backupPath.trim(),
          drive_label: driveLabel.trim() || undefined,
          max_copies: parsedMaxCopies,
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
          {backup ? 'Edit Backup Destination' : 'Add Backup Destination'}
        </h2>

        <div className="mt-4 space-y-4">
          {!backup ? (
            <>
              <div>
                <label htmlFor="backup-path" className="mb-1 block text-sm font-medium">
                  Backup Path
                </label>
                <input
                  id="backup-path"
                  value={backupPath}
                  onChange={(event) => setBackupPath(event.target.value)}
                  placeholder="/mnt/backup-drive/bitprotector"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                />
              </div>
              <div>
                <label htmlFor="drive-label" className="mb-1 block text-sm font-medium">
                  Drive Label
                </label>
                <input
                  id="drive-label"
                  value={driveLabel}
                  onChange={(event) => setDriveLabel(event.target.value)}
                  placeholder="usb-backup-1"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                />
              </div>
            </>
          ) : (
            <div className="rounded-md border border-border bg-muted/40 p-3 text-sm">
              <p className="font-medium">{backup.backup_path}</p>
              <p className="text-muted-foreground">{backup.drive_label ?? 'No drive label'}</p>
            </div>
          )}

          <div>
            <label htmlFor="max-copies" className="mb-1 block text-sm font-medium">
              Max Copies
            </label>
            <input
              id="max-copies"
              type="number"
              min={1}
              value={maxCopies}
              onChange={(event) => setMaxCopies(event.target.value)}
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
            {saving ? 'Saving…' : backup ? 'Save Changes' : 'Create Destination'}
          </button>
        </div>
      </div>
    </div>
  )
}

export function DatabaseBackupsPage() {
  const [backups, setBackups] = useState<DbBackupConfig[]>([])
  const [loading, setLoading] = useState(true)
  const [showCreate, setShowCreate] = useState(false)
  const [editTarget, setEditTarget] = useState<DbBackupConfig | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DbBackupConfig | null>(null)
  const [dbPath, setDbPath] = useState(
    import.meta.env.VITE_DB_PATH ?? '/var/lib/bitprotector/bitprotector.db'
  )
  const [runningBackup, setRunningBackup] = useState(false)
  const [runResults, setRunResults] = useState<RunBackupResult[]>([])

  const loadBackups = async () => {
    setLoading(true)
    try {
      const nextBackups = await databaseApi.listBackups()
      setBackups(nextBackups)
    } catch {
      toast.error('Failed to load backup destinations')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void loadBackups()
  }, [])

  const closeForm = () => {
    setShowCreate(false)
    setEditTarget(null)
  }

  const saveBackup = async (data: CreateBackupConfigRequest | UpdateBackupConfigRequest) => {
    try {
      if (editTarget) {
        await databaseApi.updateBackup(editTarget.id, data as UpdateBackupConfigRequest)
        toast.success('Backup destination updated')
      } else {
        await databaseApi.createBackup(data as CreateBackupConfigRequest)
        toast.success('Backup destination created')
      }
      closeForm()
      await loadBackups()
    } catch {
      toast.error('Failed to save backup destination')
    }
  }

  const deleteBackup = async () => {
    if (!deleteTarget) return

    try {
      await databaseApi.deleteBackup(deleteTarget.id)
      setBackups((current) => current.filter((backup) => backup.id !== deleteTarget.id))
      setDeleteTarget(null)
      toast.success('Backup destination deleted')
    } catch {
      toast.error('Failed to delete backup destination')
    }
  }

  const runBackupNow = async () => {
    if (!dbPath.trim()) {
      toast.error('Database path is required')
      return
    }

    setRunningBackup(true)
    try {
      const results = await databaseApi.runBackup(dbPath.trim())
      setRunResults(results)
      const failures = results.filter((result) => result.status === 'failed').length
      if (failures === 0) {
        toast.success(`Backed up to ${results.length} destination(s)`)
      } else {
        toast.warning(`Backup completed with ${failures} failure(s)`)
      }
      await loadBackups()
    } catch {
      toast.error('Failed to run database backup')
    } finally {
      setRunningBackup(false)
    }
  }

  if (loading && backups.length === 0) {
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
          <h1 className="text-xl font-semibold">Database Backups</h1>
          <p className="text-sm text-muted-foreground">
            Manage backup destinations and trigger manual database backups.
          </p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Plus className="h-4 w-4" />
          Add Destination
        </button>
      </div>

      <div className="rounded-lg border border-border bg-card p-4">
        <div className="grid gap-4 md:grid-cols-[1fr_auto] md:items-end">
          <div>
            <label htmlFor="database-path" className="mb-1 block text-sm font-medium">
              Database Path
            </label>
            <input
              id="database-path"
              value={dbPath}
              onChange={(event) => setDbPath(event.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <button
            onClick={() => void runBackupNow()}
            disabled={runningBackup}
            className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
          >
            <Database className="h-4 w-4" />
            {runningBackup ? 'Running…' : 'Run Backup Now'}
          </button>
        </div>
      </div>

      <DataTable
        columns={[
          {
            key: 'backup_path',
            header: 'Backup Path',
            cell: (backup) => <span className="font-mono text-xs">{backup.backup_path}</span>,
          },
          {
            key: 'drive_label',
            header: 'Drive Label',
            cell: (backup) => backup.drive_label ?? '—',
          },
          {
            key: 'max_copies',
            header: 'Max Copies',
            cell: (backup) => backup.max_copies,
          },
          {
            key: 'enabled',
            header: 'Enabled',
            cell: (backup) =>
              backup.enabled ? (
                <span className="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800">
                  Enabled
                </span>
              ) : (
                <span className="rounded-full bg-gray-100 px-2 py-0.5 text-xs font-medium text-gray-700">
                  Disabled
                </span>
              ),
          },
          {
            key: 'last_backup',
            header: 'Last Backup',
            cell: (backup) => formatDate(backup.last_backup),
          },
          {
            key: 'actions',
            header: '',
            cell: (backup) => (
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setEditTarget(backup)}
                  className="rounded-md border border-border px-3 py-1.5 text-xs hover:bg-accent"
                >
                  Edit
                </button>
                <button
                  onClick={() => setDeleteTarget(backup)}
                  className="rounded-md border border-border px-3 py-1.5 text-xs text-destructive hover:bg-destructive/10"
                >
                  Delete
                </button>
              </div>
            ),
          },
        ]}
        data={backups}
        rowKey={(backup) => backup.id}
        emptyState={
          <EmptyState
            title="No backup destinations configured"
            description="Create a destination before running manual database backups."
          />
        }
      />

      {runResults.length > 0 && (
        <div className="rounded-lg border border-border bg-card p-4">
          <h2 className="text-sm font-semibold">Latest Backup Run</h2>
          <div className="mt-3 space-y-2">
            {runResults.map((result) => (
              <div
                key={`${result.backup_config_id}-${result.backup_path}`}
                className="flex items-start justify-between gap-4 rounded-md border border-border px-3 py-2 text-sm"
              >
                <div>
                  <p className="font-mono text-xs">{result.backup_path}</p>
                  {result.error && (
                    <p className="mt-1 text-xs text-destructive">{result.error}</p>
                  )}
                </div>
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                    result.status === 'success'
                      ? 'bg-green-100 text-green-800'
                      : 'bg-red-100 text-red-800'
                  }`}
                >
                  {result.status}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {(showCreate || editTarget) && (
        <BackupFormModal backup={editTarget} onClose={closeForm} onSave={saveBackup} />
      )}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title="Delete backup destination?"
        description={`Delete "${deleteTarget?.backup_path ?? ''}" from the backup configuration list?`}
        confirmLabel="Delete"
        destructive
        onConfirm={deleteBackup}
      />
    </div>
  )
}
