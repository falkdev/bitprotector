# Testing Overview

This folder is the home for all testing documentation in BitProtector. Each document covers a distinct layer of the test suite — what it tests, why that layer exists, and how the tests are structured.

---

## Why We Test This Way

BitProtector manages irreplaceable data: file mirrors, checksums, and failover state. A bug in the mirror logic, the integrity engine, or the drive-replacement workflow can silently corrupt the user's backup. The test suite is therefore layered so that each layer catches a different class of problem at the lowest possible cost.

The philosophy is **layered confidence**:

- **Unit tests** verify individual functions and modules in isolation. They run in milliseconds and give immediate signal on logic errors.
- **Integration tests** verify that the compiled binary and the REST API behave correctly end-to-end, against a real SQLite database, without mocking the storage layer.
- **Frontend component tests** verify that each UI page renders the right controls, calls the right API endpoints, and handles error and empty states gracefully, using mocked network responses.
- **Frontend E2E tests** drive a real browser against a running QEMU guest and verify that complete user workflows work from click to confirmed side-effect.
- **QEMU installation tests** verify the full packaged system — `.deb` install, `systemd` service, TLS, PAM, reboot persistence, hardware failover, and long-running scheduled operations — in an ephemeral virtual machine identical to the production target.

Each layer gates the next in CI: cheap tests must pass before expensive tests run.

---

## Test Layer Pyramid

```text
                ┌──────────────────────┐
                │   QEMU Installation  │  ← full system, real hardware simulation
                ├──────────────────────┤
                │  Frontend E2E        │  ← browser + live backend
                ├──────────────────────┤
                │  Frontend Unit       │  ← component rendering + MSW API mocks
                ├──────────────────────┤
                │  Integration (Rust)  │  ← binary + REST API against real SQLite
                ├──────────────────────┤
                │  Unit (Rust)         │  ← pure logic, isolated modules
                └──────────────────────┘
```

Layers are ordered from fastest and cheapest (bottom) to slowest and most realistic (top). The CI pipeline runs them in this order and stops on the first failure.

---

## Document Index

| Document | What it covers |
| --- | --- |
| [running.md](running.md) | How to run every test category locally, including frontend and QEMU |
| [unit-tests.md](unit-tests.md) | Rust inline unit tests: which modules, what they verify, and the mocking strategy |
| [integration/README.md](integration/README.md) | How Rust integration tests are structured, the two test harnesses, and the isolation strategy |
| [integration/cli.md](integration/cli.md) | Coverage and rationale for each CLI integration test file |
| [integration/api.md](integration/api.md) | Coverage and rationale for each REST API integration test file |
| [integration/core.md](integration/core.md) | Core mechanics tests, scaling budget tests, checksum strategy tests, and packaging artifact tests |
| [frontend/README.md](frontend/README.md) | Frontend test toolchain overview: Vitest, MSW, Playwright, and the QEMU fixture |
| [frontend/unit.md](frontend/unit.md) | Coverage for each Vitest page and component test file |
| [frontend/e2e.md](frontend/e2e.md) | Coverage for each Playwright E2E spec and the user workflow it validates |
| [installation/README.md](installation/README.md) | QEMU infrastructure: bundles, scenarios, shared helpers, prerequisites, and setup |
| [installation/bundles.md](installation/bundles.md) | Scenario-by-scenario coverage for all QEMU test bundles |
| [installation/qemu-upgrade-db-coverage-report.md](installation/qemu-upgrade-db-coverage-report.md) | Upgrade database compatibility investigation, risk matrix, and implementation tracking |

---

## Relationship to CI

The CI pipeline mirrors the layer structure above. For details on how each layer maps to GitHub Actions jobs, how to reproduce a failure locally, and how to trigger the full suite manually, see [../CI.md](../CI.md).
