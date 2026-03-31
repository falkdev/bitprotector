import { useState, useEffect, useCallback } from 'react'
import { toast } from 'sonner'
import { Plus } from 'lucide-react'
import { drivesApi } from '@/api/drives'
import { filesApi } from '@/api/files'
import { virtualPathsApi } from '@/api/virtual-paths'
import { useFilesStore } from '@/stores/files-store'
import { FileTree } from '@/components/file-browser/FileTree'
import { FileGrid } from '@/components/file-browser/FileGrid'
import { FileDetails } from '@/components/file-browser/FileDetails'
import { BreadcrumbNav } from '@/components/file-browser/BreadcrumbNav'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { getActiveDrivePath, resolveAbsolutePathForPicker, resolveTrackedPathInput } from '@/lib/path'
import type { TrackedFile, TrackFileRequest } from '@/types/file'
import type { DrivePair } from '@/types/drive'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'

const trackSchema = z.object({
  drive_pair_id: z.coerce.number().int().positive('Drive pair ID is required'),
  relative_path: z.string().min(1, 'Path is required'),
})

const vPathSchema = z.object({
  virtual_path: z.string().min(1, 'Virtual path is required'),
})

type TrackFormData = z.infer<typeof trackSchema>
type VPathFormData = z.infer<typeof vPathSchema>

export function TrackFileModal({
  open,
  onClose,
  onTrack,
  drives,
}: {
  open: boolean
  onClose: () => void
  onTrack: (data: TrackFileRequest) => Promise<void>
  drives: DrivePair[]
}) {
  const {
    register,
    handleSubmit,
    reset,
    setError,
    clearErrors,
    setValue,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<TrackFormData>({ resolver: zodResolver(trackSchema) as never })
  const [showPicker, setShowPicker] = useState(false)
  const drivePairId = watch('drive_pair_id')
  const rawPath = watch('relative_path') ?? ''
  const selectedDrive = drives.find((drive) => drive.id === Number(drivePairId))
  const activeRoot = selectedDrive
    ? getActiveDrivePath(
        selectedDrive.primary_path,
        selectedDrive.secondary_path,
        selectedDrive.active_role
      )
    : null
  const pathResolution = resolveTrackedPathInput(activeRoot, rawPath)

  if (!open) return null

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
        <div className="bg-white rounded-lg shadow-xl w-full max-w-2xl p-6">
        <h2 className="text-lg font-semibold mb-4">Track new file</h2>
        <form
          onSubmit={handleSubmit(async (d) => {
            const resolution = resolveTrackedPathInput(activeRoot, d.relative_path)
            if (resolution.error || !resolution.relativePath) {
              setError('relative_path', {
                type: 'manual',
                message: resolution.error ?? 'Path is required',
              })
              return
            }

            clearErrors('relative_path')
            await onTrack({
              drive_pair_id: Number(d.drive_pair_id),
              relative_path: resolution.relativePath,
            })
            reset()
            onClose()
          })}
          className="space-y-4"
        >
          <div>
            <label htmlFor="track-file-drive-pair" className="block text-sm font-medium text-gray-700 mb-1">Drive pair</label>
            <select
              id="track-file-drive-pair"
              {...register('drive_pair_id')}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
            >
              <option value="">Select…</option>
              {drives.map((drive) => (
                <option key={drive.id} value={drive.id}>
                  {drive.name}
                </option>
              ))}
            </select>
            {errors.drive_pair_id && <p className="mt-1 text-xs text-red-500">{errors.drive_pair_id.message}</p>}
          </div>
          <div>
            <label htmlFor="track-file-path" className="block text-sm font-medium text-gray-700 mb-1">File path</label>
            <div className="flex gap-2">
              <input
                id="track-file-path"
                type="text"
                {...register('relative_path')}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
                placeholder="docs/report.pdf or /mnt/drive-a/docs/report.pdf"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                disabled={!selectedDrive}
                className="whitespace-nowrap rounded-md border border-gray-300 px-3 py-2 text-sm font-medium hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
              >
                Browse
              </button>
            </div>
            <p className="mt-1 text-xs text-gray-500">
              {selectedDrive
                ? `Active root: ${activeRoot}`
                : 'Select a drive pair before browsing or submitting.'}
            </p>
            {selectedDrive && rawPath.trim() && !pathResolution.error && pathResolution.relativePath ? (
              <p className="mt-1 text-xs text-gray-500">
                Will be stored as <span className="font-mono">{pathResolution.relativePath}</span>
              </p>
            ) : null}
            {errors.relative_path && <p className="mt-1 text-xs text-red-500">{errors.relative_path.message}</p>}
          </div>
          <div className="flex gap-2 justify-end pt-2">
            <button
              type="button"
              onClick={() => { reset(); onClose() }}
              className="px-4 py-2 rounded-md border border-gray-300 text-sm font-medium hover:bg-gray-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="px-4 py-2 rounded-md bg-blue-600 text-white text-sm font-medium hover:bg-blue-700 disabled:opacity-50"
            >
              {isSubmitting ? 'Tracking…' : 'Track file'}
            </button>
          </div>
        </form>
      </div>
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select File Path"
        description="Browse the BitProtector host filesystem and choose a file under the selected drive pair’s active root."
        mode="file"
        value={rawPath}
        startPath={resolveAbsolutePathForPicker(activeRoot, rawPath)}
        confirmLabel="Use File Path"
        validatePath={(path) => resolveTrackedPathInput(activeRoot, path).error}
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue('relative_path', path, { shouldDirty: true, shouldValidate: true })
          clearErrors('relative_path')
          setShowPicker(false)
        }}
      />
    </>
  )
}

