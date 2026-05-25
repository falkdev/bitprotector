import { useEffect, useState } from 'react'
import { Outlet, Link } from 'react-router-dom'
import { Info, X } from 'lucide-react'
import { Sidebar } from './Sidebar'
import { useTheme } from '@/lib/use-theme'
import { useDrivesStore } from '@/stores/drives-store'

export function AppLayout() {
  const { theme } = useTheme()
  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark')
  }, [theme])

  const { drives, initialized, fetch: fetchDrives } = useDrivesStore()
  const [bannerDismissed, setBannerDismissed] = useState(false)
  useEffect(() => {
    void fetchDrives()
  }, [fetchDrives])

  const showNoDrivesBanner = initialized && drives.length === 0 && !bannerDismissed

  return (
    <div className="flex h-screen overflow-hidden bg-background">
      <Sidebar />
      <div className="flex flex-1 flex-col overflow-hidden">
        {showNoDrivesBanner && (
          <div className="flex items-center gap-2.5 border-b border-amber-300 bg-amber-50 px-5 py-2.5 text-sm text-amber-900 dark:border-amber-700/50 dark:bg-amber-900/20 dark:text-amber-200">
            <Info className="h-4 w-4 shrink-0" />
            <span className="flex-1">
              <strong>Get started:</strong> Add a drive pair on the{' '}
              <Link
                to="/drives"
                className="font-medium underline underline-offset-2 hover:opacity-80"
              >
                Drives page
              </Link>{' '}
              to begin using BitProtector.
            </span>
            <button
              type="button"
              onClick={() => setBannerDismissed(true)}
              aria-label="Dismiss"
              className="shrink-0 rounded p-0.5 opacity-70 hover:opacity-100"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}
        <main className="flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
