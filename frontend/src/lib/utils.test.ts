import { describe, expect, it } from 'vitest'
import { cn, truncate, resolveActivePath } from './utils'

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
