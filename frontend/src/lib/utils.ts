import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** Truncate a string to maxLength with ellipsis */
export function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str
  return str.slice(0, maxLength - 1) + '…'
}

/** Resolve a file's full active-side path from drive pair and relative_path */
export function resolveActivePath(
  activePath: string,
  relativePath: string
): string {
  return activePath.replace(/\/$/, '') + '/' + relativePath.replace(/^\//, '')
}
