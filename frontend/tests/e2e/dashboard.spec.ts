import { test, expect } from './support/fixtures'
import { expectToast } from './support/ui'

test('loads dashboard status and runs the core quick actions against the live backend', async ({
  page,
  qemu,
}) => {
  const fixture = await qemu.seedDriveFixture()
  await qemu.runBitProtector([
    'database',
    'add',
    fixture.backupPath,
    '--drive-label',
    `${fixture.runId}-backup`,
  ])

  await page.goto('/dashboard')
  await expect(page).toHaveURL(/\/dashboard$/)
  await expect(page.getByTestId('status-metric-files-tracked')).toBeVisible()
  await expect(page.getByTestId('quick-action-sync')).toBeVisible()

  await page.getByTestId('quick-action-sync').click()
  await expectToast(page, /Sync queue processed/)

  await page.getByTestId('quick-action-integrity').click()
  await expectToast(page, /Integrity run #\d+ started/)

  await page.getByTestId('quick-action-backup').click()
  await expectToast(page, /Database backup completed|Backup completed with/)
})
