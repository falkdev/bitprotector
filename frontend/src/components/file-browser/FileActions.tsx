import { Trash2, RefreshCw, Link } from 'lucide-react'
import type { TrackedFile } from '@/types/file'

interface FileActionsProps {
  file: TrackedFile
  onMirror: (file: TrackedFile) => void
  onDelete: (file: TrackedFile) => void
  onSetVirtualPath: (file: TrackedFile) => void
  mirrorDisabled?: boolean
  mirroring?: boolean
  deleteDisabled?: boolean
  deleting?: boolean
}

export function FileActions({
  file,
  onMirror,
  onDelete,
  onSetVirtualPath,
  mirrorDisabled = false,
  mirroring = false,
  deleteDisabled = false,
  deleting = false,
}: FileActionsProps) {
  return (
    <div
      className="flex w-full items-center justify-end gap-1"
      data-testid={`file-actions-${file.id}`}
    >
      <button
        className="rounded p-1 text-muted-foreground hover:bg-primary/10 hover:text-primary"
        title="Set virtual path"
        onClick={(event) => {
          event.stopPropagation()
          onSetVirtualPath(file)
        }}
        data-testid="action-set-virtual-path"
      >
        <Link className="h-4 w-4" />
      </button>
      <button
        className="rounded p-1 text-muted-foreground hover:bg-green-500/10 hover:text-green-500 disabled:cursor-not-allowed disabled:opacity-50"
        title="Mirror file"
        disabled={mirrorDisabled}
        onClick={(event) => {
          event.stopPropagation()
          if (mirrorDisabled) return
          onMirror(file)
        }}
        data-testid="action-mirror"
      >
        <RefreshCw className={`h-4 w-4${mirroring ? ' animate-spin' : ''}`} />
      </button>
      <button
        className="rounded p-1 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:cursor-not-allowed disabled:opacity-50"
        title="Delete file"
        disabled={deleteDisabled}
        onClick={(event) => {
          event.stopPropagation()
          if (deleteDisabled) return
          onDelete(file)
        }}
        data-testid="action-delete"
      >
        {deleting ? <RefreshCw className="h-4 w-4 animate-spin" /> : <Trash2 className="h-4 w-4" />}
      </button>
    </div>
  )
}
