import { describe, expect, it } from 'vitest'
import { cn, getErrorMessage, resolveActivePath, truncate } from './utils'

describe('cn', () => {
  it('merges class names', () => {
    expect(cn('foo', 'bar')).toBe('foo bar')
  })

  it('handles falsy conditional classes', () => {
    const conditional = false
    expect(cn('foo', conditional && 'bar', 'baz')).toBe('foo baz')
  })

  it('resolves tailwind conflicts (last one wins)', () => {
    expect(cn('text-red-500', 'text-blue-500')).toBe('text-blue-500')
  })
})

describe('truncate', () => {
  it('returns string unchanged when shorter than maxLength', () => {
    expect(truncate('hello', 10)).toBe('hello')
  })

  it('returns string unchanged when exactly maxLength', () => {
    expect(truncate('hello', 5)).toBe('hello')
  })

  it('truncates long strings with an ellipsis character', () => {
    expect(truncate('hello world', 8)).toBe('hello w…')
  })
})

describe('resolveActivePath', () => {
  it('joins activePath and relativePath with a slash', () => {
    expect(resolveActivePath('/mnt/primary', 'docs/file.pdf')).toBe('/mnt/primary/docs/file.pdf')
  })

  it('strips trailing slash from activePath', () => {
    expect(resolveActivePath('/mnt/primary/', 'docs/file.pdf')).toBe('/mnt/primary/docs/file.pdf')
  })

  it('strips leading slash from relativePath', () => {
    expect(resolveActivePath('/mnt/primary', '/docs/file.pdf')).toBe('/mnt/primary/docs/file.pdf')
  })
})

describe('getErrorMessage', () => {
  it('prefers plain-text API response bodies', () => {
    const error = {
      message: 'Request failed with status code 400',
      response: {
        data: 'Selected path must be inside the active drive root',
      },
    }

    expect(getErrorMessage(error, 'Failed to add folder')).toBe(
      'Selected path must be inside the active drive root'
    )
  })

  it('prefers nested API error.message payloads', () => {
    const error = {
      message: 'Request failed with status code 400',
      response: {
        data: {
          error: {
            message: 'Folder already tracked',
          },
        },
      },
    }

    expect(getErrorMessage(error, 'Failed to add folder')).toBe('Folder already tracked')
  })

  it('falls back to Error.message when response has no usable message', () => {
    expect(getErrorMessage(new Error('Network Error'), 'Failed to add folder')).toBe('Network Error')
  })

  it('falls back to default message for unknown error values', () => {
    expect(getErrorMessage(null, 'Failed to add folder')).toBe('Failed to add folder')
  })
})
