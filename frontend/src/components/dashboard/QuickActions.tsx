import { useState } from 'react'
import { ShieldCheck, RefreshCw, Database } from 'lucide-react'

interface QuickActionsProps {
  onIntegrityCheck: () => Promise<void>
  onProcessSync: () => Promise<void>
  onRunBackup: () => Promise<void>
}

export function QuickActions({ onIntegrityCheck, onProcessSync, onRunBackup }: QuickActionsProps) {
  const [loadingAction, setLoadingAction] = useState<string | null>(null)

  const handle = (key: string, fn: () => Promise<void>) => async () => {
    setLoadingAction(key)
    try {
      await fn()
    } finally {
      setLoadingAction(null)
    }
  }

  const actions = [
    {
      key: 'integrity',
      label: 'Check All Files',
      description: 'Run integrity check on every tracked file',
      icon: ShieldCheck,
      onClick: handle('integrity', onIntegrityCheck),
    },
    {
      key: 'sync',
      label: 'Process Sync Queue',
      description: 'Process all pending sync queue items',
      icon: RefreshCw,
      onClick: handle('sync', onProcessSync),
    },
    {
      key: 'backup',
      label: 'Run Database Backup',
      description: 'Trigger a manual database backup now',
      icon: Database,
      onClick: handle('backup', onRunBackup),
    },
  ]

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <h2 className="mb-3 text-sm font-semibold">Quick Actions</h2>
      <div className="space-y-2">
        {actions.map(({ key, label, description, icon: Icon, onClick }) => (
          <button
            key={key}
            onClick={onClick}
            disabled={loadingAction !== null}
            data-testid={`quick-action-${key}`}
            className="flex w-full items-start gap-3 rounded-md border border-border p-3 text-left hover:bg-accent transition-colors disabled:opacity-60 disabled:cursor-not-allowed"
          >
            <Icon className="mt-0.5 h-4 w-4 flex-shrink-0 text-primary" />
            <div>
              <p className="text-sm font-medium">
                {loadingAction === key ? 'Running…' : label}
              </p>
              <p className="text-xs text-muted-foreground">{description}</p>
            </div>
          </button>
        ))}
      </div>
    </div>
  )
}
