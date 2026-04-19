import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface PageIntroProps {
  title: string
  subtitle: string
  actions?: ReactNode
  className?: string
}

export function PageIntro({ title, subtitle, actions, className }: PageIntroProps) {
  return (
    <div className={cn('flex flex-wrap items-start justify-between gap-3', className)}>
      <div className="min-w-0">
        <h1 className="text-2xl font-semibold tracking-tight" data-testid="page-title">
          {title}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground" data-testid="page-subtitle">
          {subtitle}
        </p>
      </div>
      {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  )
}
