import { useEffect, useState } from 'react'
import axios from 'axios'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { X } from 'lucide-react'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import type { DrivePair, CreateDrivePairRequest, UpdateDrivePairRequest } from '@/types/drive'
import type { ApiError } from '@/types/api'

const schema = z.object({
  name: z.string().min(1, 'Name is required'),
  primary_path: z.string().min(1, 'Primary path is required'),
  secondary_path: z.string().min(1, 'Secondary path is required'),
  skip_validation: z.boolean().default(false),
})

type FormData = z.infer<typeof schema>

interface DriveFormProps {
  initial?: DrivePair
  onClose: () => void
  onSave: (data: CreateDrivePairRequest | UpdateDrivePairRequest) => Promise<void>
}

function getSaveErrorMessage(error: unknown, fallback: string): string {
  if (axios.isAxiosError(error)) {
    const apiMessage = (error.response?.data as ApiError | undefined)?.error?.message
    if (typeof apiMessage === 'string' && apiMessage.trim()) {
      return apiMessage
    }

    if (typeof error.message === 'string' && error.message.trim()) {
      return error.message
    }
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  return fallback
}

export function DriveForm({ initial, onClose, onSave }: DriveFormProps) {
  const {
    register,
    handleSubmit,
    reset,
    setValue,
    watch,
    formState: { errors, isSubmitting },
  } = useForm<FormData>({
    resolver: zodResolver(schema) as never,
    defaultValues: {
      name: '',
      primary_path: '',
      secondary_path: '',
      skip_validation: false,
    },
  })
  const [pickerField, setPickerField] = useState<'primary_path' | 'secondary_path' | null>(null)
  const [submitError, setSubmitError] = useState<string | null>(null)

  useEffect(() => {
    if (initial) {
      reset({
        name: initial.name,
        primary_path: initial.primary_path,
        secondary_path: initial.secondary_path,
        skip_validation: false,
      })
    }
  }, [initial, reset])

  const primaryPath = watch('primary_path')
  const secondaryPath = watch('secondary_path')
  const skipValidation = watch('skip_validation')

  const submitForm = handleSubmit(async (data) => {
    setSubmitError(null)

    try {
      if (initial) {
        await onSave({
          name: data.name,
          primary_path: data.primary_path,
          secondary_path: data.secondary_path,
        })
        return
      }

      await onSave({
        name: data.name,
        primary_path: data.primary_path,
        secondary_path: data.secondary_path,
        skip_validation: data.skip_validation,
      })
    } catch (error) {
      setSubmitError(
        getSaveErrorMessage(
          error,
          initial ? 'Failed to update drive pair' : 'Failed to create drive pair'
        )
      )
    }
  })

  return (
    <>
      <ModalLayer className="px-4">
        <div className="w-full max-w-lg rounded-xl border border-border bg-card p-6 shadow-lg">
          <div className="mb-4 flex items-center justify-between">
            <h2 className="font-semibold">{initial ? 'Edit Drive Pair' : 'New Drive Pair'}</h2>
            <button type="button" onClick={onClose} className="rounded p-1 hover:bg-accent transition-colors">
              <X className="h-4 w-4" />
            </button>
          </div>

          <form onSubmit={submitForm} className="space-y-4">
            <Field label="Name" error={errors.name?.message}>
              <input
                {...register('name')}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                placeholder="e.g. Main Mirror"
                data-testid="drive-name-input"
              />
            </Field>
            <Field label="Primary Path" error={errors.primary_path?.message}>
              <div className="flex gap-2">
                <input
                  {...register('primary_path')}
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                  placeholder="/mnt/drive-a"
                  data-testid="drive-primary-path-input"
                />
                <button
                  type="button"
                  onClick={() => setPickerField('primary_path')}
                  className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent transition-colors"
                >
                  Browse
                </button>
              </div>
            </Field>
            <Field label="Secondary Path" error={errors.secondary_path?.message}>
              <div className="flex gap-2">
                <input
                  {...register('secondary_path')}
                  className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
                  placeholder="/mnt/drive-b"
                  data-testid="drive-secondary-path-input"
                />
                <button
                  type="button"
                  onClick={() => setPickerField('secondary_path')}
                  className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent transition-colors"
                >
                  Browse
                </button>
              </div>
            </Field>

            {!initial ? (
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input type="checkbox" {...register('skip_validation')} checked={skipValidation} />
                Skip path validation when creating this drive pair
              </label>
            ) : null}

            <div className="flex justify-end gap-2 pt-2">
              <div className="mr-auto max-w-xs self-center text-xs text-destructive">
                {submitError}
              </div>
              <button type="button" onClick={onClose} className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent transition-colors">
                Cancel
              </button>
              <button type="submit" disabled={isSubmitting} className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-60">
                {isSubmitting ? 'Saving…' : initial ? 'Update' : 'Create'}
              </button>
            </div>
          </form>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={pickerField !== null}
        title={pickerField === 'primary_path' ? 'Select Primary Drive Path' : 'Select Secondary Drive Path'}
        description="Browse the BitProtector host filesystem and choose a directory path for this drive slot."
        mode="directory"
        value={pickerField === 'primary_path' ? primaryPath ?? '' : secondaryPath ?? ''}
        startPath={pickerField === 'primary_path' ? primaryPath ?? '' : secondaryPath ?? ''}
        confirmLabel="Use Directory"
        onClose={() => setPickerField(null)}
        onPick={(path) => {
          if (pickerField) {
            setValue(pickerField, path, { shouldDirty: true, shouldValidate: true })
          }
          setPickerField(null)
        }}
      />
    </>
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
