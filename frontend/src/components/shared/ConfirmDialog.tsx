import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
} from '@radix-ui/react-alert-dialog'

interface ConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description?: string
  confirmLabel?: string
  cancelLabel?: string
  onConfirm: () => void
  destructive?: boolean
}

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  onConfirm,
  destructive = false,
}: ConfirmDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent className="fixed left-1/2 top-1/2 z-50 w-full max-w-md -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-card p-6 shadow-lg">
        <div>
          <AlertDialogTitle className="text-base font-semibold">{title}</AlertDialogTitle>
          {description && (
            <AlertDialogDescription className="mt-1 text-sm text-muted-foreground">
              {description}
            </AlertDialogDescription>
          )}
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <AlertDialogCancel className="rounded-md border border-border bg-background px-4 py-2 text-sm hover:bg-accent transition-colors">
            {cancelLabel}
          </AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className={`rounded-md px-4 py-2 text-sm font-medium text-white transition-colors ${
              destructive ? 'bg-destructive hover:bg-destructive/90' : 'bg-primary hover:bg-primary/90'
            }`}
          >
            {confirmLabel}
          </AlertDialogAction>
        </div>
      </AlertDialogContent>
    </AlertDialog>
  )
}
