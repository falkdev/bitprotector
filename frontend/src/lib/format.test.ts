import { afterEach, describe, expect, it, vi } from 'vitest'
import { formatDate } from './format'

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