function VirtualPathModal({
  file,
  onClose,
  onSave,
}: {
  file: TrackedFile | null
  onClose: () => void
  onSave: (fileId: number, vpath: string) => Promise<void>
}) {
  const { register, handleSubmit, reset, setValue, formState: { errors, isSubmitting } } =
    useForm<VPathFormData>({ resolver: zodResolver(vPathSchema) as never })

  useEffect(() => {
    if (file) setValue('virtual_path', file.virtual_path ?? '')
  }, [file, setValue])

  if (!file) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="bg-white rounded-lg shadow-xl w-full max-w-md p-6">
        <h2 className="text-lg font-semibold mb-1">Set virtual path</h2>
        <p className="text-sm text-gray-500 mb-4 font-mono truncate">{file.relative_path}</p>
        <form
          onSubmit={handleSubmit(async (d) => {
            await onSave(file.id, d.virtual_path)
            reset()
            onClose()
          })}
          className="space-y-4"
        >
          <div>
            <label htmlFor="file-virtual-path" className="block text-sm font-medium text-gray-700 mb-1">Virtual path</label>
            <input
              id="file-virtual-path"
              type="text"
              {...register('virtual_path')}
              className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="photos/2024/vacation"
            />
            {errors.virtual_path && <p className="mt-1 text-xs text-red-500">{errors.virtual_path.message}</p>}
          </div>
          <div className="flex gap-2 justify-end pt-2">
            <button
              type="button"
              onClick={() => { reset(); onClose() }}
              className="px-4 py-2 rounded-md border border-gray-300 text-sm font-medium hover:bg-gray-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="px-4 py-2 rounded-md bg-blue-600 text-white text-sm font-medium hover:bg-blue-700 disabled:opacity-50"
            >
              {isSubmitting ? 'Saving…' : 'Save'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

export function FileBrowserPage() {
  const { response, fetch, setParams, params } = useFilesStore()
  const [selectedFile, setSelectedFile] = useState<TrackedFile | null>(null)
  const [treePrefix, setTreePrefix] = useState('')
  const [showTrack, setShowTrack] = useState(false)
  const [vpathFile, setVpathFile] = useState<TrackedFile | null>(null)
  const [deleteFile, setDeleteFile] = useState<TrackedFile | null>(null)
  const [loading, setLoading] = useState(false)
  const [drives, setDrives] = useState<DrivePair[]>([])

  const loadFiles = useCallback(async (nextParams: typeof params) => {
    setLoading(true)
    try {
      await fetch(nextParams)
    } finally {
      setLoading(false)
    }
  }, [fetch])

  useEffect(() => {
    void loadFiles(params)
  }, [loadFiles, params])

  useEffect(() => {
    let active = true

    const loadDrives = async () => {
      try {
        const nextDrives = await drivesApi.list()
        if (active) {
          setDrives(nextDrives)
        }
      } catch {
        toast.error('Failed to load drive pairs')
      }
    }

    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  const allVirtualPaths = (response?.files ?? [])
    .map((f) => f.virtual_path)
    .filter((p): p is string => !!p)

  const handleTreeSelect = (path: string) => {
    const nextParams = { ...params, virtual_prefix: path || undefined, page: 1 }
    setTreePrefix(path)
    setParams(nextParams)
    setSelectedFile(null)
  }

  const handlePageChange = (page: number) => {
    const nextParams = { ...params, page }
    setParams(nextParams)
  }

  const handleTrack = async (data: TrackFileRequest) => {
    try {
      await filesApi.track(data)
      toast.success('File tracked')
      await loadFiles(params)
    } catch {
      toast.error('Failed to track file')
    }
  }

  const handleMirror = async (file: TrackedFile) => {
    try {
      await filesApi.mirror(file.id)
      toast.success('Mirror requested')
      await loadFiles(params)
    } catch {
      toast.error('Mirror failed')
    }
  }

  const handleDelete = async () => {
    if (!deleteFile) return
    try {
      await filesApi.delete(deleteFile.id)
      toast.success('File removed from tracking')
      setDeleteFile(null)
      if (selectedFile?.id === deleteFile.id) setSelectedFile(null)
      await loadFiles(params)
    } catch {
      toast.error('Failed to delete file')
    }
  }

  const handleSetVirtualPath = async (fileId: number, vpath: string) => {
    try {
      await virtualPathsApi.set(fileId, { virtual_path: vpath })
      toast.success('Virtual path updated')
      await loadFiles(params)
    } catch {
      toast.error('Failed to set virtual path')
    }
  }

  const files = response?.files ?? []
  const total = response?.total ?? 0
  const page = response?.page ?? 1
  const perPage = response?.per_page ?? 50

  return (
    <div className="flex h-full gap-0" data-testid="file-browser-page">
      {/* Sidebar tree */}
      <aside className="w-56 shrink-0 border-r border-gray-200 overflow-auto bg-white">
        <div className="p-3 border-b">
          <h2 className="text-xs font-semibold text-gray-500 uppercase tracking-wide">Virtual paths</h2>
        </div>
        <FileTree
          virtualPaths={allVirtualPaths}
          selected={treePrefix}
          onSelect={handleTreeSelect}
        />
      </aside>

      {/* Main content */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="flex items-center justify-between px-4 py-3 border-b bg-white">
          <BreadcrumbNav path={treePrefix} onNavigate={handleTreeSelect} />
          <button
            className="flex items-center gap-1.5 px-3 py-1.5 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700"
            onClick={() => setShowTrack(true)}
            data-testid="track-file-btn"
          >
            <Plus className="h-4 w-4" /> Track file
          </button>
        </div>
        <div className="flex-1 overflow-auto p-4">
          <FileGrid
            files={files}
            total={total}
            page={page}
            perPage={perPage}
            loading={loading}
            selectedFileId={selectedFile?.id ?? null}
            onSelectFile={setSelectedFile}
            onPageChange={handlePageChange}
            onMirror={handleMirror}
            onDelete={setDeleteFile}
            onSetVirtualPath={setVpathFile}
          />
        </div>
      </div>

      {/* Details panel */}
      {selectedFile && (
        <aside className="w-72 shrink-0 border-l border-gray-200 bg-white overflow-auto">
          <FileDetails file={selectedFile} onClose={() => setSelectedFile(null)} />
        </aside>
      )}

      {/* Modals */}
      <TrackFileModal
        open={showTrack}
        onClose={() => setShowTrack(false)}
        onTrack={handleTrack}
        drives={drives}
      />
      <VirtualPathModal
        file={vpathFile}
        onClose={() => setVpathFile(null)}
        onSave={handleSetVirtualPath}
      />
      <ConfirmDialog
        open={!!deleteFile}
        onOpenChange={(open) => {
          if (!open) setDeleteFile(null)
        }}
        title="Remove file from tracking"
        description={`Remove "${deleteFile?.relative_path}" from tracking? The actual file will not be deleted.`}
        destructive
        onConfirm={handleDelete}
      />
    </div>
  )
}
