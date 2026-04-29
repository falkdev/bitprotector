import { renderHook, act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useTheme } from './use-theme'
import { useThemeStore } from '@/stores/theme-store'

function resetStore() {
  localStorage.clear()
  useThemeStore.setState({ override: null })
}

type ChangeListener = (event: MediaQueryListEvent) => void

function makeMatchMediaStub(initialMatches: boolean) {
  const listeners: ChangeListener[] = []
  const stub = {
    matches: initialMatches,
    media: '(prefers-color-scheme: dark)',
    onchange: null,
    addEventListener: (_type: string, cb: ChangeListener) => {
      listeners.push(cb)
    },
    removeEventListener: (_type: string, cb: ChangeListener) => {
      const idx = listeners.indexOf(cb)
      if (idx !== -1) listeners.splice(idx, 1)
    },
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
    /** Helper: fire the change event with a new matches value */
    fireChange(matches: boolean) {
      stub.matches = matches
      for (const cb of listeners) {
        cb({ matches } as MediaQueryListEvent)
      }
    },
  }
  return stub
}

describe('useTheme', () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('returns dark when override is null and system is dark', () => {
    const stub = makeMatchMediaStub(true)
    vi.spyOn(window, 'matchMedia').mockReturnValue(stub as unknown as MediaQueryList)

    const { result } = renderHook(() => useTheme())
    expect(result.current.theme).toBe('dark')
  })

  it('returns light when override is null and system is light', () => {
    const stub = makeMatchMediaStub(false)
    vi.spyOn(window, 'matchMedia').mockReturnValue(stub as unknown as MediaQueryList)

    const { result } = renderHook(() => useTheme())
    expect(result.current.theme).toBe('light')
  })

  it('override wins over system preference (light override, dark system)', () => {
    const stub = makeMatchMediaStub(true)
    vi.spyOn(window, 'matchMedia').mockReturnValue(stub as unknown as MediaQueryList)

    useThemeStore.setState({ override: 'light' })
    const { result } = renderHook(() => useTheme())
    expect(result.current.theme).toBe('light')
  })

  it('toggle from dark sets override to light', () => {
    const stub = makeMatchMediaStub(true) // system dark
    vi.spyOn(window, 'matchMedia').mockReturnValue(stub as unknown as MediaQueryList)

    const { result } = renderHook(() => useTheme())
    expect(result.current.theme).toBe('dark')

    act(() => {
      result.current.toggle()
    })

    expect(useThemeStore.getState().override).toBe('light')
    expect(result.current.theme).toBe('light')
  })

  it('live OS change updates theme when no override is set', async () => {
    const stub = makeMatchMediaStub(false) // start: system light
    vi.spyOn(window, 'matchMedia').mockReturnValue(stub as unknown as MediaQueryList)

    const { result } = renderHook(() => useTheme())
    expect(result.current.theme).toBe('light')

    act(() => {
      stub.fireChange(true) // OS switches to dark
    })

    expect(result.current.theme).toBe('dark')
  })
})
