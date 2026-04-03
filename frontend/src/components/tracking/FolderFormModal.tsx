import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { getActiveDrivePath, resolveAbsolutePathForPicker, resolveTrackedPathInput } from '@/lib/path'
import type { DrivePair } from '@/types/drive'
import type { CreateFolderRequest } from '@/types/folder'

const schema = z.object({
  drive_pair_id: z.coerce.number().min(1, 'Select a drive pair'),
  folder_path: z.string().min(1, 'Folder path is required'),
  virtual_path: z
    .string()
    .optional()
    .refine((value) => !value || value.trim().startsWith('/'), 'Virtual path must be absolute'),
})

type FormData = z.infer<typeof schema>

export function FolderFormModal({
  drives,
  onClose,
  onSave,
}: {
  drives: DrivePair[]
  onClose: () => void
  onSave: (data: CreateFolderRequest) => Promise<void>
}) {
  const {
    register,
    handleSubmit,
    setError,
    clearErrors,
    setValue,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    resolver: zodResolver(schema) as never,
  })
  const [showPicker, setShowPicker] = useState(false)
  const [showVirtualPicker, setShowVirtualPicker] = useState(false)
  const drivePairId = watch('drive_pair_id')
  const rawPath = watch('folder_path') ?? ''
  const rawVirtualPath = watch('virtual_path') ?? ''
  const selectedDrive = drives.find((drive) => drive.id === Number(drivePairId))
  const activeRoot = selectedDrive
    ? getActiveDrivePath(
        selectedDrive.primary_path,
        selectedDrive.secondary_path,
        selectedDrive.active_role
      )
    : null
  const pathResolution = resolveTrackedPathInput(activeRoot, rawPath)

  return (
    <>
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
        <div className="w-full max-w-2xl rounded-xl border border-border bg-card p-6 shadow-lg">
          <h2 className="mb-4 font-semibold">Add Tracked Folder</h2>
          <form
            onSubmit={handleSubmit(async (data) => {
              const resolution = resolveTrackedPathInput(activeRoot, data.folder_path)
              if (resolution.error || !resolution.relativePath) {
                setError('folder_path', {
                  type: 'manual',
                  message: resolution.error ?? 'Folder path is required',
                })
                return
              }

              clearErrors('folder_path')
              await onSave({
                drive_pair_id: Number(data.drive_pair_id),
                folder_path: resolution.relativePath,
                virtual_path: data.virtual_path?.trim() || undefined,
              })
            })}
            className="space-y-4"
          >
            <div>
              <label htmlFor="folder-drive-pair" className="mb-1 block text-sm font-medium">
                Drive Pair
              </label>
              <select
                id="folder-drive-pair"
                {...register('drive_pair_id')}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              >
                <option value="">Select...</option>
                {drives.map((drive) => (
                  <option key={drive.id} value={drive.id}>
                    {drive.name}
                  </option>
                ))}
              </select>
              {errors.drive_pair_id ? <p className="mt-1 text-xs text-destructive">{errors.drive_pair_id.message}</p> : null}
            </div>
            <div>
              <label htmlFor="folder-path" className="mb-1 block text-sm font-medium">
                Folder Path
              </label>
              <div className="flex gap-2">
                <input
                  id="folder-path"
                  {...register('folder_path')}
                  placeholder="documents or /mnt/drive-a/documents"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                />
                <button
                  type="button"
                  onClick={() => setShowPicker(true)}
                  disabled={!selectedDrive}
                  className="rounded-md border border-border px-3 py-2 text-sm transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
                >
                  Browse
                </button>
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                {selectedDrive
                  ? `Active root: ${activeRoot}`
                  : 'Select a drive pair before browsing or submitting.'}
              </p>
              {selectedDrive && rawPath.trim() && !pathResolution.error && pathResolution.relativePath ? (
                <p className="mt-1 text-xs text-muted-foreground">
                  Will be stored as <span className="font-mono">{pathResolution.relativePath}</span>
                </p>
              ) : null}
              {errors.folder_path ? <p className="mt-1 text-xs text-destructive">{errors.folder_path.message}</p> : null}
            </div>
            <div>
              <label htmlFor="folder-virtual-path" className="mb-1 block text-sm font-medium">
                Virtual Path (optional)
              </label>
              <div className="flex gap-2">
                <input
                  id="folder-virtual-path"
                  {...register('virtual_path')}
                  placeholder="/docs"
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                />
                <button
                  type="button"
                  onClick={() => setShowVirtualPicker(true)}
                  className="rounded-md border border-border px-3 py-2 text-sm transition-colors hover:bg-accent"
                >
                  Browse
                </button>
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                If set, BitProtector will create a symlink exactly at this path to the tracked folder.
              </p>
              {errors.virtual_path ? <p className="mt-1 text-xs text-destructive">{errors.virtual_path.message}</p> : null}
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="rounded-md border border-border px-4 py-2 text-sm transition-colors hover:bg-accent"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={isSubmitting}
                className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-60"
              >
                {isSubmitting ? 'Adding...' : 'Add Folder'}
              </button>
            </div>
          </form>
        </div>
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select Folder Path"
        description="Browse the BitProtector host filesystem and choose a directory under the selected drive pair’s active root."
        mode="directory"
        value={rawPath}
        startPath={resolveAbsolutePathForPicker(activeRoot, rawPath)}
        confirmLabel="Use Folder Path"
        validatePath={(path) => resolveTrackedPathInput(activeRoot, path).error}
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue('folder_path', path, { shouldDirty: true, shouldValidate: true })
          clearErrors('folder_path')
          setShowPicker(false)
        }}
      />
      <PathPickerDialog
        open={showVirtualPicker}
        title="Select Folder Virtual Path"
        description="Choose the absolute virtual path for this tracked folder."
        mode="directory"
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
