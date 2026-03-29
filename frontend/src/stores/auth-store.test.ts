import { beforeEach, describe, expect, it } from 'vitest'
import type { LoginResponse } from '@/types/auth'
import { useAuthStore } from './auth-store'

function resetStore() {
  localStorage.clear()
  useAuthStore.setState({
    token: null,
    username: null,
    expiresAt: null,
    isAuthenticated: false,
  })
}

describe('auth-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('stores auth fields on login', () => {
    const response: LoginResponse = {
      token: 'jwt-token',
      username: 'alice',
      expires_at: new Date(Date.now() + 60_000).toISOString(),
    }

    useAuthStore.getState().login(response)
    const state = useAuthStore.getState()

    expect(state.token).toBe('jwt-token')
    expect(state.username).toBe('alice')
    expect(state.expiresAt).toBe(response.expires_at)
    expect(state.isAuthenticated).toBe(true)
  })

  it('clears auth fields on logout', () => {
    useAuthStore.getState().login({
      token: 'jwt-token',
      username: 'alice',
      expires_at: new Date(Date.now() + 60_000).toISOString(),
    })

    useAuthStore.getState().logout()
    const state = useAuthStore.getState()

    expect(state.token).toBeNull()
    expect(state.username).toBeNull()
    expect(state.expiresAt).toBeNull()
    expect(state.isAuthenticated).toBe(false)
  })

  it('detects token expiration correctly', () => {
    useAuthStore.setState({
      expiresAt: new Date(Date.now() - 1_000).toISOString(),
      isAuthenticated: true,
    })
    expect(useAuthStore.getState().isTokenExpired()).toBe(true)

    useAuthStore.setState({
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      isAuthenticated: true,
    })
    expect(useAuthStore.getState().isTokenExpired()).toBe(false)
  })
})
