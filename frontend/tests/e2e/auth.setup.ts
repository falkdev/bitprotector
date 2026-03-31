import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { test as setup, expect } from '@playwright/test'
import { loginThroughUi } from './support/ui'

const dirname = path.dirname(fileURLToPath(import.meta.url))
const authFile = path.resolve(dirname, '../../playwright/.auth/testuser.json')

setup('authenticate against the manual QEMU backend', async ({ page }) => {
  fs.mkdirSync(path.dirname(authFile), { recursive: true })

  await loginThroughUi(page)
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()

  await page.context().storageState({ path: authFile })
})
