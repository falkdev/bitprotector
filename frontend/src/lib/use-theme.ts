import { useSyncExternalStore } from 'react'
import { useThemeStore } from '@/stores/theme-store'

function getSystemSnapshot(): boolean {
  if (typeof window === 'undefined') return false
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

function subscribeToSystem(callback: () => void): () => void {
  if (typeof window === 'undefined') return () => {}
  const mq = window.matchMedia('(prefers-color-scheme: dark)')
  mq.addEventListener('change', callback)
  return () => mq.removeEventListener('change', callback)
}

export function useTheme() {
  const override = useThemeStore((state) => state.override)
  const setOverride = useThemeStore((state) => state.setOverride)

  const systemIsDark = useSyncExternalStore(subscribeToSystem, getSystemSnapshot, () => false)

  const theme: 'light' | 'dark' = override ?? (systemIsDark ? 'dark' : 'light')

  function toggle() {
    setOverride(theme === 'dark' ? 'light' : 'dark')
  }

  return { theme, override, toggle }
}
