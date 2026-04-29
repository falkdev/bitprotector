import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

type ThemeOverride = 'light' | 'dark' | null

interface ThemeState {
  override: ThemeOverride
  setOverride(value: Exclude<ThemeOverride, null>): void
  clearOverride(): void
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set) => ({
      override: null,

      setOverride(value) {
        set({ override: value })
      },

      clearOverride() {
        set({ override: null })
      },
    }),
    {
      name: 'bitprotector-theme',
      storage: createJSONStorage(() => localStorage),
    }
  )
)
