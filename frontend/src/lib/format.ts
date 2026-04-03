/** Format bytes as a human-readable size string */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  const value = bytes / Math.pow(1024, i)
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

/** Format an ISO 8601 timestamp as a locale-aware string */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const preferredLanguage =
    typeof navigator !== 'undefined' && navigator.language ? navigator.language : undefined
  return new Date(iso).toLocaleString(preferredLanguage)
}

/** Format an ISO 8601 timestamp as a relative time string (e.g. "2 hours ago") */
export function formatRelative(iso: string | null | undefined): string {
  if (!iso) return '—'
  const diff = Date.now() - new Date(iso).getTime()
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

/** Truncate a BLAKE3 checksum for display */
export function formatChecksum(checksum: string | null | undefined): string {
  if (!checksum) return '—'
  return checksum.slice(0, 12) + '…'
}

/** Format a publish path or real path for display */
export function formatPath(path: string | null | undefined): string {
  if (!path) return '—'
  return path
}
