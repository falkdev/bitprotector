import { fileURLToPath } from 'node:url'
import path from 'path'
import { defineConfig, devices } from '@playwright/test'

const dirname = path.dirname(fileURLToPath(import.meta.url))
const PLAYWRIGHT_HOST = process.env.FRONTEND_HOST ?? '127.0.0.1'
const PLAYWRIGHT_PORT = Number(process.env.FRONTEND_PORT ?? '4173')
const PLAYWRIGHT_BASE_URL = `http://${PLAYWRIGHT_HOST}:${PLAYWRIGHT_PORT}`
const QEMU_API_HOST = process.env.QEMU_API_HOST ?? 'localhost'
const QEMU_API_PORT = process.env.QEMU_API_PORT ?? '18443'
const QEMU_PROXY_TARGET =
  process.env.BITPROTECTOR_DEV_PROXY_TARGET ?? `https://${QEMU_API_HOST}:${QEMU_API_PORT}`
const FRONTEND_QEMU_MANUAL = path.resolve(dirname, '../scripts/frontend_qemu_manual.sh')
const AUTH_STATE_PATH = path.resolve(dirname, 'playwright/.auth/testuser.json')
const WEB_SERVER_TIMEOUT_MS = Number(process.env.PLAYWRIGHT_WEB_SERVER_TIMEOUT ?? '180000')

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [
    ['line'],
    ['html', { open: 'never' }],
  ],
  use: {
    baseURL: PLAYWRIGHT_BASE_URL,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'setup',
      testMatch: /auth\.setup\.ts/,
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'qemu-chromium',
      dependencies: ['setup'],
      testIgnore: /auth\.setup\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        storageState: AUTH_STATE_PATH,
      },
    },
  ],
  webServer: {
    command: `SKIP_NPM_CI=1 FRONTEND_HOST=${PLAYWRIGHT_HOST} FRONTEND_PORT=${PLAYWRIGHT_PORT} BITPROTECTOR_DEV_PROXY_TARGET=${QEMU_PROXY_TARGET} bash ${FRONTEND_QEMU_MANUAL}`,
    url: PLAYWRIGHT_BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: WEB_SERVER_TIMEOUT_MS,
  },
})
