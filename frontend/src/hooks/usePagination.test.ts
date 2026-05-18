import { renderHook, act } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { usePagination } from './usePagination'

describe('usePagination', () => {
  it('starts at page 1 and perPage 50 by default', () => {
    const { result } = renderHook(() => usePagination())
    expect(result.current.page).toBe(1)
    expect(result.current.perPage).toBe(50)
  })

  it('accepts custom initial page and perPage', () => {
    const { result } = renderHook(() => usePagination(3, 20))
    expect(result.current.page).toBe(3)
    expect(result.current.perPage).toBe(20)
  })

  it('nextPage increments page', () => {
    const { result } = renderHook(() => usePagination())
    act(() => result.current.nextPage())
    expect(result.current.page).toBe(2)
  })

  it('prevPage decrements page', () => {
    const { result } = renderHook(() => usePagination(3))
    act(() => result.current.prevPage())
    expect(result.current.page).toBe(2)
  })

  it('prevPage does not go below 1', () => {
    const { result } = renderHook(() => usePagination(1))
    act(() => result.current.prevPage())
    expect(result.current.page).toBe(1)
  })

  it('goToPage sets specific page', () => {
    const { result } = renderHook(() => usePagination())
    act(() => result.current.goToPage(5))
    expect(result.current.page).toBe(5)
  })

  it('goToPage clamps to 1 when given value < 1', () => {
    const { result } = renderHook(() => usePagination())
    act(() => result.current.goToPage(0))
    expect(result.current.page).toBe(1)
  })

  it('reset returns to page 1', () => {
    const { result } = renderHook(() => usePagination())
    act(() => result.current.goToPage(7))
    act(() => result.current.reset())
    expect(result.current.page).toBe(1)
  })
})
