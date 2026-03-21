import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

export interface Column<T> {
  key: string
  header: string
  cell: (row: T) => ReactNode
  className?: string
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  rowKey: (row: T) => string | number
  onRowClick?: (row: T) => void
  selectedRowKey?: string | number | null
  className?: string
  emptyState?: ReactNode
}

export function DataTable<T>({
  columns,
  data,
  rowKey,
  onRowClick,
  selectedRowKey,
  className,
  emptyState,
}: DataTableProps<T>) {
  if (data.length === 0 && emptyState) return <>{emptyState}</>

  return (
    <div className={cn('w-full overflow-auto rounded-md border border-border', className)}>
      <table className="w-full text-sm">
        <thead className="bg-muted/40 text-muted-foreground">
          <tr>
            {columns.map((col) => (
              <th
                key={col.key}
                className={cn('px-3 py-2 text-left font-medium', col.className)}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {data.map((row) => {
            const key = rowKey(row)
            return (
              <tr
                key={key}
                onClick={onRowClick ? () => onRowClick(row) : undefined}
                className={cn(
                  'border-t border-border transition-colors',
                  onRowClick && 'cursor-pointer hover:bg-accent/50',
                  selectedRowKey === key && 'bg-primary/5'
                )}
              >
                {columns.map((col) => (
                  <td key={col.key} className={cn('px-3 py-2', col.className)}>
                    {col.cell(row)}
                  </td>
                ))}
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
