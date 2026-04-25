# Test Coverage Implementation Progress

> Companion to [TEST_COVERAGE_PLAN.md](TEST_COVERAGE_PLAN.md). This file tracks what is implemented on branch `improve-test-coverage` and serves as the resume checkpoint for future sessions.
>
> Last updated: 2026-04-23

---

## Decisions carried into implementation

| # | Decision | Choice |
|---|---|---|
| Q6 | Cloud-init strategy | Inline heredocs (no separate YAML files) |
| Q7 | `integrity_runs.rs` test strategy | TempDir + real files (no mockall) |
| Q8 | `api_routes.rs` split depth | 10 files (drives/files/folders/virtual_paths/integrity/scheduler/sync/logs/database/residual routes) |
| Q1 | alpha1 `.deb` for upgrade bundle | Build from tag `v1.0.0-alpha1` in CI and pass as `ALPHA1_DEB` |
| Q2 | resilience panic trigger | `sudo kill -SEGV $(pidof bitprotector)` |
| Q3 | Coverage upload target | Artifacts only (no Codecov in this scope) |
| Q5 | Ubuntu 26.04 behavior | Keep existing `continue-on-error` behavior for provisional image |

---

## Phase status

| Phase | Status | Deliverables |
|---|---|---|
| 1 — QEMU harness refactor | **DONE** | Bundle/scenario architecture, shared scenario helpers, compatibility wrappers |
| 2 — Log upload on CI failure | **DONE** | QEMU jobs upload serial/QEMU logs on failure |
| 3 — Journal error scraper | **DONE** | `journal_error_scraper` added and wired at bundle end |
| 4 — Smoke bundle expansion | **DONE** | `smoke-02`, `smoke-05..12`, TLS/PAM cloud-init, reboot persistence helper |
| 5 — Failover bundle expansion | **DONE** | `failover-03..12`, XFS replacement primary, `xfsprogs` install, full wiring |
| 6 — Resilience bundle | **DONE** | New bundle + `resilience-01..08`, QMP baseline/restore, CI job |
| 7 — Upgrade bundle | **DONE** | New bundle + `upgrade-01/02`, alpha1 build path in CI, CI job |
| 8 — Uninstall +1 | **DONE** | `uninstall-04-purge-preserves-user-drive-data` + bundle wiring |
| 9 — Degraded-boot bundle | **DONE** | New bundle + degraded-boot scenarios + CI job |
| 10 — Scale bundles | **DONE** | `scale`, `scale_lowmem` bundles + scenarios + nightly jobs |
| 11 — Rust inline unit tests | **DONE** | Added/extended tests in `integrity_runs`, `path_resolution`, `main`, `repository` |
| 12 — Split `api_routes.rs` | **DONE** | Split into 10 integration files + shared `tests/integration/common/mod.rs` |
| 13 — Property-based tests | **DONE** | `proptest` in `virtual_path.rs` and `path_resolution.rs` |
| 14 — Frontend API client tests | **DONE** | Added tests for all previously untested API modules |
| 15 — Coverage reporting | **DONE** | Non-gating `coverage` CI job + docs updates in `TESTING.md` and `CI.md` |

---

## Implementation details by phase

### Phase 1-4 (QEMU smoke + shared harness)

- Added shared libraries:
  - [tests/installation/lib/scenarios.sh](../tests/installation/lib/scenarios.sh)
  - [tests/installation/lib/snapshots.sh](../tests/installation/lib/snapshots.sh)
- Added bundle entrypoints:
  - [tests/installation/bundles/smoke.sh](../tests/installation/bundles/smoke.sh)
  - [tests/installation/bundles/failover.sh](../tests/installation/bundles/failover.sh)
  - [tests/installation/bundles/uninstall.sh](../tests/installation/bundles/uninstall.sh)
- Kept compatibility wrappers (purpose unchanged):
  - [tests/installation/qemu_test.sh](../tests/installation/qemu_test.sh)
  - [tests/installation/qemu_failover_test.sh](../tests/installation/qemu_failover_test.sh)
  - [tests/installation/qemu_uninstall_test.sh](../tests/installation/qemu_uninstall_test.sh)
- Smoke expansion completed:
  - Added [tests/installation/scenarios/smoke/smoke-12-reboot-persistence.sh](../tests/installation/scenarios/smoke/smoke-12-reboot-persistence.sh)
  - Added `wait_for_reboot_and_ssh` in `scenarios.sh`
  - Wired `smoke-02` and `smoke-05..smoke-12`
  - Expanded smoke cloud-init with TLS cert/key generation, TLS config, `testauth/hunter2`, and tooling (`jq`, `openssl`, `curl`)

### Phase 5-10 (remaining QEMU bundles/scenarios)

- Failover:
  - Added `failover-03..12` under `tests/installation/scenarios/failover/`
  - Updated failover provisioning to format `bpreplprimary` as XFS and install `xfsprogs`
- Uninstall:
  - Added [tests/installation/scenarios/uninstall/uninstall-04-purge-preserves-user-drive-data.sh](../tests/installation/scenarios/uninstall/uninstall-04-purge-preserves-user-drive-data.sh)
