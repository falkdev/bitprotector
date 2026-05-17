import { afterEach, describe, expect, it, vi } from 'vitest'
import { formatBytes, formatChecksum, formatDate, formatPath, formatRelative } from './format'

describe('formatDate', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('returns a placeholder for empty values', () => {
    expect(formatDate(null)).toBe('—')
    expect(formatDate(undefined)).toBe('—')
    expect(formatDate('')).toBe('—')
  })

  it('formats using navigator.language', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('sv')
    const toLocaleStringSpy = vi
      .spyOn(Date.prototype, 'toLocaleString')
      .mockReturnValue('formatted')

    expect(formatDate('2026-04-03T12:34:56Z')).toBe('formatted')
    expect(toLocaleStringSpy).toHaveBeenCalledWith('sv')
  })

  it('falls back to default locale when navigator.language is empty', () => {
    vi.spyOn(navigator, 'language', 'get').mockReturnValue('')
    const toLocaleStringSpy = vi
      .spyOn(Date.prototype, 'toLocaleString')
      .mockReturnValue('formatted')

    expect(formatDate('2026-04-03T12:34:56Z')).toBe('formatted')
    expect(toLocaleStringSpy).toHaveBeenCalledWith(undefined)
  })
})

describe('formatBytes', () => {
  it('returns placeholder for null', () => {
    expect(formatBytes(null)).toBe('—')
  })

  it('returns placeholder for undefined', () => {
    expect(formatBytes(undefined)).toBe('—')
  })

  it('returns 0 B for zero', () => {
    expect(formatBytes(0)).toBe('0 B')
  })

  it('formats bytes without decimal', () => {
    expect(formatBytes(512)).toBe('512 B')
  })

  it('formats KB with one decimal', () => {
    expect(formatBytes(1024)).toBe('1.0 KB')
  })

  it('formats MB', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB')
  })
})

describe('formatRelative', () => {
  it('returns placeholder for null', () => {
    expect(formatRelative(null)).toBe('—')
  })

  it('returns placeholder for undefined', () => {
    expect(formatRelative(undefined)).toBe('—')
  })

  it('returns seconds ago for recent timestamps', () => {
    const iso = new Date(Date.now() - 30_000).toISOString()
    expect(formatRelative(iso)).toBe('30s ago')
  })

  it('returns minutes ago', () => {
    const iso = new Date(Date.now() - 5 * 60_000).toISOString()
    expect(formatRelative(iso)).toBe('5m ago')
  })

  it('returns hours ago', () => {
    const iso = new Date(Date.now() - 3 * 3_600_000).toISOString()
    expect(formatRelative(iso)).toBe('3h ago')
  })

  it('returns days ago', () => {
    const iso = new Date(Date.now() - 2 * 24 * 3_600_000).toISOString()
    expect(formatRelative(iso)).toBe('2d ago')
  })
})

describe('formatChecksum', () => {
  it('returns placeholder for null', () => {
    expect(formatChecksum(null)).toBe('—')
  })

  it('returns placeholder for undefined', () => {
    expect(formatChecksum(undefined)).toBe('—')
  })

  it('truncates to 12 chars with trailing ellipsis', () => {
    expect(formatChecksum('abcdef123456789xyz')).toBe('abcdef123456…')
  })
})

describe('formatPath', () => {
  it('returns placeholder for null', () => {
    expect(formatPath(null)).toBe('—')
  })

  it('returns placeholder for undefined', () => {
    expect(formatPath(undefined)).toBe('—')
  })

  it('returns the path string as-is', () => {
    expect(formatPath('/some/path/file.txt')).toBe('/some/path/file.txt')
  })
})
