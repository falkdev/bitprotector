import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { resolveAbsolutePathForPicker, resolveTrackedPathInput } from '@/lib/path'
import type { DrivePair } from '@/types/drive'
import type { TrackFileRequest } from '@/types/file'

const trackSchema = z.object({
  drive_pair_id: z.coerce.number().int().positive('Drive pair ID is required'),
  relative_path: z.string().min(1, 'Path is required'),
  virtual_path: z
    .string()
    .optional()
    .refine((value) => !value || value.trim().startsWith('/'), 'Virtual path must be absolute'),
})

type TrackFormData = z.infer<typeof trackSchema>

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
  const [showVirtualPicker, setShowVirtualPicker] = useState(false)
  const drivePairId = watch('drive_pair_id')
  const rawPath = watch('relative_path') ?? ''
  const rawVirtualPath = watch('virtual_path') ?? ''
  const selectedDrive = drives.find((drive) => drive.id === Number(drivePairId))
  const primaryRoot = selectedDrive?.primary_path ?? null
  const pathResolution = resolveTrackedPathInput(primaryRoot, rawPath)

  if (!open) return null

  return (
    <>
      <ModalLayer>
        <div className="w-full max-w-2xl rounded-lg bg-white p-6 shadow-xl">
          <h2 className="mb-4 text-lg font-semibold">Track new file</h2>
          <form
            onSubmit={handleSubmit(async (data) => {
              const resolution = resolveTrackedPathInput(primaryRoot, data.relative_path)
              if (resolution.error || !resolution.relativePath) {
                setError('relative_path', {
                  type: 'manual',
                  message: resolution.error ?? 'Path is required',
                })
                return
              }

              clearErrors('relative_path')
              const payload: TrackFileRequest = {
                drive_pair_id: Number(data.drive_pair_id),
                relative_path: resolution.relativePath,
              }
              const virtualPath = data.virtual_path?.trim()
              if (virtualPath) {
                payload.virtual_path = virtualPath
              }
              await onTrack(payload)
              reset()
              onClose()
            })}
            className="space-y-4"
          >
            <div>
              <label htmlFor="track-file-drive-pair" className="mb-1 block text-sm font-medium text-gray-700">
                Drive pair
              </label>
              <select
                id="track-file-drive-pair"
                {...register('drive_pair_id')}
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="">Select...</option>
                {drives.map((drive) => (
                  <option key={drive.id} value={drive.id}>
                    {drive.name}
                  </option>
                ))}
              </select>
              {errors.drive_pair_id ? <p className="mt-1 text-xs text-red-500">{errors.drive_pair_id.message}</p> : null}
            </div>
            <div>
              <label htmlFor="track-file-path" className="mb-1 block text-sm font-medium text-gray-700">
                File path
              </label>
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
                  ? `Primary root: ${primaryRoot}`
                  : 'Select a drive pair before browsing or submitting.'}
              </p>
              {selectedDrive && rawPath.trim() && !pathResolution.error && pathResolution.relativePath ? (
                <p className="mt-1 text-xs text-gray-500">
                  Will be stored as <span className="font-mono">{pathResolution.relativePath}</span>
                </p>
              ) : null}
              {errors.relative_path ? <p className="mt-1 text-xs text-red-500">{errors.relative_path.message}</p> : null}
            </div>
            <div>
              <label htmlFor="track-file-virtual-path" className="mb-1 block text-sm font-medium text-gray-700">
                Virtual Path (optional)
              </label>
              <div className="flex gap-2">
                <input
                  id="track-file-virtual-path"
                  type="text"
                  {...register('virtual_path')}
                  className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-blue-500"
                  placeholder="/docs/report.pdf"
                />
                <button
                  type="button"
                  onClick={() => setShowVirtualPicker(true)}
                  className="whitespace-nowrap rounded-md border border-gray-300 px-3 py-2 text-sm font-medium hover:bg-gray-50"
                >
                  Browse
                </button>
              </div>
              <p className="mt-1 text-xs text-gray-500">
                If set, BitProtector will create a symlink exactly at this path to the tracked file.
              </p>
              {errors.virtual_path ? <p className="mt-1 text-xs text-red-500">{errors.virtual_path.message}</p> : null}
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => {
                  reset()
                  onClose()
                }}
                className="rounded-md border border-gray-300 px-4 py-2 text-sm font-medium hover:bg-gray-50"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting}
                className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
              >
                {isSubmitting ? 'Tracking...' : 'Track file'}
              </button>
            </div>
          </form>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={showPicker}
        title="Select File Path"
        description="Browse the BitProtector host filesystem and choose a file under the selected drive pair’s primary root."
        mode="file"
        value={rawPath}
        startPath={resolveAbsolutePathForPicker(primaryRoot, rawPath)}
        rootPath={primaryRoot ?? undefined}
        confirmLabel="Use File Path"
        validatePath={(path) => resolveTrackedPathInput(primaryRoot, path).error}
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue('relative_path', path, { shouldDirty: true, shouldValidate: true })
          clearErrors('relative_path')
          setShowPicker(false)
        }}
      />
      <PathPickerDialog
        open={showVirtualPicker}
        title="Select File Virtual Path"
        description="Choose the absolute virtual path for this tracked file."
        mode="file"
        value={rawVirtualPath}
        startPath={rawVirtualPath || '/'}
        confirmLabel="Use Virtual Path"
        onClose={() => setShowVirtualPicker(false)}
        onPick={(path) => {
          setValue('virtual_path', path, { shouldDirty: true, shouldValidate: true })
          clearErrors('virtual_path')
          setShowVirtualPicker(false)
        }}
      />
    </>
  )
}
