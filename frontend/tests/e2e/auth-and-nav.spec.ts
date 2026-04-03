import { test, expect } from './support/fixtures'
import { loginThroughUi, openSidebarRoute } from './support/ui'

test.use({ storageState: { cookies: [], origins: [] } })

test('logs in through the live backend, visits protected pages, and logs out', async ({ page }) => {
  await loginThroughUi(page)

  await expect(page.getByRole('heading', { level: 1, name: 'Dashboard' })).toBeVisible()

  await openSidebarRoute(page, 'drives')
  await expect(page.getByRole('heading', { level: 1, name: 'Drive Pairs' })).toBeVisible()

  await openSidebarRoute(page, 'sync')
  await expect(page.getByRole('heading', { level: 1, name: 'Sync Queue' })).toBeVisible()

  await openSidebarRoute(page, 'files')
  await expect(page.getByTestId('file-browser-page')).toBeVisible()

  await page.goto('/folders')
  await expect(page).toHaveURL(/\/files$/)

  await page.getByTestId('logout-button').click()
  await expect(page).toHaveURL(/\/login$/)

  await page.goto('/dashboard')
  await expect(page).toHaveURL(/\/login$/)
})
