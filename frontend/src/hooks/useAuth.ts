import { useCallback } from 'react'
import { useAuthStore } from '@/stores/auth-store'
import { authApi } from '@/api/auth'

export function useAuth() {
  const { token, username, expiresAt, isAuthenticated, login, logout, isTokenExpired } =
    useAuthStore()

  const validate = useCallback(async (): Promise<boolean> => {
    if (!token || isTokenExpired()) {
      logout()
      return false
    }
    try {
      const result = await authApi.validate()
      return result.valid
    } catch {
      logout()
      return false
    }
  }, [token, isTokenExpired, logout])

  return { token, username, expiresAt, isAuthenticated, login, logout, validate, isTokenExpired }
}
