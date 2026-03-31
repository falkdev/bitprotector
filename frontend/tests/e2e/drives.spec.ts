import { test, expect } from './support/fixtures'
import { createDrivePair, driveCardByName, expectToast } from './support/ui'

test('creates, edits, and deletes a drive pair through the GUI', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()

  const createdCard = await createDrivePair(page, {
    name: fixture.driveName,
    primaryPath: fixture.primaryPath,
    secondaryPath: fixture.secondaryPath,
  })

  await createdCard.getByRole('button', { name: 'Edit' }).click()
  await page.getByTestId('drive-name-input').fill(fixture.updatedDriveName)
  await page.getByRole('button', { name: 'Update' }).click()

  const updatedCard = driveCardByName(page, fixture.updatedDriveName)
  await expect(updatedCard).toBeVisible()

  await updatedCard.getByRole('button', { name: 'Delete' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click()
  await expectToast(page, `Drive pair "${fixture.updatedDriveName}" deleted`)
  await expect(updatedCard).toHaveCount(0)
})

test('runs the replacement workflow against the live backend', async ({ page, qemu }) => {
  const fixture = await qemu.seedDriveFixture()

  const card = await createDrivePair(page, {
    name: fixture.driveName,
    primaryPath: fixture.primaryPath,
    secondaryPath: fixture.secondaryPath,
  })

  await card.getByRole('button', { name: 'Replace' }).click()
  await page.getByTestId('mark-replacement-button').click()
  await expectToast(page, 'Marked for replacement')
  await page.getByTestId('close-replacement-workflow').click()

  await expect(card).toContainText('P: quiescing')

  await card.getByRole('button', { name: 'Replace' }).click()
  await page.getByTestId('confirm-failure-button').click()
  await expectToast(page, 'Failure confirmed')
  await page.getByTestId('close-replacement-workflow').click()

  await expect(card).toContainText('P: failed')

  await card.getByRole('button', { name: 'Replace' }).click()
  await page.getByTestId('assign-path-input').fill(fixture.replacementPrimaryPath)
  await page.getByTestId('assign-replacement-button').click()
  await expectToast(page, 'Replacement drive assigned')
  await page.getByTestId('close-replacement-workflow').click()

  await expect(card).toContainText(fixture.replacementPrimaryPath)
})
