import type { DrivePair, DriveState } from '@/types/drive'
import { HardDrive, Edit, Trash2, Wrench } from 'lucide-react'
import { cn } from '@/lib/utils'
import { formatDate } from '@/lib/format'

const stateColors: Record<DriveState, string> = {
  active: 'bg-green-100 text-green-700',
  quiescing: 'bg-yellow-100 text-yellow-700',
  failed: 'bg-red-100 text-red-700',
  rebuilding: 'bg-blue-100 text-blue-700',
}

interface DriveCardProps {
  drive: DrivePair
  onEdit: () => void
  onDelete: () => void
  onManageReplacement: () => void
}

export function DriveCard({ drive, onEdit, onDelete, onManageReplacement }: DriveCardProps) {
  const isHealthy = drive.primary_state === 'active' && drive.secondary_state === 'active'

  return (
    <div
      className={cn(
        'rounded-lg border bg-card p-4 flex flex-col gap-3',
        isHealthy ? 'border-border' : 'border-orange-300'
      )}
      data-testid={`drive-card-${drive.id}`}
    >
      {/* Header */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          <HardDrive className="h-4 w-4 text-muted-foreground" />
          <h3 className="font-medium text-sm">{drive.name}</h3>
        </div>
        <span className="text-xs text-muted-foreground">{drive.active_role} active</span>
      </div>

      {/* Paths */}
      <div className="space-y-1 text-xs text-muted-foreground font-mono">
        <div className="flex items-center gap-2">
          <span className="w-16 flex-shrink-0 text-foreground font-sans font-medium">ID</span>
          <span className="truncate">{drive.id}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-16 flex-shrink-0 text-foreground font-sans font-medium">Primary</span>
          <span className="truncate">{drive.primary_path}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-16 flex-shrink-0 text-foreground font-sans font-medium">
            Secondary
          </span>
          <span className="truncate">{drive.secondary_path}</span>
        </div>
      </div>

      {/* State badges */}
      <div className="flex gap-2">
        <span
          className={cn(
            'rounded-full px-2 py-0.5 text-xs font-medium',
            stateColors[drive.primary_state]
          )}
        >
          P: {drive.primary_state}
        </span>
        <span
          className={cn(
            'rounded-full px-2 py-0.5 text-xs font-medium',
            stateColors[drive.secondary_state]
          )}
        >
          S: {drive.secondary_state}
        </span>
      </div>

      {/* Updated at */}
      <p className="text-xs text-muted-foreground">Updated {formatDate(drive.updated_at)}</p>

      {/* Actions */}
      <div className="flex items-center gap-2 border-t border-border pt-3">
        <button
          onClick={onEdit}
          className="flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-accent transition-colors"
          data-testid={`edit-drive-${drive.id}`}
        >
          <Edit className="h-3 w-3" /> Edit
        </button>
        <button
          onClick={onManageReplacement}
          className="flex items-center gap-1 rounded px-2 py-1 text-xs hover:bg-accent transition-colors"
          data-testid={`replace-drive-${drive.id}`}
        >
          <Wrench className="h-3 w-3" /> Replace
        </button>
        <button
          onClick={onDelete}
          className="ml-auto flex items-center gap-1 rounded px-2 py-1 text-xs text-destructive hover:bg-destructive/10 transition-colors"
          data-testid={`delete-drive-${drive.id}`}
        >
          <Trash2 className="h-3 w-3" /> Delete
        </button>
      </div>
    </div>
  )
}
