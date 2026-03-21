import type { TrackedFile } from '@/types/file'
import { formatBytes, formatDate } from '@/lib/format'
import { FileActions } from './FileActions'
import { cn } from '@/lib/utils'

interface FileRowProps {
  file: TrackedFile
  isSelected: boolean
  onClick: () => void
  onMirror: (file: TrackedFile) => void
  onDelete: (file: TrackedFile) => void
  onSetVirtualPath: (file: TrackedFile) => void
}

export function FileRow({
  file,
  isSelected,
  onClick,
  onMirror,
  onDelete,
  onSetVirtualPath,
}: FileRowProps) {
  const filename = file.relative_path.split('/').pop() ?? file.relative_path

  return (
    <tr
      className={cn(
        'cursor-pointer hover:bg-gray-50',
        isSelected && 'bg-blue-50'
      )}
      onClick={onClick}
      data-testid={`file-row-${file.id}`}
    >
      <td className="px-4 py-2 text-sm font-medium text-gray-900 truncate max-w-xs">
        {filename}
      </td>
      <td className="px-4 py-2 text-sm text-gray-600 font-mono truncate max-w-xs hidden md:table-cell">
        {file.virtual_path ?? <span className="text-gray-400 italic">none</span>}
      </td>
      <td className="px-4 py-2 text-sm text-gray-600 hidden lg:table-cell">
        {file.file_size != null ? formatBytes(file.file_size) : '—'}
      </td>
      <td className="px-4 py-2 text-sm text-gray-600 hidden xl:table-cell">
        {formatDate(file.updated_at)}
      </td>
      <td className="px-4 py-2">
        <div className="flex items-center gap-1 justify-end">
          {file.is_mirrored && (
            <span className="px-1.5 py-0.5 text-xs rounded-full bg-green-100 text-green-700">Mirrored</span>
          )}
        </div>
      </td>
      <td className="px-4 py-2" onClick={(e) => e.stopPropagation()}>
        <FileActions
          file={file}
          onMirror={onMirror}
          onDelete={onDelete}
          onSetVirtualPath={onSetVirtualPath}
        />
      </td>
    </tr>
  )
}
