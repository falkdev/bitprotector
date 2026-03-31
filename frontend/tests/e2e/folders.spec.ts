import { test, expect } from './support/fixtures'
import { createDrivePair, expectToast, openSidebarRoute } from './support/ui'

test('adds a tracked folder and scans it against the live backend', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()

  await createDrivePair(page, {
    name: fixture.driveName,
    primaryPath: fixture.primaryPath,
    secondaryPath: fixture.secondaryPath,
  })

  await openSidebarRoute(page, 'folders')
  await page.getByTestId('add-folder-button').click()
  await page.getByLabel('Drive Pair').selectOption({ label: fixture.driveName })
  await page.getByLabel('Folder Path').fill(fixture.folderRelativePath)
  await page.getByRole('button', { name: 'Add Folder' }).last().click()
  await expectToast(page, 'Folder added')

  const row = page.locator('[data-testid^="folder-row-"]').filter({ hasText: fixture.folderRelativePath }).first()
  await expect(row).toBeVisible()

  await row.getByRole('button', { name: 'Scan' }).click()
  await expectToast(page, /Scan complete:/)
  await expect(page.getByText(new RegExp(`Scan results for ${fixture.folderRelativePath}:`))).toBeVisible()
})
