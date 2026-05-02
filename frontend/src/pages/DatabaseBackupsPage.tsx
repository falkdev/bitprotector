import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  Database,
  FolderOpen,
  Plus,
  RefreshCw,
  RotateCcw,
  Settings,
  ShieldCheck,
} from 'lucide-react'
import { databaseApi } from '@/api/database'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PageIntro } from '@/components/shared/PageIntro'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { formatDate } from '@/lib/format'
import type {
  BackupIntegrityResult,
  CreateBackupConfigRequest,
  DbBackupConfig,
  DbBackupSettings,
  RestoreBackupResult,
  RunBackupResult,
  UpdateBackupConfigRequest,
  UpdateBackupSettingsRequest,
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
  const [enabled, setEnabled] = useState(true)
  const [saving, setSaving] = useState(false)
  const [showPicker, setShowPicker] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setBackupPath(backup?.backup_path ?? '')
    setDriveLabel(backup?.drive_label ?? '')
    setEnabled(backup?.enabled ?? true)
    setSaving(false)
    setError(null)
  }, [backup])

  const submit = async () => {
    if (!backupPath.trim()) {
      setError('Backup path is required.')
      return
    }

    setSaving(true)
    setError(null)

    try {
      if (backup) {
        await onSave({
          backup_path: backupPath.trim(),
          drive_label: driveLabel.trim() || null,
          enabled,
        })
      } else {
        await onSave({
          backup_path: backupPath.trim(),
          drive_label: driveLabel.trim() || undefined,
          enabled,
        })
      }
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <ModalLayer>
        <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
          <h2 className="text-lg font-semibold">
            {backup ? 'Edit Backup Destination' : 'Add Backup Destination'}
          </h2>

          <div className="mt-4 space-y-4">
            <div>
              <label htmlFor="backup-path" className="mb-1 block text-sm font-medium">
                Backup Path
              </label>
              <div className="flex gap-2">
                <input
                  id="backup-path"
                  value={backupPath}
                  onChange={(event) => setBackupPath(event.target.value)}
                  placeholder="/mnt/backup-drive/bitprotector"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                />
                <button
                  type="button"
                  onClick={() => setShowPicker(true)}
                  className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm hover:bg-accent"
                >
                  <FolderOpen className="h-4 w-4" />
                  Browse
                </button>
              </div>
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
              {saving ? 'Saving...' : backup ? 'Save Changes' : 'Create Destination'}
            </button>
          </div>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={showPicker}
        title="Select Backup Destination"
        description="Choose the folder that should contain this destination's bitprotector.db backup."
        mode="directory"
        value={backupPath}
        startPath={backupPath || '/'}
        confirmLabel="Use Backup Folder"
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setBackupPath(path)
          setShowPicker(false)
        }}
      />
    </>
  )
}

