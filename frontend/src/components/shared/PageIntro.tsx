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
    <div className={cn('flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between', className)}>
      <div>
        <h1 className="text-2xl font-semibold tracking-tight" data-testid="page-title">
          {title}
        </h1>
        <p className="mt-1 text-sm text-muted-foreground" data-testid="page-subtitle">
          {subtitle}
        </p>
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  )
}
