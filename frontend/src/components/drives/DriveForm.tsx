import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { X } from 'lucide-react'
import type { DrivePair, CreateDrivePairRequest, UpdateDrivePairRequest } from '@/types/drive'

const schema = z.object({
  name: z.string().min(1, 'Name is required'),
  primary_path: z.string().min(1, 'Primary path is required'),
  secondary_path: z.string().min(1, 'Secondary path is required'),
})

type FormData = z.infer<typeof schema>

interface DriveFormProps {
  initial?: DrivePair
  onClose: () => void
  onSave: (data: CreateDrivePairRequest | UpdateDrivePairRequest) => Promise<void>
}

export function DriveForm({ initial, onClose, onSave }: DriveFormProps) {
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({ resolver: zodResolver(schema) })

  useEffect(() => {
    if (initial) {
      reset({ name: initial.name, primary_path: initial.primary_path, secondary_path: initial.secondary_path })
    }
  }, [initial, reset])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-semibold">{initial ? 'Edit Drive Pair' : 'New Drive Pair'}</h2>
          <button onClick={onClose} className="rounded p-1 hover:bg-accent transition-colors">
            <X className="h-4 w-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit(onSave)} className="space-y-4">
          <Field label="Name" error={errors.name?.message}>
            <input
              {...register('name')}
              className="input"
              placeholder="e.g. Main Mirror"
              data-testid="drive-name-input"
            />
          </Field>
          <Field label="Primary Path" error={errors.primary_path?.message}>
            <input
              {...register('primary_path')}
              className="input"
              placeholder="/mnt/drive-a"
              data-testid="drive-primary-path-input"
            />
          </Field>
          <Field label="Secondary Path" error={errors.secondary_path?.message}>
            <input
              {...register('secondary_path')}
              className="input"
              placeholder="/mnt/drive-b"
              data-testid="drive-secondary-path-input"
            />
          </Field>

          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button type="submit" disabled={isSubmitting} className="btn-primary">
              {isSubmitting ? 'Saving…' : initial ? 'Update' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function Field({
  label,
  error,
  children,
}: {
  label: string
  error?: string
  children: React.ReactNode
}) {
  return (
    <div>
      <label className="mb-1 block text-sm font-medium">{label}</label>
      {children}
      {error && <p className="mt-1 text-xs text-destructive">{error}</p>}
    </div>
  )
}
