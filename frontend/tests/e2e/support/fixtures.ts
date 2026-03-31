import { test as base, expect } from '@playwright/test'
import { createQemuContext, createRunId, type QemuContext } from './qemu'

interface Fixtures {
  runId: string
  qemu: QemuContext
}

export const test = base.extend<Fixtures>({
  // eslint-disable-next-line no-empty-pattern
  runId: async ({}, runFixture, testInfo) => {
    await runFixture(createRunId(testInfo))
  },
  qemu: async ({ runId }, runFixture, testInfo) => {
    const qemu = createQemuContext(runId)

    await runFixture(qemu)

    if (testInfo.status !== testInfo.expectedStatus) {
      try {
        const diagnostics = await qemu.diagnostics()
        await testInfo.attach('qemu-service-diagnostics.txt', {
          body: diagnostics,
          contentType: 'text/plain',
        })
      } catch (error) {
        await testInfo.attach('qemu-service-diagnostics.txt', {
          body: `Failed to collect diagnostics: ${String(error)}`,
          contentType: 'text/plain',
        })
      }
    }

    try {
      await qemu.cleanup()
    } catch (error) {
      await testInfo.attach('qemu-cleanup.txt', {
        body: `Cleanup failed: ${String(error)}`,
        contentType: 'text/plain',
      })
    }
  },
})

export { expect }
