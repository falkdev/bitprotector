import { renderHook, act, waitFor } from '@testing-library/react'
import { describe, expect, it, vi, afterEach } from 'vitest'
import { usePolling } from './usePolling'

describe('usePolling', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('fetches immediately when immediate=true (default)', async () => {
    const fetchFn = vi.fn().mockResolvedValue('data')
    renderHook(() => usePolling(fetchFn, { interval: 1000 }))

    await waitFor(() => expect(fetchFn).toHaveBeenCalledTimes(1))
  })

  it('does not fetch immediately when immediate=false', async () => {
    vi.useFakeTimers()
    const fetchFn = vi.fn().mockResolvedValue('data')
    renderHook(() => usePolling(fetchFn, { interval: 1000, immediate: false }))

    // Small tick to let effects run
    act(() => {
      vi.advanceTimersByTime(0)
    })
    expect(fetchFn).not.toHaveBeenCalled()
  })

  it('sets data on successful fetch', async () => {
    const fetchFn = vi.fn().mockResolvedValue('hello')
    const { result } = renderHook(() => usePolling(fetchFn, { interval: 10000 }))

    await waitFor(() => expect(result.current.data).toBe('hello'))
    expect(result.current.error).toBeNull()
  })

  it('sets error on failed fetch', async () => {
    const fetchFn = vi.fn().mockRejectedValue(new Error('network error'))
    const { result } = renderHook(() => usePolling(fetchFn, { interval: 10000 }))

    await waitFor(() => expect(result.current.error).toBeTruthy())
  })

  it('does not fetch when enabled=false', async () => {
    vi.useFakeTimers()
    const fetchFn = vi.fn().mockResolvedValue('data')
    renderHook(() => usePolling(fetchFn, { enabled: false, interval: 100 }))

    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(fetchFn).not.toHaveBeenCalled()
  })

  it('pause stops polling', async () => {
    vi.useFakeTimers()
    const fetchFn = vi.fn().mockResolvedValue('data')
    const { result } = renderHook(() => usePolling(fetchFn, { interval: 100, immediate: false }))

    act(() => result.current.pause())
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(fetchFn).not.toHaveBeenCalled()
  })

  it('refresh triggers a manual fetch', async () => {
    const fetchFn = vi.fn().mockResolvedValue('refreshed')
    const { result } = renderHook(() => usePolling(fetchFn, { interval: 100000 }))

    await waitFor(() => expect(fetchFn).toHaveBeenCalledTimes(1))

    act(() => result.current.refresh())
    await waitFor(() => expect(fetchFn).toHaveBeenCalledTimes(2))
  })
})
