# Testing Guide

This document explains how to run the test suite, what each test category covers, how to write new unit tests, and how to run the full QEMU-based installation tests.

---

## Table of Contents

- [Test Layout](#test-layout)
- [Running Tests](#running-tests)
- [Integration Tests](#integration-tests)
  - [CLI Integration Tests](#cli-integration-tests)
  - [Auth / API Integration Tests](#auth--api-integration-tests)
  - [Packaging Tests](#packaging-tests)
- [Unit Tests](#unit-tests)
- [QEMU Installation Tests](#qemu-installation-tests)

---

## Test Layout

```
tests/
├── integration/          # Integration tests (Rust — cargo test)
│   ├── cli_auth.rs       # JWT middleware, token lifecycle
│   ├── cli_drives.rs     # Drive pair CLI commands
│   ├── cli_files.rs      # File tracking CLI commands
│   ├── cli_folders.rs    # Tracked folder CLI commands
│   ├── cli_integrity.rs  # Integrity check CLI commands
│   ├── cli_logs.rs       # Event log CLI commands
│   ├── cli_sync.rs       # Sync queue CLI commands
│   ├── cli_status.rs     # SSH status display
│   ├── cli_virtual_paths.rs  # Virtual path CLI commands
│   ├── cli_database.rs   # Database backup CLI commands
│   └── packaging.rs      # Verifies packaging artifacts exist
├── unit/                 # Out-of-tree unit test files (currently empty)
│                         # Prefer inline #[cfg(test)] modules — see Writing Unit Tests
├── module/               # Module-level tests (currently empty)
│                         # Used for multi-component round-trip tests
└── installation/
    └── qemu_test.sh      # Full install test on Ubuntu 24 via QEMU
```

Inline unit tests (`#[cfg(test)]` blocks inside `src/`) are the primary home for unit-level testing. The `tests/unit/` directory is available for cases where keeping tests outside the source tree is preferable.

---

## Running Tests

### Run everything

```bash
cargo test
```

### Run all tests in one integration file

```bash
cargo test --test cli_drives
cargo test --test cli_auth
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

### Packaging Tests

File: `tests/integration/packaging.rs`

These Rust tests use only `std::fs` — no binary invocations. They verify that all packaging artifacts (systemd service file, default config, `profile.d` hook, QEMU test script) exist and contain the required sections. Run them with:

```bash
cargo test --test packaging
```

---

## Unit Tests

Unit tests live as inline `#[cfg(test)]` modules at the bottom of each `src/` file. This keeps them next to the code they test and avoids additional file management.

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

**When to use `tests/unit/` or `tests/module/`:**

- `tests/unit/` — for test files that need to test multiple modules together but still at unit granularity, or when the test file would be too large to embed inline.
- `tests/module/` — for multi-component round-trip tests that cross module boundaries (e.g., track a file → run integrity check → verify queue entry). These are larger than unit tests but do not invoke the binary.

---

## QEMU Installation Tests

The script `tests/installation/qemu_test.sh` boots a fresh Ubuntu 24 VM, installs the `.deb` package produced by `cargo deb`, and runs smoke tests against the installed system.

### Prerequisites

| Tool | Install |
|---|---|
| `qemu-system-x86_64` | `sudo apt install qemu-system-x86_64` |
| KVM acceleration | Verify with `kvm-ok`. The script passes `-enable-kvm`; remove that flag if KVM is unavailable (slower). |
| `cloud-image-utils` | `sudo apt install cloud-image-utils` |
| Ubuntu 24 noble cloud image | See download step below |

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
cargo deb
```

The package is written to `target/debian/bitprotector_*.deb`.

### Running the tests

```bash
./tests/installation/qemu_test.sh

# Or pass the .deb path explicitly
./tests/installation/qemu_test.sh /path/to/bitprotector_0.1.0_amd64.deb
```

The script streams serial console lines to your terminal as the VM boots (up to 600 seconds timeout).

### What gets tested

| Test | Description |
|---|---|
| Package installed | `which bitprotector` and `bitprotector --version` succeed |
| Service status | `systemctl is-active bitprotector` (NOTE: requires TLS certs to be present for full start) |
| CLI smoke tests | `bitprotector --db /tmp/test.db drives list` and `status` succeed |
| SSH login hook | `/etc/profile.d/bitprotector-status.sh` is present |

### Exit codes

| Code | Meaning |
|---|---|
| `0` | All tests passed |
| `1` | `.deb` build or installation failed |
| `2` | systemd service failed to start |
| `3` | CLI smoke tests failed |
| `4` | API not accessible |

### TLS for full service startup

During the QEMU test the service may not start cleanly because no TLS certificate is provisioned. This is expected — the CLI smoke tests use `--db /tmp/test.db` and do not require the daemon to be running. To test the full service (including the API), add a self-signed cert to the cloud-init `runcmd` block in the script. See [docs/CONFIGURATION.md](CONFIGURATION.md#generating-a-self-signed-certificate) for a suitable `openssl` command.
