import type { TrackedFile } from '@/types/file'
import { FileRow } from './FileRow'
import { Pagination } from '@/components/shared/Pagination'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { FileText } from 'lucide-react'

interface FileGridProps {
  files: TrackedFile[]
  total: number
  page: number
  perPage: number
  loading: boolean
  selectedFileId: number | null
  onSelectFile: (file: TrackedFile) => void
  onPageChange: (page: number) => void
  onMirror: (file: TrackedFile) => void
  onDelete: (file: TrackedFile) => void
  onSetVirtualPath: (file: TrackedFile) => void
}

export function FileGrid({
  files,
  total,
  page,
  perPage,
  loading,
  selectedFileId,
  onSelectFile,
  onPageChange,
  onMirror,
  onDelete,
  onSetVirtualPath,
}: FileGridProps) {
  if (loading) {
    return (
      <div className="flex items-center justify-center py-16">
        <LoadingSpinner size="lg" />
      </div>
    )
  }

  if (files.length === 0) {
    return (
      <EmptyState
        icon={<FileText className="h-8 w-8 text-gray-400" />}
        title="No files found"
        description="No files match the current filter."
      />
    )
  }

  return (
    <div className="flex flex-col gap-2" data-testid="file-grid">
      <div className="overflow-x-auto rounded-lg border border-gray-200">
        <table className="min-w-full divide-y divide-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide">Name</th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide hidden md:table-cell">Virtual path</th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide hidden lg:table-cell">Size</th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide hidden xl:table-cell">Updated</th>
              <th className="px-4 py-2 text-right text-xs font-medium text-gray-500 uppercase tracking-wide">Status</th>
              <th className="px-4 py-2 text-right text-xs font-medium text-gray-500 uppercase tracking-wide">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 bg-white">
            {files.map((file) => (
              <FileRow
                key={file.id}
                file={file}
                isSelected={selectedFileId === file.id}
                onClick={() => onSelectFile(file)}
                onMirror={onMirror}
                onDelete={onDelete}
                onSetVirtualPath={onSetVirtualPath}
              />
            ))}
          </tbody>
        </table>
      </div>
      <Pagination
        page={page}
        perPage={perPage}
        total={total}
        onPageChange={onPageChange}
      />
    </div>
  )
}
