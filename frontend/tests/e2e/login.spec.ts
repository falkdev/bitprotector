import { expect, test } from '@playwright/test'

test('login page renders form controls', async ({ page }) => {
  await page.goto('/login')

  await expect(page.getByRole('heading', { name: 'BitProtector' })).toBeVisible()
  await expect(page.getByTestId('login-form')).toBeVisible()
  await expect(page.getByTestId('username-input')).toBeVisible()
  await expect(page.getByTestId('password-input')).toBeVisible()
  await expect(page.getByTestId('login-button')).toBeVisible()

  await page.getByTestId('username-input').fill('testuser')
  await page.getByTestId('password-input').fill('secret')

  await expect(page.getByTestId('username-input')).toHaveValue('testuser')
  await expect(page.getByTestId('password-input')).toHaveValue('secret')
})
