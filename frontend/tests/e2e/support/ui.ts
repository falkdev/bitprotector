import { expect, type Locator, type Page } from '@playwright/test'

const QEMU_WEB_USER = process.env.QEMU_WEB_USER ?? 'testuser'
const QEMU_WEB_PASSWORD = process.env.QEMU_WEB_PASSWORD ?? 'bitprotector'

export async function expectToast(page: Page, message: string | RegExp) {
  await expect(page.locator('[data-sonner-toast]').filter({ hasText: message }).first()).toBeVisible()
}

export async function loginThroughUi(page: Page) {
  await page.goto('/login')
  await page.getByTestId('username-input').fill(QEMU_WEB_USER)
  await page.getByTestId('password-input').fill(QEMU_WEB_PASSWORD)
  await page.getByTestId('login-button').click()
  await expect(page).toHaveURL(/\/dashboard$/)
}

export async function openSidebarRoute(page: Page, route: string) {
  await page.getByTestId(`nav-${route}`).click()
}

export async function createDrivePair(
  page: Page,
  options: {
    name: string
    primaryPath: string
    secondaryPath: string
  }
) {
  await page.goto('/drives')
  await page.getByTestId('add-drive-button').click()
  await page.getByTestId('drive-name-input').fill(options.name)
  await page.getByTestId('drive-primary-path-input').fill(options.primaryPath)
  await page.getByTestId('drive-secondary-path-input').fill(options.secondaryPath)
  await page.getByRole('button', { name: 'Create' }).click()

  const card = page
    .locator('[data-testid^="drive-card-"]')
    .filter({ hasText: options.name })
    .first()
  await expect(card).toBeVisible()
  return card
}

export function driveCardByName(page: Page, name: string): Locator {
  return page.locator('[data-testid^="drive-card-"]').filter({ hasText: name }).first()
}
