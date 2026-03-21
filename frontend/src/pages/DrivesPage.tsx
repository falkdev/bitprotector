import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Plus } from 'lucide-react'
import { useDrivesStore } from '@/stores/drives-store'
import { drivesApi } from '@/api/drives'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { EmptyState } from '@/components/shared/EmptyState'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DriveCard } from '@/components/drives/DriveCard'
import { DriveForm } from '@/components/drives/DriveForm'
import { ReplacementWorkflow } from '@/components/drives/ReplacementWorkflow'
import type {
  DrivePair,
  CreateDrivePairRequest,
  UpdateDrivePairRequest,
} from '@/types/drive'

export function DrivesPage() {
  const { drives, loading, fetch, create, update, remove, refresh } = useDrivesStore()
  const [showForm, setShowForm] = useState(false)
  const [editTarget, setEditTarget] = useState<DrivePair | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DrivePair | null>(null)
  const [replacementTarget, setReplacementTarget] = useState<DrivePair | null>(null)

  useEffect(() => {
    void fetch()
  }, [fetch])

  const handleDelete = async () => {
    if (!deleteTarget) return
    try {
      await remove(deleteTarget.id)
      toast.success(`Drive pair "${deleteTarget.name}" deleted`)
    } catch {
      toast.error('Failed to delete drive pair')
    } finally {
      setDeleteTarget(null)
    }
  }

  const handleReplacementUpdate = async (id: number) => {
    try {
      const updated = await drivesApi.get(id)
      refresh(updated)
    } catch {
      void fetch()
    }
  }

  if (loading && drives.length === 0) {
    return (
      <div className="flex items-center justify-center py-16">
        <LoadingSpinner />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">Drive Pairs</h1>
          <p className="text-sm text-muted-foreground">Manage mirrored drive configurations</p>
        </div>
        <button
          onClick={() => setShowForm(true)}
          className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
          data-testid="add-drive-button"
        >
          <Plus className="h-4 w-4" /> Add Drive Pair
        </button>
      </div>

      {drives.length === 0 ? (
        <EmptyState
          title="No drive pairs configured"
          description="Add a drive pair to start mirroring files"
          action={
            <button
              onClick={() => setShowForm(true)}
              className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              Add Drive Pair
            </button>
          }
        />
      ) : (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          {drives.map((drive) => (
            <DriveCard
              key={drive.id}
              drive={drive}
              onEdit={() => setEditTarget(drive)}
              onDelete={() => setDeleteTarget(drive)}
              onManageReplacement={() => setReplacementTarget(drive)}
            />
          ))}
        </div>
      )}

      {/* Create / Edit form */}
      {(showForm || editTarget) && (
        <DriveForm
          initial={editTarget ?? undefined}
          onClose={() => {
            setShowForm(false)
            setEditTarget(null)
          }}
          onSave={async (data: CreateDrivePairRequest | UpdateDrivePairRequest) => {
            if (editTarget) {
              const updated = await update(editTarget.id, data)
              toast.success(`Drive pair "${updated.name}" updated`)
            } else {
              const created = await create(data as CreateDrivePairRequest)
              toast.success(`Drive pair "${created.name}" created`)
            }
            setShowForm(false)
            setEditTarget(null)
          }}
        />
      )}

      {/* Delete confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
        title={`Delete "${deleteTarget?.name}"?`}
        description="This will permanently remove the drive pair. Tracked files will remain but become unassociated."
        confirmLabel="Delete"
        destructive
        onConfirm={handleDelete}
      />

      {/* Replacement workflow */}
      {replacementTarget && (
        <ReplacementWorkflow
          drive={replacementTarget}
          onClose={() => setReplacementTarget(null)}
          onUpdate={handleReplacementUpdate}
        />
      )}
    </div>
  )
}
