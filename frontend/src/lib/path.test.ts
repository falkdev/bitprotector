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

  it('returns error when resolving path with no active drive root', () => {
    expect(resolveTrackedPathInput(null, 'docs/report.pdf')).toEqual({
      relativePath: null,
      absolutePath: null,
      error: 'Select a drive pair first',
    })
  })

  it('resolveAbsolutePathForPicker returns root when value has parent-dir traversal', () => {
    expect(resolveAbsolutePathForPicker('/mnt/drive-a', '../etc/passwd')).toBe('/mnt/drive-a')
  })

  it('returns "/" when resolveAbsolutePathForPicker has no activeRoot and value is relative', () => {
    expect(resolveAbsolutePathForPicker(null, 'relative/path')).toBe('/')
  })

  it('joins relative path under root "/"', () => {
    expect(joinAbsoluteFilesystemPath('/', 'home/user')).toBe('/home/user')
  })

  it('returns root when relativePath is empty string', () => {
    expect(joinAbsoluteFilesystemPath('/mnt/drive', '')).toBe('/mnt/drive')
  })

  it('resolveTrackedPathInput returns error when path has parent-dir traversal', () => {
    expect(resolveTrackedPathInput('/mnt/primary', '../etc/passwd')).toEqual({
      relativePath: null,
      absolutePath: null,
      error: 'Parent-directory traversal is not allowed',
    })
  })

  it('resolveTrackedPathInput returns error when relative path resolves to root', () => {
    // Giving the exact root path as input — resolves to normalizedRoot which equals absolutePath
    expect(resolveTrackedPathInput('/mnt/primary', '/mnt/primary').error).toBe(
      'Select a path inside the active drive root'
    )
  })
})
