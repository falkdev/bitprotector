import { X } from 'lucide-react'
import type { TrackedFile } from '@/types/file'
import { formatBytes, formatDate } from '@/lib/format'

interface FileDetailsProps {
  file: TrackedFile
  drivePairName?: string
  onClose: () => void
}

export function FileDetails({ file, drivePairName, onClose }: FileDetailsProps) {
  const filename = file.relative_path.split('/').pop() ?? file.relative_path
  return (
    <div className="flex flex-col h-full" data-testid="file-details">
      <div className="flex items-center justify-between border-b px-4 py-3">
        <h3 className="font-medium text-gray-900 truncate">{filename}</h3>
        <button
          className="rounded p-1 text-gray-500 hover:bg-gray-100"
          onClick={onClose}
          data-testid="close-details"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="flex-1 overflow-auto p-4 space-y-4">
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">
            Relative path
          </p>
          <p className="text-sm text-gray-900 break-all font-mono">{file.relative_path}</p>
        </div>
        {file.virtual_path && (
          <div>
            <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">
              Virtual path
            </p>
            <p className="text-sm text-gray-900 break-all font-mono">{file.virtual_path}</p>
          </div>
        )}
        {file.file_size != null && (
          <div>
            <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">Size</p>
            <p className="text-sm text-gray-900">{formatBytes(file.file_size)}</p>
          </div>
        )}
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">
            Drive pair
          </p>
          <p className="text-sm text-gray-900">{drivePairName ?? `ID ${file.drive_pair_id}`}</p>
        </div>
        {file.checksum && (
          <div>
            <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">
              Checksum (BLAKE3)
            </p>
            <p className="text-sm font-mono text-gray-900 break-all">{file.checksum}</p>
          </div>
        )}
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">Mirrored</p>
          <p className="text-sm text-gray-900">{file.is_mirrored ? 'Yes' : 'No'}</p>
        </div>
        {file.last_integrity_check_at && (
          <div>
            <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">
              Last integrity check
            </p>
            <p className="text-sm text-gray-900">{formatDate(file.last_integrity_check_at)}</p>
          </div>
        )}
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">Added</p>
          <p className="text-sm text-gray-900">{formatDate(file.created_at)}</p>
        </div>
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wide mb-1">Updated</p>
          <p className="text-sm text-gray-900">{formatDate(file.updated_at)}</p>
        </div>
      </div>
    </div>
  )
}
