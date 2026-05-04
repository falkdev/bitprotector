import { test, expect } from './support/fixtures'
import { expectToast } from './support/ui'

test('manages database backups from the web UI', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()

  await page.goto('/database')
  await expect(page).toHaveURL(/\/database$/)
  await expect(page.getByTestId('page-title')).toHaveText('Database Backups')

  await page.getByRole('button', { name: 'Add Destination' }).click()
  await page.getByRole('button', { name: 'Browse' }).click()
  await page.getByLabel('Selected Path').fill(fixture.backupPath)
  await page.getByRole('button', { name: 'Use Backup Folder' }).click()
  await page.getByLabel('Drive Label').fill(`${fixture.runId}-backup`)
  await page.getByRole('button', { name: 'Create Destination' }).click()
  await expectToast(page, 'Backup destination created')
  await expect(page.getByTestId('database-backups-table')).toContainText(fixture.backupPath)

  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Enable automatic backups' }).click()
  await page.getByLabel('Automatic backups interval value').fill('1')
  await page.getByLabel('Automatic backups interval unit').selectOption('hours')
  await page.getByRole('button', { name: 'Enable automatic integrity checks' }).click()
  await page.getByLabel('Automatic integrity checks interval value').fill('2')
  await page.getByLabel('Automatic integrity checks interval unit').selectOption('hours')
  await page.getByRole('button', { name: 'Save Settings' }).click()
  await expectToast(page, 'Backup settings updated')

  await page.getByRole('button', { name: 'Run Backup Now' }).click()
  await expectToast(page, /Backed up to|Backup completed with/)
  expect(await qemu.pathExists(`${fixture.backupPath}/bitprotector.db`)).toBe(true)

  await page.getByRole('button', { name: 'Check Integrity Now' }).click()
  await expectToast(page, /Backup integrity check completed|Integrity check found/)
  await expect(page.getByText('Latest Integrity Check')).toBeVisible()

  await page.getByRole('button', { name: 'Restore Older Backup' }).click()
  await page.getByRole('button', { name: 'Browse' }).click()
  await page.getByLabel('Selected Path').fill(`${fixture.backupPath}/bitprotector.db`)
  await page.getByRole('button', { name: 'Use Backup File' }).click()
  await page.getByRole('button', { name: 'Stage Restore' }).click()
  await expectToast(page, 'Restore staged; restart BitProtector to apply it')
  await expect(page.getByText('Restore Staged')).toBeVisible()
})
