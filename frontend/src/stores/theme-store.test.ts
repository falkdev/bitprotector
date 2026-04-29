import { beforeEach, describe, expect, it } from 'vitest'
import { useThemeStore } from './theme-store'

function resetStore() {
  localStorage.clear()
  useThemeStore.setState({ override: null })
}

describe('theme-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('has null override by default', () => {
    expect(useThemeStore.getState().override).toBeNull()
  })

  it('setOverride updates state and persists to localStorage', () => {
    useThemeStore.getState().setOverride('dark')

    expect(useThemeStore.getState().override).toBe('dark')

    const stored = localStorage.getItem('bitprotector-theme')
    expect(stored).not.toBeNull()
    const parsed = JSON.parse(stored!)
    expect(parsed.state.override).toBe('dark')
  })

  it('setOverride can set light', () => {
    useThemeStore.getState().setOverride('light')
    expect(useThemeStore.getState().override).toBe('light')
  })

  it('clearOverride resets to null', () => {
    useThemeStore.getState().setOverride('dark')
    useThemeStore.getState().clearOverride()
    expect(useThemeStore.getState().override).toBeNull()
  })
})
