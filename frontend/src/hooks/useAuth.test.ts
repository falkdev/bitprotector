import { renderHook, act, waitFor } from '@testing-library/react'
import { describe, expect, it, beforeEach } from 'vitest'
import { useAuth } from './useAuth'
import { useAuthStore } from '@/stores/auth-store'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { HttpResponse } from 'msw'

const FUTURE_EXPIRY = new Date(Date.now() + 1000 * 60 * 60).toISOString()
const PAST_EXPIRY = new Date(Date.now() - 1000).toISOString()

beforeEach(() => {
  useAuthStore.setState({
    token: null,
    username: null,
    expiresAt: null,
    isAuthenticated: false,
  })
})

describe('useAuth', () => {
  it('returns isAuthenticated false initially', () => {
    const { result } = renderHook(() => useAuth())
    expect(result.current.isAuthenticated).toBe(false)
  })

  it('validate returns false and calls logout when no token', async () => {
    const { result } = renderHook(() => useAuth())
    let valid: boolean
    await act(async () => {
      valid = await result.current.validate()
    })
    expect(valid!).toBe(false)
    expect(result.current.isAuthenticated).toBe(false)
  })

  it('validate returns false and calls logout when token is expired', async () => {
    useAuthStore.setState({
      token: 'expired-token',
      username: 'alice',
      expiresAt: PAST_EXPIRY,
      isAuthenticated: true,
    })

    const { result } = renderHook(() => useAuth())
    let valid: boolean
    await act(async () => {
      valid = await result.current.validate()
    })
    expect(valid!).toBe(false)
    expect(result.current.isAuthenticated).toBe(false)
  })

  it('validate calls the API and returns true on success', async () => {
    server.use(
      api.get('/auth/validate', () => HttpResponse.json({ username: 'alice', valid: true }))
    )

    useAuthStore.setState({
      token: 'valid-token',
      username: 'alice',
      expiresAt: FUTURE_EXPIRY,
      isAuthenticated: true,
    })

    const { result } = renderHook(() => useAuth())
    let valid: boolean
    await act(async () => {
      valid = await result.current.validate()
    })
    expect(valid!).toBe(true)
  })

  it('validate calls logout and returns false on API error', async () => {
    server.use(
      api.get('/auth/validate', () => HttpResponse.json({ error: 'unauthorized' }, { status: 401 }))
    )

    useAuthStore.setState({
      token: 'bad-token',
      username: 'bob',
      expiresAt: FUTURE_EXPIRY,
      isAuthenticated: true,
    })

    const { result } = renderHook(() => useAuth())
    let valid: boolean
    await act(async () => {
      valid = await result.current.validate()
    })
    await waitFor(() => expect(result.current.isAuthenticated).toBe(false))
    expect(valid!).toBe(false)
  })

  it('login updates authentication state', () => {
    const { result } = renderHook(() => useAuth())
    act(() => {
      result.current.login({
        token: 'new-token',
        username: 'carol',
        expires_at: FUTURE_EXPIRY,
      })
    })
    expect(result.current.isAuthenticated).toBe(true)
    expect(result.current.username).toBe('carol')
    expect(result.current.token).toBe('new-token')
  })

  it('logout clears authentication state', () => {
    useAuthStore.setState({
      token: 'some-token',
      username: 'dave',
      expiresAt: FUTURE_EXPIRY,
      isAuthenticated: true,
    })

    const { result } = renderHook(() => useAuth())
    act(() => result.current.logout())
    expect(result.current.isAuthenticated).toBe(false)
    expect(result.current.token).toBeNull()
  })
})
