import { test, expect } from './support/fixtures'
import { createDrivePair, expectToast, openSidebarRoute } from './support/ui'

test('starts an integrity run and shows issue-only results without hanging the UI', async ({
  page,
  qemu,
}) => {
  const fixture = await qemu.seedDriveFixture()

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

  await openSidebarRoute(page, 'integrity')
  await page.getByRole('button', { name: 'Run Check' }).click()
  await expect(page.getByText('Start Integrity Run')).toBeVisible()
  await page.getByLabel('Drive Pair').selectOption({ label: fixture.driveName })
  await page.getByLabel('Attempt automatic recovery').uncheck()

  await page.getByRole('button', { name: 'Start' }).click()
  await expectToast(page, /Integrity run #\d+ started/)
  await expect(page.getByText(/Integrity check running/)).toBeVisible()

  const expectedIssueRow = page
    .locator('[data-testid^="integrity-row-"]')
    .filter({ hasText: fixture.fileRelativePath })
    .first()
  await expect(expectedIssueRow).toBeVisible({ timeout: 30_000 })
})
