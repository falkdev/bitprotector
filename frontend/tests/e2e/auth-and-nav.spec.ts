import { test, expect } from './support/fixtures'
import { loginThroughUi, openSidebarRoute } from './support/ui'

test.use({ storageState: { cookies: [], origins: [] } })

test('logs in through the live backend, visits protected pages, and logs out', async ({ page }) => {
  await loginThroughUi(page)
  await expect(page).toHaveURL(/\/dashboard$/)
  await expect(page.getByTestId('page-title')).toHaveText('Dashboard')

  await expect(page.getByTestId('status-metric-files-tracked')).toBeVisible()

  await openSidebarRoute(page, 'drives')
  await expect(page.getByTestId('page-title')).toHaveText('Drives')
  await expect(page.getByTestId('add-drive-button')).toBeVisible()

  await openSidebarRoute(page, 'sync')
  await expect(page.getByTestId('page-title')).toHaveText('Sync Queue')
  await expect(page.getByRole('button', { name: 'Process Queue' })).toBeVisible()

  await openSidebarRoute(page, 'files')
  await expect(page.getByTestId('page-title')).toHaveText('Tracking Workspace')
  await expect(page.getByTestId('file-browser-page')).toBeVisible()

  await openSidebarRoute(page, 'integrity')
  await expect(page.getByTestId('page-title')).toHaveText('Integrity')

  await openSidebarRoute(page, 'scheduler')
  await expect(page.getByTestId('page-title')).toHaveText('Scheduler')

  await openSidebarRoute(page, 'logs')
  await expect(page.getByTestId('page-title')).toHaveText('Logs')

  await openSidebarRoute(page, 'database')
  await expect(page.getByTestId('page-title')).toHaveText('Database Backups')

  await page.goto('/folders')
  await expect(page).toHaveURL(/\/files$/)
  await expect(page.getByTestId('page-title')).toHaveText('Tracking Workspace')

  await page.getByTestId('user-menu-trigger').click()
  await page.getByTestId('user-menu-logout').click()
  await expect(page).toHaveURL(/\/login$/)

  await page.goto('/dashboard')
  await expect(page).toHaveURL(/\/login$/)
})
