import { test, expect } from './support/fixtures'
import { expectToast } from './support/ui'

test('creates an interval-based sync schedule, edits it, and deletes it', async ({ page }) => {
  await page.goto('/scheduler')
  await expect(page.getByTestId('page-title')).toHaveText('Scheduler')

  // ── Create: interval-based File Sync ──────────────────────────────────
  await page.getByRole('button', { name: 'Add Schedule' }).click()

  // Task type defaults to File Sync — verify the card is selected
  await expect(page.getByRole('button', { name: /File Sync/i })).toHaveAttribute(
    'aria-pressed',
    'true'
  )

  // Fill interval: 2 hours
  const intervalInput = page.getByLabel('Interval value')
  await intervalInput.fill('2')
  await page.getByLabel('Interval unit').selectOption('hours')

  await page.getByRole('button', { name: 'Create Schedule' }).click()
  await expectToast(page, 'Schedule created')

  // Verify the row appears with human-friendly text
  const table = page.getByTestId('scheduler-table')
  const row = table.locator('tr').filter({ hasText: 'File Sync' }).first()
  await expect(row).toBeVisible()
  await expect(row).toContainText('Every 2 hours')

  // ── Edit: change to 30 minutes ────────────────────────────────────────
  await row.getByRole('button', { name: 'Edit' }).click()

  // Task type cards should be disabled during edit
  await expect(page.getByRole('button', { name: /File Sync/i })).toBeDisabled()

  const editIntervalInput = page.getByLabel('Interval value')
  await editIntervalInput.fill('30')
  await page.getByLabel('Interval unit').selectOption('minutes')

  await page.getByRole('button', { name: 'Save Changes' }).click()
  await expectToast(page, 'Schedule updated')

  await expect(row).toContainText('Every 30 minutes')

  // ── Delete ────────────────────────────────────────────────────────────
  await row.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click()
  await expectToast(page, 'Schedule deleted')
  await expect(row).toHaveCount(0)
})

test('creates a cron-based integrity check schedule using a preset', async ({ page }) => {
  await page.goto('/scheduler')

  await page.getByRole('button', { name: 'Add Schedule' }).click()

  // Select Integrity Check
  await page.getByRole('button', { name: /Integrity Check/i }).click()
  await expect(page.getByRole('button', { name: /Integrity Check/i })).toHaveAttribute(
    'aria-pressed',
    'true'
  )

  // Switch to Cron Schedule
  await page.getByRole('button', { name: 'Cron Schedule' }).click()

  // Pick a preset — target the "Daily at 02:00" / "Daily at 2:00 AM" button (the 0 2 * * * cron)
  const presetButtons = page.getByRole('button', { name: /Daily at/ })
  await presetButtons.last().click()

  await page.getByRole('button', { name: 'Create Schedule' }).click()
  await expectToast(page, 'Schedule created')

  const table = page.getByTestId('scheduler-table')
  const row = table.locator('tr').filter({ hasText: 'Integrity Check' }).first()
  await expect(row).toBeVisible()
  await expect(row).toContainText(/Daily at/)

  // Cleanup
  await row.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click()
  await expectToast(page, 'Schedule deleted')
})

test('creates a schedule with a custom cron expression', async ({ page }) => {
  await page.goto('/scheduler')

  await page.getByRole('button', { name: 'Add Schedule' }).click()

  // Switch to Cron Schedule
  await page.getByRole('button', { name: 'Cron Schedule' }).click()

  // Click Custom
  await page.getByRole('button', { name: /Custom/ }).click()

  // Type custom cron expression
  await page.getByLabel('Custom cron expression').fill('15 3 * * 1-5')

  await page.getByRole('button', { name: 'Create Schedule' }).click()
  await expectToast(page, 'Schedule created')

  const table = page.getByTestId('scheduler-table')
  const row = table.locator('tr').filter({ hasText: 'File Sync' }).first()
  await expect(row).toBeVisible()
  await expect(row).toContainText('Cron: 15 3 * * 1-5')

  // Cleanup
  await row.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click()
  await expectToast(page, 'Schedule deleted')
})

test('toggles a schedule between enabled and disabled', async ({ page }) => {
  await page.goto('/scheduler')

  // Create a schedule first
  await page.getByRole('button', { name: 'Add Schedule' }).click()
  const intervalInput = page.getByLabel('Interval value')
  await intervalInput.fill('6')
  await page.getByLabel('Interval unit').selectOption('hours')
  await page.getByRole('button', { name: 'Create Schedule' }).click()
  await expectToast(page, 'Schedule created')

  const table = page.getByTestId('scheduler-table')
  const row = table.locator('tr').filter({ hasText: 'File Sync' }).first()
  await expect(row).toBeVisible()

  // Disable
  await row.getByRole('button', { name: 'Enabled' }).click()
  await expectToast(page, 'Schedule disabled')
  await expect(row.getByRole('button', { name: 'Disabled' })).toBeVisible()

  // Re-enable
  await row.getByRole('button', { name: 'Disabled' }).click()
  await expectToast(page, 'Schedule enabled')
  await expect(row.getByRole('button', { name: 'Enabled' })).toBeVisible()

  // Cleanup
  await row.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click()
  await expectToast(page, 'Schedule deleted')
})
