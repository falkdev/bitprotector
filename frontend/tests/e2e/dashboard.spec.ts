import { test, expect } from './support/fixtures'

test('loads dashboard status metrics and recent activity', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()
  await qemu.runBitProtector([
    'drives',
    'add',
    fixture.driveName,
    fixture.primaryPath,
    fixture.secondaryPath,
  ])
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
  await expect(page.getByRole('heading', { name: 'Recent Activity' })).toBeVisible()
})
