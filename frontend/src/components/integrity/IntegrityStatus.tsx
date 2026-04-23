import { AlertTriangle, CheckCircle, XCircle, HelpCircle, RefreshCw } from 'lucide-react'
import type { IntegrityStatus } from '@/types/integrity'
import { cn } from '@/lib/utils'

interface IntegrityStatusBadgeProps {
  status: IntegrityStatus | 'internal_error'
  className?: string
}

const STATUS_CONFIG: Record<
  IntegrityStatus | 'internal_error',
  { label: string; color: string; icon: React.ReactNode }
> = {
  ok: {
    label: 'OK',
    color: 'text-green-700 bg-green-50 border-green-200',
    icon: <CheckCircle className="h-4 w-4" />,
  },
  master_corrupted: {
    label: 'Primary corrupt',
    color: 'text-red-700 bg-red-50 border-red-200',
    icon: <XCircle className="h-4 w-4" />,
  },
  mirror_corrupted: {
    label: 'Mirror corrupt',
    color: 'text-red-700 bg-red-50 border-red-200',
    icon: <XCircle className="h-4 w-4" />,
  },
  both_corrupted: {
    label: 'Both corrupt',
    color: 'text-red-700 bg-red-50 border-red-200',
    icon: <XCircle className="h-4 w-4" />,
  },
  master_missing: {
    label: 'Primary missing',
    color: 'text-orange-700 bg-orange-50 border-orange-200',
    icon: <AlertTriangle className="h-4 w-4" />,
  },
  mirror_missing: {
    label: 'Mirror missing',
    color: 'text-yellow-700 bg-yellow-50 border-yellow-200',
    icon: <AlertTriangle className="h-4 w-4" />,
  },
  primary_drive_unavailable: {
    label: 'Primary unavailable',
    color: 'text-blue-700 bg-blue-50 border-blue-200',
    icon: <RefreshCw className="h-4 w-4" />,
  },
  secondary_drive_unavailable: {
    label: 'Mirror unavailable',
    color: 'text-blue-700 bg-blue-50 border-blue-200',
    icon: <RefreshCw className="h-4 w-4" />,
  },
  internal_error: {
    label: 'Internal error',
    color: 'text-red-700 bg-red-50 border-red-200',
    icon: <AlertTriangle className="h-4 w-4" />,
  },
}

const FALLBACK_STATUS = {
  label: 'Unknown',
  color: 'text-gray-600 bg-gray-50 border-gray-200',
  icon: <HelpCircle className="h-4 w-4" />,
}

export function IntegrityStatusBadge({ status, className }: IntegrityStatusBadgeProps) {
  const cfg = STATUS_CONFIG[status] ?? FALLBACK_STATUS
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-2 py-0.5 rounded-full border text-xs font-medium',
        cfg.color,
        className
      )}
    >
      {cfg.icon}
      {cfg.label}
    </span>
  )
}
