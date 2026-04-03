import { test, expect } from './support/fixtures'
import { createDrivePair, expectToast, openSidebarRoute } from './support/ui'

test('tracks, mirrors, sets virtual path, and deletes a file through the GUI', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()
  const fileName = fixture.fileRelativePath.split('/').at(-1) ?? fixture.fileRelativePath

  await createDrivePair(page, {
    name: fixture.driveName,
    primaryPath: fixture.primaryPath,
    secondaryPath: fixture.secondaryPath,
  })

  await openSidebarRoute(page, 'files')
  await page.getByTestId('track-file-btn').click()
  await page.getByLabel('Drive pair').selectOption({ label: fixture.driveName })
  await page.getByLabel('File path').fill(fixture.absoluteFilePath)
  await page.getByRole('button', { name: 'Track file' }).last().click()
  await expectToast(page, 'File tracked')

  const row = page.locator('[data-testid^="file-row-"]').filter({ hasText: fileName }).first()
  await expect(row).toBeVisible()
  await row.click()
  await expect(page.getByTestId('file-details')).toContainText(fixture.fileRelativePath)

  await row.getByTestId('action-set-virtual-path').click()
  await page.getByLabel('Virtual Path').fill(fixture.virtualPath)
  await page.getByRole('button', { name: 'Save' }).click()
  await expectToast(page, 'Virtual path updated')

  const updatedRow = page
    .locator('[data-testid^="file-row-"]')
    .filter({ hasText: fixture.virtualPath })
    .first()
  await updatedRow.click()
  await expect(page.getByTestId('file-details')).toContainText(fixture.virtualPath)

  await updatedRow.getByTestId('action-mirror').click()
  await expectToast(page, 'Mirror requested')
  await expect(updatedRow).toContainText('Mirrored')
  await expect(await qemu.pathExists(fixture.secondaryFilePath)).toBe(true)
  await expect(await qemu.readFile(fixture.secondaryFilePath)).toContain(`report for ${fixture.runId}`)

  await updatedRow.getByTestId('action-delete').click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Confirm' }).click()
  await expectToast(page, 'File removed from tracking')
  await expect(updatedRow).toHaveCount(0)
  await expect(await qemu.pathExists(fixture.virtualPath)).toBe(false)
})
