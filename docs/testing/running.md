# Running Tests

This document covers how to run every test category locally. For CI pipeline details, job ordering, and how to reproduce a specific CI failure, see [../CI.md](../CI.md).

---

## Table of Contents

- [Prerequisites](#prerequisites)
- [Rust Tests](#rust-tests)
- [Frontend Tests](#frontend-tests)
- [QEMU Installation Tests](#qemu-installation-tests)
- [Full Pipeline Locally](#full-pipeline-locally)
- [Coverage](#coverage)

---

## Prerequisites

| Requirement | Minimum version / notes |
| --- | --- |
| Rust toolchain | Stable, with `cargo` |
| Node.js | 20.19 or later (required by the Vite/Vitest stack) |
| QEMU tools | Only needed for installation tests — see [installation/README.md](installation/README.md) |

---

## Rust Tests

### Run the entire Rust test suite

```bash
cargo test
```

This runs all inline unit tests and all integration test files together.

### Run only inline unit tests

The `--lib` flag targets only the `#[cfg(test)]` blocks embedded in `src/`:

```bash
cargo test --lib
```

### Run a single integration test file

Each file in `tests/integration/` can be addressed by name:

```bash
cargo test --test cli_drives
cargo test --test cli_auth
cargo test --test api_integrity
cargo test --test scaling_100k
cargo test --test packaging
```

### Run a single test by name

Cargo accepts a filter string that matches any substring of the test name:

```bash
cargo test test_drives_add_and_list
cargo test test_mirror_selects_correct_pair
```

### Show captured output

By default, output from passing tests is suppressed. To see `println!` and `eprintln!` output:

```bash
cargo test -- --nocapture
```

### Run tests serially

Tests run in parallel by default. Use `--test-threads=1` when ordering matters or when diagnosing flaky output:

```bash
cargo test -- --test-threads=1
```

---

## Frontend Tests

The frontend toolchain requires Node.js 20.19 or later.

### Install dependencies and verify a clean build

```bash
cd frontend
npm ci
npm run build
npm run lint
```

### Run Vitest unit and component tests

```bash
cd frontend
npm test
```

### Run all E2E tests against a local QEMU guest

First boot a guest with the manual QEMU script (see [installation/README.md](installation/README.md)), then:

```bash
cd frontend
npm run test:e2e:qemu
```

### Run specific E2E specs

Pass one or more spec file paths after `--`:

```bash
cd frontend
npm run test:e2e:qemu -- tests/e2e/auth-and-nav.spec.ts tests/e2e/integrity.spec.ts
npm run test:e2e -- tests/e2e/file-browser.spec.ts tests/e2e/folders.spec.ts
```

---

## QEMU Installation Tests

The installation tests require a built `.deb` package and a QEMU-compatible Ubuntu cloud image. Full setup instructions are in [installation/README.md](installation/README.md).

### Smoke bundle (fast baseline)

```bash
./tests/installation/qemu_test.sh
```

### Failover bundle

```bash
./tests/installation/qemu_failover_test.sh
```

### Uninstall bundle

```bash
./tests/installation/qemu_uninstall_test.sh
```

### Additional bundles

```bash
./tests/installation/bundles/application_workflows.sh
./tests/installation/bundles/resilience.sh
./tests/installation/bundles/upgrade.sh
./tests/installation/bundles/degraded_boot.sh
./tests/installation/bundles/drive_media_type.sh
```

Nightly-only bundles (resource-heavy; not run in standard CI):

```bash
./tests/installation/bundles/scale.sh
./tests/installation/bundles/scale_lowmem.sh
./tests/installation/bundles/scheduled_load.sh
```

### Port and timeout overrides

If another QEMU VM is already running on the default ports, set alternate values:

```bash
SSH_PORT=2224 API_PORT=18445 TIMEOUT=240 ./tests/installation/qemu_test.sh
SSH_PORT=2225 API_PORT=18446 TIMEOUT=360 ./tests/installation/qemu_failover_test.sh
```

---

## Full Pipeline Locally

The `run-tests.sh` script mirrors the CI layer structure without Docker:

```bash
./scripts/run-tests.sh fast    # lint + unit + Rust integration
./scripts/run-tests.sh smoke   # fast + .deb build + QEMU smoke
./scripts/run-tests.sh full    # smoke + application_workflows + failover + uninstall + resilience + upgrade + degraded-boot
```

To run through `act` (Docker-based, uses the same GitHub Actions YAML as CI):

```bash
./scripts/ci-local.sh lint
./scripts/ci-local.sh fast
./scripts/ci-local.sh smoke
./scripts/ci-local.sh full
```

---

## Coverage

Generate Rust and frontend coverage reports locally:

```bash
# Rust — outputs rust.lcov
cargo llvm-cov --lib --workspace --lcov --output-path rust.lcov

# Frontend — outputs frontend/coverage/
cd frontend && npm run test:coverage
```

The CI pipeline uploads both artifacts from the non-gating `coverage` job. They do not block merges.
