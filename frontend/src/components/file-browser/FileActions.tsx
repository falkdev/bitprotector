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
        className="rounded p-1 text-gray-500 hover:bg-blue-50 hover:text-blue-600"
        title="Set publish path"
        onClick={() => onSetVirtualPath(file)}
        data-testid="action-set-virtual-path"
      >
        <Link className="h-4 w-4" />
      </button>
      <button
        className="rounded p-1 text-gray-500 hover:bg-green-50 hover:text-green-600"
        title="Mirror file"
        onClick={() => onMirror(file)}
        data-testid="action-mirror"
      >
        <RefreshCw className="h-4 w-4" />
      </button>
      <button
        className="rounded p-1 text-gray-500 hover:bg-red-50 hover:text-red-600"
        title="Delete file"
        onClick={() => onDelete(file)}
        data-testid="action-delete"
      >
        <Trash2 className="h-4 w-4" />
      </button>
    </div>
  )
}
