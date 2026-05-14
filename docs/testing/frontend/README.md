# Frontend Tests — Overview

This folder covers frontend testing for the BitProtector web UI. The frontend has two test layers: Vitest component tests and Playwright end-to-end tests.

---

## Table of Contents

- [Two Frontend Test Layers](#two-frontend-test-layers)
- [Vitest and React Testing Library](#vitest-and-react-testing-library)
- [API Mocking with MSW](#api-mocking-with-msw)
- [Factory Helpers](#factory-helpers)
- [Render Helper](#render-helper)
- [Playwright and the QEMU Fixture](#playwright-and-the-qemu-fixture)
- [Documents in This Folder](#documents-in-this-folder)

---

## Two Frontend Test Layers

**Component and unit tests** (`frontend/src/**/*.test.tsx`) use Vitest and React Testing Library to render individual pages and components in a JSDOM environment with mocked API responses. These tests run in milliseconds, require no backend, and cover UI logic, rendering correctness, user interactions, error states, and edge cases.

**End-to-end tests** (`frontend/tests/e2e/*.spec.ts`) use Playwright to drive a real browser against a running QEMU guest. These tests follow complete user workflows from login through to a confirmed side-effect on the guest (e.g., checking that a file actually exists at the expected secondary path after being mirrored). They are slower and require a running VM, but they validate the full integration between the browser, the backend, and the filesystem.

---

## Vitest and React Testing Library

Component tests use [Vitest](https://vitest.dev/) as the test runner and [@testing-library/react](https://testing-library.com/docs/react-testing-library/intro/) for rendering and interaction.

React Testing Library intentionally discourages testing implementation details. Tests find elements by accessible roles, labels, and test IDs rather than CSS selectors or component class names. This makes tests more resilient to refactoring and more closely aligned with how users interact with the UI.

Interactions are performed through [userEvent](https://testing-library.com/docs/user-event/intro), which simulates real browser events (clicks, keyboard input, focus) in the correct sequence rather than firing synthetic events directly. This catches issues like inputs that require a specific focus-before-type order.

---

## API Mocking with MSW

Component tests do not make real network requests. Instead, [Mock Service Worker (MSW)](https://mswjs.io/) intercepts `fetch` calls at the network level and returns configured responses.

MSW is configured in `frontend/src/test/msw/server.ts`. Each test file adds handlers using `server.use(...)` at the start of each test case. Handlers are registered for the duration of that test and then removed, so there is no cross-test pollution from leftover mock handlers.

The `api` helper in `frontend/src/test/msw/http.ts` wraps MSW's `http.get`, `http.post`, etc. with the `/api/v1` prefix automatically, so test handlers can be written against path-only strings (e.g., `api.get('/drives', ...)`) without repeating the prefix.

The benefit of MSW over manual `fetch` mocking is that the same handlers can be reused between tests and between the test environment and development storybook tools. MSW intercepts at the service worker or Node.js level, so the application code itself never changes — it calls `fetch` exactly as it would in production.

---

## Factory Helpers

`frontend/src/test/factories.ts` provides typed factory functions for every domain object the API returns: `makeDrivePair()`, `makeTrackedFile()`, `makeIntegrityRun()`, `makeLogEntry()`, and so on.

Each factory returns a complete object with sensible defaults and accepts a `Partial<T>` override to customise specific fields. Tests only specify the fields relevant to the scenario being tested, keeping test code concise and focused.

Using factories rather than inline object literals prevents tests from becoming brittle when a new required field is added to a type — the factory provides the default, and only tests that care about the new field need to be updated.

---

## Render Helper

`frontend/src/test/render.tsx` exports `renderWithApp()`, a thin wrapper around React Testing Library's `render` that wraps the component under test in a `MemoryRouter` and a `Toaster`. This gives every component test the routing context and toast notification infrastructure it needs without each test having to configure it manually.

An optional `route` parameter sets the initial URL in the memory router, which is useful for testing components that behave differently based on the current route.

---

## Playwright and the QEMU Fixture

E2E tests require a running QEMU guest. The guest is typically started with `./scripts/qemu_manual.sh` for local development or is provisioned automatically in CI as part of the test pipeline.

`frontend/tests/e2e/support/fixtures.ts` extends Playwright's base test fixture with two additions:

- **`runId`**: A unique string derived from the test title and a random UUID. Used to name drive pairs, directories, and files so that parallel tests do not interfere with each other on the shared VM.
- **`qemu`**: A `QemuContext` object that provides helpers for interacting with the guest over SSH and verifying state on the guest filesystem.

The `QemuContext` (in `frontend/tests/e2e/support/qemu.ts`) provides:

- `seedDriveFixture()`: Creates the directory structure and test files on the guest that a test will use. Returns a fixture object with all the paths pre-computed as typed fields. This is called at the start of tests that need drive pairs and tracked files to already exist.
- `runBitProtector(args)`: Runs a CLI command on the guest over SSH and returns stdout/stderr.
- `resolvePath(path)`: Resolves a path on the guest filesystem using `readlink -f` and returns the canonical absolute path. Used when tests need to normalise symlinked or relative paths before asserting.
- `pathExists(path)`: Checks whether a path exists on the guest filesystem. Used after mirror operations to confirm the file was actually written.
- `readFile(path)`: Reads the content of a file on the guest. Used to verify the content of mirrored files.
- `diagnostics()`: Returns service logs and status information from the guest. Automatically attached to failed test results for debugging.
- `cleanup()`: Removes the test-specific directories from the guest. Called after the test regardless of outcome via Playwright's fixture teardown.

If a test fails, Playwright attaches the diagnostic output to the test report, giving a clear view of the guest's service state at the time of failure without needing to SSH into the VM manually.

---

## Documents in This Folder

| Document | What it covers |
| --- | --- |
| [unit.md](unit.md) | Every Vitest page and component test file: what it tests and why |
| [e2e.md](e2e.md) | Every Playwright E2E spec file: the user workflow it validates end-to-end |
