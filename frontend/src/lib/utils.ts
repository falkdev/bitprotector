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
  if (error && typeof error === 'object' && 'response' in error) {
    const response = error.response
    if (response && typeof response === 'object' && 'data' in response) {
      const data = response.data
      if (typeof data === 'string' && data.trim()) {
        return data
      }
      if (data && typeof data === 'object') {
        if ('error' in data && data.error && typeof data.error === 'object' && 'message' in data.error) {
          const nestedMessage = data.error.message
          if (typeof nestedMessage === 'string' && nestedMessage.trim()) {
            return nestedMessage
          }
        }
        if ('message' in data) {
          const message = data.message
          if (typeof message === 'string' && message.trim()) {
            return message
          }
        }
      }
    }
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  return defaultMessage
}
