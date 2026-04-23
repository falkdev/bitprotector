import { describe, expect, it } from 'vitest'
import {
  getActiveDrivePath,
  joinAbsoluteFilesystemPath,
  normalizeAbsoluteFilesystemPath,
  resolveAbsolutePathForPicker,
  resolveTrackedPathInput,
} from './path'

describe('path helpers', () => {
  it('normalizes absolute paths', () => {
    expect(normalizeAbsoluteFilesystemPath('/mnt//drive-a/./docs')).toBe('/mnt/drive-a/docs')
  })

  it('joins a drive root with a relative path', () => {
    expect(joinAbsoluteFilesystemPath('/mnt/drive-a', 'docs/report.pdf')).toBe(
      '/mnt/drive-a/docs/report.pdf'
    )
  })

  it('resolves tracked absolute paths back to a relative path', () => {
    expect(resolveTrackedPathInput('/mnt/drive-a', '/mnt/drive-a/docs/report.pdf')).toEqual({
      relativePath: 'docs/report.pdf',
      absolutePath: '/mnt/drive-a/docs/report.pdf',
      error: null,
    })
  })

  it('rejects paths outside the selected drive root', () => {
    expect(resolveTrackedPathInput('/mnt/drive-a', '/mnt/other-drive/report.pdf').error).toBe(
      'Selected path must be inside the active drive root'
    )
  })

  it('opens the picker at the current relative path when a drive root exists', () => {
    expect(resolveAbsolutePathForPicker('/mnt/drive-a', 'docs/report.pdf')).toBe(
      '/mnt/drive-a/docs/report.pdf'
    )
  })

  it('returns the active drive path for the active role', () => {
    expect(getActiveDrivePath('/mnt/primary', '/mnt/secondary', 'secondary')).toBe('/mnt/secondary')
  })
})
