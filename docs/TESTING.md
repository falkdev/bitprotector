# Testing Guide

This document explains how to run the test suite, what each test category covers, how to write new unit tests, and how to run the QEMU-based installation, failover, and uninstall suites.

---

## Table of Contents

- [Test Layout](#test-layout)
- [Running Tests](#running-tests)
- [Running in CI](#running-in-ci)
- [Integration Tests](#integration-tests)
  - [CLI Integration Tests](#cli-integration-tests)
  - [Auth / API Integration Tests](#auth--api-integration-tests)
  - [Tracking Scale Integration Test (100k rows)](#tracking-scale-integration-test-100k-rows)
  - [Packaging Tests](#packaging-tests)
- [Unit Tests](#unit-tests)
- [QEMU Installation Tests](#qemu-installation-tests)

---

## Test Layout

```text
tests/
├── integration/          # Integration tests (Rust — cargo test)
│   ├── api_routes.rs         # REST API endpoint coverage (drives, files, auth, etc.)
│   ├── api_filesystem_browser.rs # Filesystem browse route coverage for the web path picker
│   ├── cli_auth.rs           # JWT middleware, token lifecycle, logout
│   ├── cli_drives.rs         # Drive pair CLI commands
│   ├── cli_files.rs          # File tracking CLI commands
│   ├── cli_folders.rs        # Tracked folder CLI commands
│   ├── cli_integrity.rs      # Integrity check CLI commands
│   ├── cli_logs.rs           # Event log CLI commands
│   ├── cli_sync.rs           # Sync queue CLI commands
│   ├── cli_status.rs         # SSH status display
│   ├── cli_virtual_paths.rs  # Virtual path CLI commands
│   ├── cli_database.rs       # Database backup CLI commands
│   ├── core_mirror.rs        # File mirroring and restore mechanics
│   ├── core_change_detection.rs  # File change detection and re-mirroring
│   ├── core_scheduler.rs     # Background task scheduling
│   ├── scaling_100k.rs       # 100k-row tracking listing/filtering performance budgets
│   └── packaging.rs          # Verifies packaging artifacts exist
└── installation/
    ├── qemu_test.sh          # Fast package/install smoke test on Ubuntu 24 via QEMU
    ├── qemu_failover_test.sh # Extra-disk failover/replacement end-to-end test via QEMU
    └── qemu_uninstall_test.sh # Full package purge/uninstall verification via QEMU
```

Inline unit tests (`#[cfg(test)]` blocks inside `src/`) are the primary home for unit-level testing.

Frontend tests live in:

- `frontend/src/**/*.test.tsx` for unit/component tests
- `frontend/tests/e2e/*.spec.ts` for Playwright end-to-end tests (including QEMU-targeted runs)

---

## Running Tests

### Run everything

```bash
cargo test
```

### Frontend checks

The frontend toolchain targets Node.js 20.19+.

```bash
cd frontend
npm ci
npm run build
npm run lint
npm test
```

`npm test` also expects Node.js 20.19+ because the current Vite/Vitest stack requires newer Node APIs.

To run the live frontend smoke suite against a manual QEMU guest that is already booted with `./scripts/qemu_manual.sh`:

```bash
cd frontend
npm run test:e2e:qemu
```

To run only auth/nav plus integrity live specs:

```bash
cd frontend
npm run test:e2e:qemu -- tests/e2e/auth-and-nav.spec.ts tests/e2e/integrity.spec.ts
```

To run only the tracking-focused live specs:

```bash
cd frontend
npm run test:e2e -- tests/e2e/file-browser.spec.ts tests/e2e/folders.spec.ts
```

### Run all tests in one integration file

```bash
cargo test --test cli_drives
cargo test --test cli_auth
cargo test --test scaling_100k
cargo test --test packaging
```

### Run inline unit tests only (from `src/` `#[cfg(test)]` blocks)

```bash
cargo test --lib
```

### Run a single test by name

```bash
cargo test test_drives_add_and_list
```

### Show `println!` / `eprintln!` output

```bash
cargo test -- --nocapture
```

### Run tests in parallel (default) or serially

```bash
# Serial — useful when tests share state or you need ordered output
cargo test -- --test-threads=1
```

---

## Running in CI

All test categories run automatically on GitHub Actions. The pipeline is layered and fail-fast — cheaper tests gate the expensive QEMU suites.

| Trigger | Layers |
| --- | --- |
| Pull request | Lint → unit → integration → build → QEMU smoke (Layers 0-5) |
| Push to `main` | Full suite including QEMU failover and uninstall (Layers 0-7) |
| Nightly cron (03:00 UTC) | Same as push to main |
| `workflow_dispatch` with `run_heavy_qemu=true` | Full suite from any branch |

To reproduce a CI run locally, see [docs/CI.md](CI.md).

To run the full pipeline natively (no Docker):

```bash
./scripts/run-tests.sh fast    # lint + unit + Rust integration
./scripts/run-tests.sh smoke   # + .deb build + QEMU smoke
./scripts/run-tests.sh full    # + QEMU failover + uninstall
```

To run through `act` (Docker-in-Docker, same YAML as CI):

```bash
./scripts/ci-local.sh lint
./scripts/ci-local.sh fast
./scripts/ci-local.sh smoke
./scripts/ci-local.sh full
```

---

## Integration Tests

### CLI Integration Tests

Files: `tests/integration/cli_*.rs` (except `cli_auth.rs`)

These tests invoke the compiled `bitprotector` binary through [`assert_cmd`](https://docs.rs/assert_cmd) and assert on `stdout`, `stderr`, and exit codes using [`predicates`](https://docs.rs/predicates).

**Key pattern — isolated database per test:**

Each test creates a temporary file with [`tempfile::NamedTempFile`](https://docs.rs/tempfile) and passes its path as `--db`. This prevents any shared mutable state between tests.

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

fn cmd(db: &str) -> Command {
    let mut c = Command::cargo_bin("bitprotector").unwrap();
    c.arg("--db").arg(db);
    c
}

fn temp_db() -> NamedTempFile {
    NamedTempFile::new().unwrap()
}

#[test]
fn test_drives_add_and_list() {
    let db = temp_db();
    // ... add drive pair ...
    cmd(db.path().to_str().unwrap())
        .args(["drives", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mybackup"));
}
```

**Creating real directories for path validation:**

When a command validates that paths exist (e.g., `drives add`), use `tempfile::TempDir`:

```rust
use tempfile::TempDir;

let primary = TempDir::new().unwrap();
let secondary = TempDir::new().unwrap();

cmd(db.path().to_str().unwrap())
    .args([
        "drives", "add", "backup",
        primary.path().to_str().unwrap(),
        secondary.path().to_str().unwrap(),
    ])
    .assert()
    .success();
```

Both `NamedTempFile` and `TempDir` are dropped at the end of the test, cleaning up all temporary files automatically.

The failover/replacement coverage currently lives in:

- `tests/integration/cli_drives.rs` for planned replacement workflows and rebuild completion via the CLI
- `tests/integration/cli_folders.rs` for active-secondary folder scanning and change detection
- `src/api/server.rs` (inline `#[cfg(test)]` module) for API route coverage of mark/cancel/confirm/assign

### Auth / API Integration Tests

File: `tests/integration/cli_auth.rs`

These tests exercise the JWT middleware inside the actix-web application using [`actix_web::test`](https://docs.rs/actix-web/latest/actix_web/test/index.html) instead of spawning the binary. The `JwtSecret` data extractor is injected through `.app_data()`, and `issue_token` / `validate_token` from `bitprotector_lib::api::auth` are used directly.

```rust
use bitprotector_lib::api::auth::{issue_token, validate_token, JwtSecret, JwtAuth};
use actix_web::{test, web, App, HttpResponse};

const SECRET: &[u8] = b"integration_test_secret_key";

#[actix_rt::test]
async fn test_auth_middleware_accepts_valid_token() {
    let token = issue_token("carol", SECRET, 3600).unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(JwtSecret(SECRET.to_vec())))
            .route("/secret", web::get().to(protected_handler)),
    ).await;

    let req = test::TestRequest::get()
        .uri("/secret")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
```

Note: these tests require `actix_rt` as a dev dependency. Annotate async test functions with `#[actix_rt::test]` instead of `#[tokio::test]`.

The broader API route integration file (`tests/integration/api_routes.rs`) now also covers tracking-workspace-specific semantics:

- track-file queue-first behavior (no immediate mirror copy)
- folder scan queue-first behavior (tracks + enqueues, no immediate mirror)
- immediate file/folder mirror actions and queue reconciliation
- effective virtual path derivation for folder-origin files
- virtual-prefix / has-virtual-path filtering correctness against effective virtual paths
- `source=both` rejection (`400`)
- folder aggregate status fields in mixed tracking responses

It also covers the async integrity-run lifecycle:

- `POST /integrity/runs` start behavior
- single active-run conflict (`409`)
- `GET /integrity/runs/active` progress polling shape
- `POST /integrity/runs/{id}/stop` cooperative stop behavior
- latest/per-run paged result endpoints with `issues_only=true`

### Filesystem Browser Integration Tests

File: `tests/integration/api_filesystem_browser.rs`

These tests cover the read-only filesystem route that powers the web UI path picker. They verify:

- default root browsing
- nested directory loading
- hidden-file toggle behavior
- invalid and unreadable path handling
- directory-only filtering for directory pickers

The tracked file/folder validation edge cases remain covered in `tests/integration/api_routes.rs`, where absolute-path submissions are accepted only when they resolve under the selected drive pair's active root.

### Tracking Scale Integration Test (100k rows)

File: `tests/integration/scaling_100k.rs`

This test seeds `100,000` tracked-file rows and validates the scaled tracking endpoint (`GET /api/v1/tracking/items`) for:

- server pagination behavior (including per-page cap to 200)
- virtual-path filtering and source filtering correctness (`source=direct|folder|all`)
- targeted search correctness
- query-duration budget checks for representative list/filter requests

The current budget enforced in the test is `3000 ms` per measured query path.

### Packaging Tests

File: `tests/integration/packaging.rs`

These Rust tests use only `std::fs` — no binary invocations. They verify that all packaging artifacts (systemd service file, default config, `profile.d` hook, QEMU install/failover/uninstall scripts, and maintainer scripts) exist and contain the required sections. Run them with:

```bash
cargo test --test packaging
```

---

## Unit Tests

Unit tests live as inline `#[cfg(test)]` modules at the bottom of each `src/` file. This keeps them next to the code they test and avoids additional file management.

Frontend component/unit tests now also cover the path-picker workflow in `frontend/src/**/*.test.tsx`, including:

- path normalization helpers
- lazy-loading behavior in the shared path picker dialog
- absolute-to-relative conversion for tracked file/folder submits
- absolute path fill behavior in drive configuration forms

Integrity and layout-focused frontend tests now cover:

- Integrity page bootstrap loading indicator while latest run data is fetched
- Integrity page intro rendering of `Last integrity check` timestamp
- start-run modal flow (drive-pair/all selection + recovery toggle)
- running banner/progress and stop transition behavior
- issue-only table rendering and no-issues empty states
- sidebar footer placement for username/logout controls
- authenticated layout rendering without top header chrome
- Tracking file detail rendering of `Last integrity check` from `last_integrity_check_at`

Tracking Workspace UI tests (`frontend/src/pages/TrackingWorkspacePage.test.tsx`) also cover:

- source dropdown semantics (`Both` removed)
- folder status badge rendering (`empty` / `tracked` / `mirrored` / `partial`)
- folder `Scan` to `Mirror` action switching after queue-first scans
- virtual-path tree selection driving server-side table filtering
- left-pane collapse/expand behavior

**Basic structure:**

```rust
// src/core/checksum.rs

pub fn compute(path: &Path) -> anyhow::Result<String> {
    // ...
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_known_hash() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        let hash = compute(f.path()).unwrap();
        // BLAKE3 of "hello world"
        assert_eq!(hash, "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24");
    }
}
```

**Mocking with `mockall`:**

Use [`mockall`](https://docs.rs/mockall) to mock trait-based dependencies (e.g., the repository). Annotate the trait with `#[mockall::automock]` (or `#[cfg_attr(test, mockall::automock)]` to avoid the dependency in non-test builds).

```rust
// src/db/repository.rs

#[cfg_attr(test, mockall::automock)]
pub trait Repository {
    fn get_drive_pair(&self, id: i64) -> anyhow::Result<Option<DrivePair>>;
    fn list_drive_pairs(&self) -> anyhow::Result<Vec<DrivePair>>;
    // ...
}
```

```rust
// In a unit test inside src/core/mirror.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::MockRepository;

    #[test]
    fn test_mirror_selects_correct_pair() {
        let mut mock = MockRepository::new();
        mock.expect_get_drive_pair()
            .with(mockall::predicate::eq(1))
            .returning(|_| Ok(Some(DrivePair {
                id: 1,
                name: "test".to_string(),
                primary_path: "/primary".to_string(),
                secondary_path: "/secondary".to_string(),
            })));

        // Pass mock to the function under test
        let result = do_something_with_repo(&mock, 1);
        assert!(result.is_ok());
    }
}
```

---

## QEMU Installation Tests

There are three QEMU suites:

- `tests/installation/qemu_test.sh` is the fast package/install smoke test.
- `tests/installation/qemu_failover_test.sh` is the heavier end-to-end failover suite with extra virtio disks and QMP hot-remove.
- `tests/installation/qemu_uninstall_test.sh` validates full package purge, including removal of package-owned config/DB/backup/log paths.

### Prerequisites

| Tool | Install |
| --- | --- |
| `qemu-system-x86_64` | `sudo apt install qemu-system-x86_64` |
| KVM acceleration | Verify with `kvm-ok`. The script passes `-enable-kvm`; remove that flag if KVM is unavailable (slower). |
| `cloud-image-utils` | `sudo apt install cloud-image-utils` |
| `socat` | `sudo apt install socat` (required for QMP hot-remove in the failover suite) |
| Ubuntu 24 noble cloud image | See download step below |
| SSH public key | Either `~/.ssh/id_ed25519.pub` / `~/.ssh/id_rsa.pub`, or `BITPROTECTOR_QEMU_SSH_KEY` |

### Setup (one-time)

**1. Download the Ubuntu 24 cloud image:**

```bash
mkdir -p ~/images
wget -O ~/images/noble-server-cloudimg-amd64.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
```

The script looks for the image at `~/images/noble-server-cloudimg-amd64.img` by default. Override with the `UBUNTU_IMAGE` environment variable:

```bash
UBUNTU_IMAGE=/data/images/noble.img ./tests/installation/qemu_test.sh
```

**2. Build the `.deb` package:**

```bash
cd frontend
npm ci
npm run build
cd ..
cargo deb
```

The package is written to `target/debian/bitprotector_*.deb`.

### Running the tests

```bash
# Fast smoke test
./tests/installation/qemu_test.sh

# Or pass the .deb path explicitly
./tests/installation/qemu_test.sh /path/to/bitprotector_0.1.0_amd64.deb

# Full failover / replacement suite
./tests/installation/qemu_failover_test.sh

# Full uninstall / purge suite
./tests/installation/qemu_uninstall_test.sh

# Optional port/timeout overrides (useful when another VM is running)
SSH_PORT=2224 API_PORT=18445 TIMEOUT=240 ./tests/installation/qemu_test.sh
SSH_PORT=2225 API_PORT=18446 TIMEOUT=360 ./tests/installation/qemu_failover_test.sh
SSH_PORT=2226 API_PORT=18447 TIMEOUT=360 ./tests/installation/qemu_uninstall_test.sh
```

All scripts stream serial console lines to your terminal as the VM boots. They also fail fast if the QEMU process exits early instead of waiting out the full timeout.

### What gets tested

### Smoke test coverage

| Test | Description |
| --- | --- |
| Package installed | `which bitprotector` and `bitprotector --version` succeed |
| Service status | `systemctl is-active bitprotector` (NOTE: requires TLS certs to be present for full start) |
| CLI smoke tests | `bitprotector --db /tmp/test.db drives list` and `status` succeed |
| SSH login hook | `/etc/profile.d/bitprotector-status.sh` is present |

### Failover suite coverage

| Test | Description |
| --- | --- |
| Planned failover | Marks the primary slot `quiescing`, confirms failure, and verifies `active_role` switches to `secondary` |
| Virtual-path retargeting | Confirms symlinks move from `/mnt/primary/...` to `/mnt/mirror/...` during failover |
| Degraded writes | Writes through the virtual path while secondary is active, then runs folder scan to update checksums and mirror metadata |
| Replacement rebuild | Assigns `/mnt/replacement-primary`, runs `sync process`, and verifies files are rebuilt and virtual paths switch back |
| Emergency failover | Uses a QMP control socket plus `device_del` to hot-remove the active disk, then verifies a follow-up BitProtector operation fails over future opens to the surviving mirror |

### Uninstall suite coverage

| Test | Description |
| --- | --- |
| Package-owned data setup | Creates DB data at `/var/lib/bitprotector/bitprotector.db`, adds a backup destination under `/var/lib/bitprotector/backups/uninstall-test`, and runs a backup |
| Full purge | Runs `apt-get purge -y bitprotector` |
| Removal verification | Confirms package metadata, `/usr/bin/bitprotector`, `/etc/bitprotector`, `/var/lib/bitprotector`, and `/var/log/bitprotector` are removed |

### Smoke test exit codes (`qemu_test.sh`)

| Code | Meaning |
| --- | --- |
| `0` | All tests passed |
| `1` | `.deb` build or installation failed |
| `2` | systemd service failed to start |
| `3` | CLI smoke tests failed |
| `4` | API not accessible |

The failover and uninstall suites currently exit non-zero on the first failed assertion and print the failing scenario step directly from script output.

### TLS for full service startup

During the QEMU tests the service may not start cleanly because no TLS certificate is provisioned. This is expected — the smoke and failover checks use CLI commands with an explicit `--db` path and do not require the daemon to be running. To test the full service (including the API), add a self-signed cert to the cloud-init `runcmd` block in the script. See [docs/CONFIGURATION.md](CONFIGURATION.md#generating-a-self-signed-certificate) for a suitable `openssl` command.
