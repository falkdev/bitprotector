import { Trash2, RefreshCw, Link } from 'lucide-react'
import type { TrackedFile } from '@/types/file'

interface FileActionsProps {
  file: TrackedFile
  onMirror: (file: TrackedFile) => void
  onDelete: (file: TrackedFile) => void
  onSetVirtualPath: (file: TrackedFile) => void
}

export function FileActions({ file, onMirror, onDelete, onSetVirtualPath }: FileActionsProps) {
  return (
    <div className="flex items-center gap-1" data-testid={`file-actions-${file.id}`}>
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
        className="rounded p-1 text-muted-foreground hover:bg-green-500/10 hover:text-green-500"
        title="Mirror file"
        onClick={(event) => {
          event.stopPropagation()
          onMirror(file)
        }}
        data-testid="action-mirror"
      >
        <RefreshCw className="h-4 w-4" />
      </button>
      <button
        className="rounded p-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
        title="Delete file"
        onClick={(event) => {
          event.stopPropagation()
          onDelete(file)
        }}
        data-testid="action-delete"
      >
        <Trash2 className="h-4 w-4" />
      </button>
    </div>
  )
}