function SettingsModal({
  settings,
  onClose,
  onSave,
}: {
  settings: DbBackupSettings
  onClose: () => void
  onSave: (data: UpdateBackupSettingsRequest) => Promise<void>
}) {
  const [backupEnabled, setBackupEnabled] = useState(settings.backup_enabled)
  const [backupInterval, setBackupInterval] = useState(String(settings.backup_interval_seconds))
  const [integrityEnabled, setIntegrityEnabled] = useState(settings.integrity_enabled)
  const [integrityInterval, setIntegrityInterval] = useState(
    String(settings.integrity_interval_seconds)
  )
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    const backupSeconds = Number(backupInterval)
    const integritySeconds = Number(integrityInterval)
    if (!Number.isFinite(backupSeconds) || backupSeconds <= 0) {
      setError('Backup interval must be greater than zero.')
      return
    }
    if (!Number.isFinite(integritySeconds) || integritySeconds <= 0) {
      setError('Integrity interval must be greater than zero.')
      return
    }

    setSaving(true)
    setError(null)
    try {
      await onSave({
        backup_enabled: backupEnabled,
        backup_interval_seconds: backupSeconds,
        integrity_enabled: integrityEnabled,
        integrity_interval_seconds: integritySeconds,
      })
    } finally {
      setSaving(false)
    }
  }

  return (
    <ModalLayer>
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <h2 className="text-lg font-semibold">Backup Settings</h2>
        <div className="mt-4 space-y-4">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={backupEnabled}
              onChange={(event) => setBackupEnabled(event.target.checked)}
            />
            Automatic backups
          </label>
          <div>
            <label htmlFor="backup-interval" className="mb-1 block text-sm font-medium">
              Backup Interval Seconds
            </label>
            <input
              id="backup-interval"
              type="number"
              min={1}
              value={backupInterval}
              onChange={(event) => setBackupInterval(event.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={integrityEnabled}
              onChange={(event) => setIntegrityEnabled(event.target.checked)}
            />
            Automatic backup integrity checks
          </label>
          <div>
            <label htmlFor="integrity-interval" className="mb-1 block text-sm font-medium">
              Integrity Interval Seconds
            </label>
            <input
              id="integrity-interval"
              type="number"
              min={1}
              value={integrityInterval}
              onChange={(event) => setIntegrityInterval(event.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
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
            {saving ? 'Saving...' : 'Save Settings'}
          </button>
        </div>
      </div>
    </ModalLayer>
  )
}

function RestoreModal({
  onClose,
  onRestore,
}: {
  onClose: () => void
  onRestore: (sourcePath: string) => Promise<void>
}) {
  const [sourcePath, setSourcePath] = useState('')
  const [showPicker, setShowPicker] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    if (!sourcePath.trim()) {
      setError('Backup file is required.')
      return
    }
    if (!sourcePath.trim().endsWith('.db')) {
      setError('Select a .db backup file.')
      return
    }

    setSaving(true)
    setError(null)
    try {
      await onRestore(sourcePath.trim())
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <ModalLayer>
        <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
          <h2 className="text-lg font-semibold">Restore Older Backup</h2>
          <div className="mt-4 space-y-4">
            <div>
              <label htmlFor="restore-path" className="mb-1 block text-sm font-medium">
                Backup File
              </label>
              <div className="flex gap-2">
                <input
                  id="restore-path"
                  value={sourcePath}
                  onChange={(event) => setSourcePath(event.target.value)}
                  placeholder="/mnt/backups/bitprotector.db"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                />
                <button
                  type="button"
                  onClick={() => setShowPicker(true)}
                  className="inline-flex items-center gap-2 rounded-md border border-border px-3 py-2 text-sm hover:bg-accent"
                >
                  <FolderOpen className="h-4 w-4" />
                  Browse
                </button>
              </div>
            </div>
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
              {saving ? 'Staging...' : 'Stage Restore'}
            </button>
          </div>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={showPicker}
        title="Select Backup File"
        description="Choose an older SQLite database backup to restore after restart."
        mode="file"
        value={sourcePath}
        startPath={sourcePath || '/'}
        confirmLabel="Use Backup File"
        validatePath={(path) => (path.endsWith('.db') ? null : 'Select a .db backup file')}
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setSourcePath(path)
          setShowPicker(false)
        }}
      />
    </>
  )
}

function IntegrityStatus({ value }: { value: string | null }) {
  if (!value) return <>-</>
  const classes =
    value === 'ok' || value === 'repaired'
      ? 'bg-green-100 text-green-800'
      : 'bg-red-100 text-red-800'
  return <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${classes}`}>{value}</span>
}

export function DatabaseBackupsPage() {
  const [backups, setBackups] = useState<DbBackupConfig[]>([])
  const [settings, setSettings] = useState<DbBackupSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [settingsLoading, setSettingsLoading] = useState(false)
  const [showCreate, setShowCreate] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [showRestore, setShowRestore] = useState(false)
  const [editTarget, setEditTarget] = useState<DbBackupConfig | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DbBackupConfig | null>(null)
  const [runningBackup, setRunningBackup] = useState(false)
  const [runningIntegrity, setRunningIntegrity] = useState(false)
  const [runResults, setRunResults] = useState<RunBackupResult[]>([])
  const [integrityResults, setIntegrityResults] = useState<BackupIntegrityResult[]>([])
  const [restoreResult, setRestoreResult] = useState<RestoreBackupResult | null>(null)

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

  const loadSettings = async () => {
    setSettingsLoading(true)
    try {
      const nextSettings = await databaseApi.getSettings()
      setSettings(nextSettings)
    } catch {
      toast.error('Failed to load backup settings')
    } finally {
      setSettingsLoading(false)
    }
  }

  useEffect(() => {
    void loadBackups()
    void loadSettings()
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

  const saveSettings = async (data: UpdateBackupSettingsRequest) => {
    try {
      const nextSettings = await databaseApi.updateSettings(data)
      setSettings(nextSettings)
      setShowSettings(false)
      toast.success('Backup settings updated')
    } catch {
      toast.error('Failed to save backup settings')
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
    setRunningBackup(true)
    try {
      const results = await databaseApi.runBackup()
      setRunResults(results)
      const failures = results.filter((result) => result.status === 'failed').length
      if (failures === 0) {
        toast.success(`Backed up to ${results.length} destination(s)`)
      } else {
        toast.warning(`Backup completed with ${failures} failure(s)`)
      }
      await loadBackups()
      await loadSettings()
    } catch {
      toast.error('Failed to run database backup')
    } finally {
      setRunningBackup(false)
    }
  }

  const runIntegrityNow = async () => {
    setRunningIntegrity(true)
    try {
      const results = await databaseApi.runIntegrityCheck()
      setIntegrityResults(results)
      const failures = results.filter(
        (result) => result.status === 'corrupt' || result.status === 'failed'
      ).length
      if (failures === 0) {
        toast.success('Backup integrity check completed')
      } else {
        toast.warning(`Integrity check found ${failures} unresolved backup(s)`)
      }
      await loadBackups()
      await loadSettings()
    } catch {
      toast.error('Failed to run backup integrity check')
    } finally {
      setRunningIntegrity(false)
    }
  }

  const restoreBackup = async (sourcePath: string) => {
    try {
      const result = await databaseApi.restoreBackup({ source_path: sourcePath })
      setRestoreResult(result)
      setShowRestore(false)
      toast.success('Restore staged; restart BitProtector to apply it')
    } catch {
      toast.error('Failed to stage backup restore')
    }
  }

  if (loading && backups.length === 0) {
    return (
      <div className="space-y-6">
        <PageIntro
          title="Database Backups"
          subtitle="Manage backup destinations, integrity checks, and restore staging."
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
        title="Database Backups"
        subtitle="Manage backup destinations, integrity checks, and restore staging."
        actions={
          <div className="flex flex-wrap justify-end gap-2">
            <button
              onClick={() => setShowSettings(true)}
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-accent"
            >
              <Settings className="h-4 w-4" />
              Settings
            </button>
            <button
              onClick={() => setShowRestore(true)}
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-accent"
            >
              <RotateCcw className="h-4 w-4" />
              Restore Older Backup
            </button>
            <button
              onClick={() => setShowCreate(true)}
              className="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              <Plus className="h-4 w-4" />
              Add Destination
            </button>
          </div>
        }
      />

      <div className="flex flex-wrap gap-2">
        <button
          onClick={() => void runBackupNow()}
          disabled={runningBackup}
          className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
        >
          <Database className="h-4 w-4" />
          {runningBackup ? 'Running...' : 'Run Backup Now'}
        </button>
        <button
          onClick={() => void runIntegrityNow()}
          disabled={runningIntegrity}
          className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
        >
          <ShieldCheck className="h-4 w-4" />
          {runningIntegrity ? 'Checking...' : 'Check Integrity Now'}
        </button>
        <button
          onClick={() => {
            void loadBackups()
            void loadSettings()
          }}
          className="inline-flex items-center gap-2 rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
        >
          <RefreshCw className="h-4 w-4" />
          Reload
        </button>
      </div>

      {settings ? (
        <div className="grid gap-3 md:grid-cols-2">
          <div className="rounded-lg border border-border bg-card p-4 text-sm">
            <p className="font-medium">Automatic Backups</p>
            <p className="mt-1 text-muted-foreground">
              {settings.backup_enabled ? `Every ${settings.backup_interval_seconds}s` : 'Disabled'}
            </p>
          </div>
          <div className="rounded-lg border border-border bg-card p-4 text-sm">
            <p className="font-medium">Automatic Integrity Checks</p>
            <p className="mt-1 text-muted-foreground">
              {settings.integrity_enabled
                ? `Every ${settings.integrity_interval_seconds}s`
                : 'Disabled'}
            </p>
          </div>
        </div>
      ) : settingsLoading ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <LoadingSpinner size="sm" />
          Loading backup settings...
        </div>
      ) : null}

      <DataTable
        tableTestId="database-backups-table"
        columns={[
          {
            key: 'priority',
            header: 'Priority',
            cell: (backup) => backup.priority,
          },
          {
            key: 'backup_path',
            header: 'Backup Path',
            cell: (backup) => <span className="font-mono text-xs">{backup.backup_path}</span>,
          },
          {
            key: 'drive_label',
            header: 'Drive Label',
            cell: (backup) => backup.drive_label ?? '-',
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
            key: 'last_integrity_status',
            header: 'Integrity',
            cell: (backup) => <IntegrityStatus value={backup.last_integrity_status} />,
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
        rowTestId={(backup) => `database-backup-row-${backup.id}`}
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
                  {result.error && <p className="mt-1 text-xs text-destructive">{result.error}</p>}
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

      {integrityResults.length > 0 && (
        <div className="rounded-lg border border-border bg-card p-4">
          <h2 className="text-sm font-semibold">Latest Integrity Check</h2>
          <div className="mt-3 space-y-2">
            {integrityResults.map((result) => (
              <div
                key={`${result.backup_config_id}-${result.backup_path}`}
                className="flex items-start justify-between gap-4 rounded-md border border-border px-3 py-2 text-sm"
              >
                <div>
                  <p className="font-mono text-xs">{result.backup_path}</p>
                  {result.error && <p className="mt-1 text-xs text-destructive">{result.error}</p>}
                </div>
                <IntegrityStatus value={result.status} />
              </div>
            ))}
          </div>
        </div>
      )}

      {restoreResult && (
        <div className="rounded-lg border border-border bg-card p-4 text-sm">
          <h2 className="font-semibold">Restore Staged</h2>
          <p className="mt-2 text-muted-foreground">
            Restart BitProtector to apply the staged database restore.
          </p>
          <p className="mt-3 font-mono text-xs">{restoreResult.staged_restore_path}</p>
          <p className="mt-1 font-mono text-xs">{restoreResult.safety_backup_path}</p>
        </div>
      )}

      {(showCreate || editTarget) && (
        <BackupFormModal backup={editTarget} onClose={closeForm} onSave={saveBackup} />
      )}

      {showSettings && settings && (
        <SettingsModal
          settings={settings}
          onClose={() => setShowSettings(false)}
          onSave={saveSettings}
        />
      )}

      {showRestore && (
        <RestoreModal onClose={() => setShowRestore(false)} onRestore={restoreBackup} />
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
