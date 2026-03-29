import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import type { LoginResponse } from '@/types/auth'

interface AuthState {
  token: string | null
  username: string | null
  expiresAt: string | null
  isAuthenticated: boolean

  login(response: LoginResponse): void
  logout(): void
  isTokenExpired(): boolean
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      token: null,
      username: null,
      expiresAt: null,
      isAuthenticated: false,

      login(response: LoginResponse) {
        set({
          token: response.token,
          username: response.username,
          expiresAt: response.expires_at,
          isAuthenticated: true,
        })
      },

      logout() {
        set({ token: null, username: null, expiresAt: null, isAuthenticated: false })
      },

      isTokenExpired() {
        const { expiresAt } = get()
        if (!expiresAt) return true
        return new Date(expiresAt) <= new Date()
      },
    }),
    {
      name: 'bitprotector-auth',
      storage: createJSONStorage(() => localStorage),
    }
  )
)
