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
export function resolveActivePath(activePath: string, relativePath: string): string {
  return activePath.replace(/\/$/, '') + '/' + relativePath.replace(/^\//, '')
}

/** Extract error message from Axios error response or Error object */
export function getErrorMessage(error: unknown, defaultMessage: string): string {
  // Check if it's an Error with a message property
  if (error instanceof Error) {
    return error.message
  }

  // Check if it's an object with response.data containing nested error.message
  if (
    error &&
    typeof error === 'object' &&
    'response' in error &&
    error.response &&
    typeof error.response === 'object' &&
    'data' in error.response &&
    error.response.data &&
    typeof error.response.data === 'object' &&
    'error' in error.response.data &&
    error.response.data.error &&
    typeof error.response.data.error === 'object' &&
    'message' in error.response.data.error
  ) {
    const message = error.response.data.error.message
    if (typeof message === 'string') {
      return message
    }
  }

  // Fallback to default message
  return defaultMessage
}