- New bundles:
  - [tests/installation/bundles/resilience.sh](../tests/installation/bundles/resilience.sh)
  - [tests/installation/bundles/upgrade.sh](../tests/installation/bundles/upgrade.sh)
  - [tests/installation/bundles/degraded_boot.sh](../tests/installation/bundles/degraded_boot.sh)
  - [tests/installation/bundles/scale.sh](../tests/installation/bundles/scale.sh)
  - [tests/installation/bundles/scale_lowmem.sh](../tests/installation/bundles/scale_lowmem.sh)
- New scenario groups:
  - `tests/installation/scenarios/resilience/resilience-01..08`
  - `tests/installation/scenarios/upgrade/upgrade-01..02`
  - `tests/installation/scenarios/degraded-boot/degraded-boot-01..02`
  - `tests/installation/scenarios/scale/scale-01..02`
  - `tests/installation/scenarios/scale-lowmem/scale-lowmem-01`

### Phase 11-13 (Rust tests)

- Added `resolve_db_path` extraction + tests in [src/main.rs](../src/main.rs)
- Expanded inline coverage in:
  - [src/core/integrity_runs.rs](../src/core/integrity_runs.rs)
  - [src/api/path_resolution.rs](../src/api/path_resolution.rs)
  - [src/db/repository.rs](../src/db/repository.rs)
- Added `proptest = "1"` dev dependency in [Cargo.toml](../Cargo.toml)
- Added property tests in:
  - [src/core/virtual_path.rs](../src/core/virtual_path.rs)
  - [src/api/path_resolution.rs](../src/api/path_resolution.rs)

### Phase 12 (integration split)

- Added shared integration common module:
  - [tests/integration/common/mod.rs](../tests/integration/common/mod.rs)
- Split API integration surface into:
  - [tests/integration/api_drives.rs](../tests/integration/api_drives.rs)
  - [tests/integration/api_files.rs](../tests/integration/api_files.rs)
  - [tests/integration/api_folders.rs](../tests/integration/api_folders.rs)
  - [tests/integration/api_virtual_paths.rs](../tests/integration/api_virtual_paths.rs)
  - [tests/integration/api_integrity.rs](../tests/integration/api_integrity.rs)
  - [tests/integration/api_scheduler.rs](../tests/integration/api_scheduler.rs)
  - [tests/integration/api_sync.rs](../tests/integration/api_sync.rs)
  - [tests/integration/api_logs.rs](../tests/integration/api_logs.rs)
  - [tests/integration/api_database.rs](../tests/integration/api_database.rs)
  - [tests/integration/api_routes.rs](../tests/integration/api_routes.rs) (residual)
- Updated test binary wiring in [Cargo.toml](../Cargo.toml), [scripts/run-tests.sh](../scripts/run-tests.sh), and [.github/workflows/ci.yml](../.github/workflows/ci.yml)

### Phase 14-15 (frontend + coverage)

- Added frontend API tests:
  - `frontend/src/api/{auth,database,drives,files,folders,integrity,logs,scheduler,status,sync,tracking,virtual-paths}.test.ts`
- CI updates:
  - Added non-gating `coverage` job in [.github/workflows/ci.yml](../.github/workflows/ci.yml)
  - Added `qemu-resilience`, `qemu-upgrade`, `qemu-degraded-boot` jobs in CI
  - Added nightly-only `qemu-scale`, `qemu-scale-lowmem` in [.github/workflows/nightly.yml](../.github/workflows/nightly.yml)
- Docs updates:
  - [docs/TESTING.md](TESTING.md)
  - [docs/CI.md](CI.md)

---

## Validation run log (this checkpoint)

Completed locally:

1. Shell syntax:
   - `find tests/installation -type f -name '*.sh' -print0 | xargs -0 -n1 bash -n`
2. Rust quality gates:
   - `cargo fmt --check`
   - `cargo clippy -- -D warnings`
   - `cargo test --lib`
3. Split API integration binaries:
   - `cargo test --test api_drives`
   - `cargo test --test api_files`
   - `cargo test --test api_folders`
   - `cargo test --test api_virtual_paths`
   - `cargo test --test api_integrity`
   - `cargo test --test api_scheduler`
   - `cargo test --test api_sync`
   - `cargo test --test api_logs`
   - `cargo test --test api_database`
   - `cargo test --test api_routes`
4. Frontend:
   - `cd frontend && npm run lint` (warnings only in `TrackingWorkspacePage.tsx`, no errors)
   - `cd frontend && npm test` (all tests passed)

Not run in this local checkpoint:

- Full QEMU bundle matrix on Ubuntu 24.04/26.04
- GitHub Actions branch run validation for the updated workflow graph/artifact uploads

---

## Resume guidance for next session

- If continuing from this checkpoint, start by running a CI branch build to verify the new QEMU jobs and nightly scale jobs in GitHub Actions.
- If CI flags performance or provisioning instability, adjust bundle timeouts/cloud-init package provisioning before broad refactors.
