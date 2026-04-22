import type { AxiosError, InternalAxiosRequestConfig } from 'axios'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { apiClient, authNavigation } from './client'
import { useAuthStore } from '@/stores/auth-store'

interface InterceptorHandler<T> {
  fulfilled?: (value: T) => T | Promise<T>
  rejected?: (error: unknown) => unknown
}

interface InterceptorWithHandlers<T> {
  handlers: Array<InterceptorHandler<T> | null>
}

function getRequestInterceptor() {
  const handlers = (
    apiClient.interceptors.request as unknown as InterceptorWithHandlers<InternalAxiosRequestConfig>
  ).handlers
  return handlers.find((handler) => handler?.fulfilled)?.fulfilled
}

function getResponseErrorInterceptor() {
  const handlers = (apiClient.interceptors.response as unknown as InterceptorWithHandlers<unknown>)
    .handlers
  return handlers.find((handler) => handler?.rejected)?.rejected
}

function resetAuthStore() {
  localStorage.clear()
  useAuthStore.setState({
    token: null,
    username: null,
    expiresAt: null,
    isAuthenticated: false,
  })
}

function readAuthorizationHeader(config: InternalAxiosRequestConfig) {
  const headers = config.headers as {
    Authorization?: string
    get?: (name: string) => string | undefined
  }
  return typeof headers.get === 'function' ? headers.get('Authorization') : headers.Authorization
}

describe('api client interceptors', () => {
  beforeEach(() => {
    resetAuthStore()
    window.history.replaceState({}, '', '/dashboard')
  })

  it('adds bearer token to outgoing requests when authenticated', async () => {
    useAuthStore.setState({
      token: 'token-123',
      username: 'alice',
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      isAuthenticated: true,
    })

    const requestInterceptor = getRequestInterceptor()
    expect(requestInterceptor).toBeDefined()

    const requestConfig = await requestInterceptor!({
      headers: {},
    } as unknown as InternalAxiosRequestConfig)

    expect(readAuthorizationHeader(requestConfig)).toBe('Bearer token-123')
  })

  it('logs out and redirects to /login on 401 responses', async () => {
    useAuthStore.setState({
      token: 'token-123',
      username: 'alice',
      expiresAt: new Date(Date.now() + 60_000).toISOString(),
      isAuthenticated: true,
    })
    const redirectSpy = vi.spyOn(authNavigation, 'redirectToLogin').mockImplementation(() => {})

    const responseErrorInterceptor = getResponseErrorInterceptor()
    expect(responseErrorInterceptor).toBeDefined()

    const unauthorizedError = {
      isAxiosError: true,
      response: { status: 401 },
    } as AxiosError

    await expect(Promise.resolve(responseErrorInterceptor!(unauthorizedError))).rejects.toBe(
      unauthorizedError
    )
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
    expect(redirectSpy).toHaveBeenCalledTimes(1)
    redirectSpy.mockRestore()
  })
})
