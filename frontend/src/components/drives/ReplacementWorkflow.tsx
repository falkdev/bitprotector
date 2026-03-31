import { useState } from 'react'
import { toast } from 'sonner'
import { X } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import type { DrivePair, DriveRole } from '@/types/drive'

interface ReplacementWorkflowProps {
  drive: DrivePair
  onClose: () => void
  onUpdate: (id: number) => Promise<void>
}

export function ReplacementWorkflow({ drive, onClose, onUpdate }: ReplacementWorkflowProps) {
  const [newPath, setNewPath] = useState('')
  const [role, setRole] = useState<DriveRole>('primary')
  const [skipValidation, setSkipValidation] = useState(false)
  const [loading, setLoading] = useState(false)
  const [showPicker, setShowPicker] = useState(false)

  const run = async (action: () => Promise<unknown>, label: string) => {
    setLoading(true)
    try {
      await action()
      toast.success(label)
      await onUpdate(drive.id)
    } catch {
      toast.error(`Failed: ${label}`)
    } finally {
      setLoading(false)
    }
  }

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-semibold">Replacement Workflow — {drive.name}</h2>
          <button
            onClick={onClose}
            className="rounded p-1 hover:bg-accent transition-colors"
            data-testid="close-replacement-workflow"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4">
          {/* Role selector */}
          <div>
            <label className="mb-1 block text-sm font-medium">Drive Role</label>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value as DriveRole)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="primary">Primary</option>
              <option value="secondary">Secondary</option>
            </select>
          </div>

          {/* Action buttons */}
          <div className="grid grid-cols-3 gap-2">
            <button
              disabled={loading}
              onClick={() => run(() => drivesApi.markReplacement(drive.id, { role }), 'Marked for replacement')}
              className="rounded-md border border-yellow-300 bg-yellow-50 px-2 py-2 text-xs font-medium text-yellow-700 hover:bg-yellow-100 transition-colors disabled:opacity-60"
              data-testid="mark-replacement-button"
            >
              Mark
            </button>
            <button
              disabled={loading}
              onClick={() => run(() => drivesApi.confirmFailure(drive.id, { role }), 'Failure confirmed')}
              className="rounded-md border border-red-300 bg-red-50 px-2 py-2 text-xs font-medium text-red-700 hover:bg-red-100 transition-colors disabled:opacity-60"
              data-testid="confirm-failure-button"
            >
              Confirm Failure
            </button>
            <button
              disabled={loading}
              onClick={() => run(() => drivesApi.cancelReplacement(drive.id, { role }), 'Replacement cancelled')}
              className="rounded-md border border-border px-2 py-2 text-xs font-medium hover:bg-accent transition-colors disabled:opacity-60"
              data-testid="cancel-replacement-button"
            >
              Cancel
            </button>
          </div>

          {/* Assign new path */}
          <div className="rounded-lg border border-border p-3 space-y-2">
            <p className="text-xs font-medium">Assign Replacement Drive</p>
            <div className="flex gap-2">
              <input
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
                placeholder="/mnt/new-drive"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                data-testid="assign-path-input"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent transition-colors"
              >
                Browse
              </button>
            </div>
            <label className="flex items-center gap-2 text-xs">
              <input
                type="checkbox"
                checked={skipValidation}
                onChange={(e) => setSkipValidation(e.target.checked)}
              />
              Skip path validation
            </label>
            <button
              disabled={loading || !newPath.trim()}
              onClick={() =>
                run(
                  () => drivesApi.assignReplacement(drive.id, { role, new_path: newPath, skip_validation: skipValidation }),
                  'Replacement drive assigned'
                )
              }
              className="w-full rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-60"
              data-testid="assign-replacement-button"
            >
              Assign Replacement
            </button>
          </div>
        </div>
      </div>
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select Replacement Drive Path"
        description="Browse the BitProtector host filesystem and choose the mounted directory for the replacement drive."
        mode="directory"
        value={newPath}
        startPath={newPath}
        confirmLabel="Use Directory"
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setNewPath(path)
          setShowPicker(false)
        }}
      />
    </>
  )
}
