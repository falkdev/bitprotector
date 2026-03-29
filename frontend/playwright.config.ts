import { defineConfig, devices } from '@playwright/test'

const PLAYWRIGHT_HOST = '127.0.0.1'
const PLAYWRIGHT_PORT = 4173
const PLAYWRIGHT_BASE_URL = `http://${PLAYWRIGHT_HOST}:${PLAYWRIGHT_PORT}`

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: PLAYWRIGHT_BASE_URL,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'firefox',
      use: { ...devices['Desktop Firefox'] },
    },
  ],
  webServer: {
    command: `npm run dev -- --host ${PLAYWRIGHT_HOST} --port ${PLAYWRIGHT_PORT} --strictPort`,
    url: PLAYWRIGHT_BASE_URL,
    reuseExistingServer: !process.env.CI,
  },
})
