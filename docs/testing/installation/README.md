# QEMU Installation Tests — Infrastructure and Setup

This document explains how the QEMU-based installation test suite is structured, how to set it up, and how to run it. For per-bundle scenario coverage, see [bundles.md](bundles.md).

---

## Table of Contents

- [What QEMU Tests Verify](#what-qemu-tests-verify)
- [How It Works — Bundles, Scenarios, and Libraries](#how-it-works--bundles-scenarios-and-libraries)
- [The Database Disk](#the-database-disk)
- [Shared Library Helpers](#shared-library-helpers)
- [Prerequisites](#prerequisites)
- [One-Time Setup](#one-time-setup)
- [Running the Tests](#running-the-tests)
- [Wrapper Scripts](#wrapper-scripts)
- [Environment Variable Overrides](#environment-variable-overrides)
- [Failure Diagnostics](#failure-diagnostics)

---

## What QEMU Tests Verify

Unit and integration tests confirm that individual functions and API endpoints work correctly in isolation. QEMU tests ask a different question: does the complete installed system work correctly?

A QEMU test boots a fresh Ubuntu 24 virtual machine from a cloud image, installs the built `.deb` package via cloud-init, configures the service identically to a production deployment (TLS, PAM, systemd), and then runs a series of scenarios over SSH and against the HTTPS API.

This layer catches bugs that only manifest in the full system context:

- Packaging errors where a file is missing from the `.deb` or installed to the wrong path.
- Service startup failures caused by a missing dependency or incorrect `systemd` unit configuration.
- Permission errors where the service user cannot access a directory or file that the test environment took for granted.
- Reboot persistence issues where data is stored in a location that does not survive a guest reboot.
- Hardware failure behaviors (failover, bit-flip corruption, disk hot-remove) that require real block devices to simulate.

---

## How It Works — Bundles, Scenarios, and Libraries

### Bundles

A **bundle** is an entry-point shell script in `tests/installation/bundles/`. Each bundle:

1. Validates prerequisites (required commands, cloud image, `.deb` file).
2. Creates a QEMU disk image layered on the base cloud image.
3. Writes a cloud-init user-data file that provisions the VM: installs the `.deb`, configures TLS, starts the service, creates test users, and mounts additional virtual disks.
4. Starts the QEMU process and waits for the SSH server to become reachable.
5. Runs each scenario script in order by sourcing it and calling its named function.
6. Stops the VM and reports results.

If any scenario fails, the bundle exits immediately with a non-zero status and prints the failing scenario name plus recent serial-console output.

Each bundle reuses a single VM across all its scenarios. This avoids the overhead of provisioning a new VM per scenario while still keeping the scenarios independent through careful state management (each scenario uses its own database file and working directories).

### Scenarios

A **scenario** is a shell script in `tests/installation/scenarios/<bundle-name>/`. Each scenario:

- Defines exactly one function (named after the scenario file, e.g., `smoke_01_package_installed`).
- Runs SSH commands against the guest using the `ssh_vm` helper.
- Makes API calls using `curl` against the guest's HTTPS port.
- Uses `assert` calls (from the shared library) to check expected outputs and exit codes.

Scenarios are numbered to define a guaranteed execution order within a bundle. The number prefix in the filename is the canonical ordering.

### Why This Structure

Separating bundles from scenarios allows:

- New scenarios to be added without modifying the bundle script.
- Scenarios to be developed and tested individually during development (`ssh` into a running manual VM and run the scenario function directly).
- Bundles to be composed differently in CI (e.g., `drive_media_type.sh` reuses two smoke scenarios rather than duplicating them).

---

## The Database Disk

Each QEMU bundle provisions a dedicated virtual disk for scenario database files:

- The disk is a 32 GB qcow2 image created fresh for each bundle run.
- Inside the VM, it is mounted at `/mnt/bitprotector-db`.
- Scenario database files are stored under `/mnt/bitprotector-db/db/`.

This separation exists because the cloud image root filesystem has limited space. Scale tests generate large amounts of data and would fill the root volume, causing failures that are not related to the feature being tested. Placing the database on a dedicated disk also mirrors a reasonable production configuration where the database is on a separate volume.

The cloud-init setup for this disk is provided by `tests/installation/lib/cloud-init-db-disk.sh`, which is sourced by every bundle that needs it.

---

## Shared Library Helpers

### `tests/installation/lib/qemu-helpers.sh`

Provides functions for VM lifecycle management:

- **`log`**: Structured logging with optional GitHub Actions annotation output (group markers, error annotations, warning annotations).
- **`require_commands`**: Verifies that all listed commands are available on the host. Exits with a descriptive error if any are missing.
- **`resolve_ssh_key`**: Finds the SSH public key to inject into the VM. Checks `BITPROTECTOR_QEMU_SSH_KEY` first, then falls back to `~/.ssh/id_ed25519.pub` and `~/.ssh/id_rsa.pub`.
- **`resolve_guest_image`**: Finds the Ubuntu cloud image. Checks `GUEST_IMAGE` first; accepts an absolute path or the shorthands `ubuntu-24.04` and `ubuntu-26.04`. Falls back to `UBUNTU_IMAGE` (deprecated alias) and defaults to `ubuntu-24.04` if neither is set.
- **`wait_for_vm`**: Polls over SSH until the cloud-init sentinel file `/tmp/install-done` exists on the guest, with a configurable timeout and fail-fast behavior if the QEMU process exits. Streams serial console progress lines to the terminal while waiting.

### `tests/installation/lib/scenarios.sh`

Provides helpers used inside scenario scripts:

- **`ssh_vm`**: Runs a command on the guest over SSH with a configurable timeout. Handles `StrictHostKeyChecking=no` and keepalive settings.
- **`make_pair`**: Registers a drive pair via the CLI and returns the assigned pair ID. Used by scenarios that need to set up a drive configuration before testing a feature.
- **`seed_file`**: Creates a file of a specified size on the guest using `dd if=/dev/urandom`. Used to set up test data for mirroring and integrity tests.
- **`api_login`**: Logs in through the API and returns a bearer token. Retries for up to 60 seconds to accommodate service startup races.
- **`api_json`**: Generic API call wrapper. Takes a method, path, token, and optional JSON body. Returns the response body on success (2xx) and exits non-zero on HTTP error, with retries for transient connectivity failures.
- **`assert_no_journal_errors`**: Queries `journalctl` for error-level entries from the bitprotector unit since a given timestamp and fails if any unexpected entries are found.
- **`expect_journal_error`**: Registers a pattern as an expected error, suppressing it from the `assert_no_journal_errors` check. Used by resilience scenarios that deliberately trigger errors.
- **`journal_error_scraper`**: Final-scenario wrapper that calls `assert_no_journal_errors` using the bundle's `BUNDLE_START_TIME`. Run as the last scenario in every bundle.

### `tests/installation/lib/snapshots.sh`

Provides QEMU snapshot management and device hotplug helpers. The failover bundle saves a VM snapshot after initial provisioning so it can restore to a known-good state between scenarios that modify disk topology, without reprovisioning the VM.

- **`qmp_savevm`**: Saves the current VM state to a named snapshot via QMP `savevm`.
- **`qmp_loadvm`**: Restores a named VM state snapshot via QMP `loadvm`.
- **`qmp_delvm`**: Deletes a named VM snapshot via QMP `delvm`.
- **`qmp_device_add`**: Hot-adds a virtual device by sending a QMP `device_add` command with a JSON device descriptor.
- **`qmp_device_del`**: Hot-removes a virtual device by ID by sending a QMP `device_del` command.

---

## Prerequisites

| Tool | Purpose | Install |
| --- | --- | --- |
| `qemu-system-x86_64` | Run virtual machines | `sudo apt install qemu-system-x86_64` |
| `qemu-img` | Create and manage disk images | Included with `qemu-utils` |
| `cloud-localds` | Build cloud-init seed ISOs | `sudo apt install cloud-image-utils` |
| `ssh` and `ssh-keygen` | Connect to the VM | Pre-installed on most systems |
| `socat` | QMP socket communication for hot-remove | `sudo apt install socat` (failover bundle only) |
| KVM acceleration | Hardware virtualisation | Verify with `kvm-ok`; optional but required for reasonable performance |
| Ubuntu 24 cloud image | Base VM image | See [One-Time Setup](#one-time-setup) |
| SSH public key | VM login | `~/.ssh/id_ed25519.pub` or set `BITPROTECTOR_QEMU_SSH_KEY` |

---

## One-Time Setup

### 1. Download the Ubuntu 24 cloud image

```bash
mkdir -p ~/images
wget -O ~/images/noble-server-cloudimg-amd64.img \
  https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
```

The scripts look for the image at `~/images/noble-server-cloudimg-amd64.img` by default. Override this with the `GUEST_IMAGE` environment variable.

### 2. Build the `.deb` package

```bash
cd frontend
npm ci
npm run build
cd ..
cargo deb
```

The package is written to `target/debian/bitprotector_*.deb`. The test scripts find it automatically by glob. Pass an explicit path as the first argument to use a different package.

---

## Running the Tests

Full run commands for each bundle are documented in [bundles.md](bundles.md). The common entry points are:

```bash
./tests/installation/qemu_test.sh             # Smoke bundle
./tests/installation/qemu_failover_test.sh    # Failover bundle
./tests/installation/qemu_uninstall_test.sh   # Uninstall bundle
./tests/installation/bundles/application_workflows.sh
./tests/installation/bundles/resilience.sh
./tests/installation/bundles/upgrade.sh
./tests/installation/bundles/degraded_boot.sh
./tests/installation/bundles/drive_media_type.sh
```

---

## Wrapper Scripts

Three backward-compatible wrapper scripts delegate to the corresponding bundle:

| Wrapper | Delegates to |
| --- | --- |
| `tests/installation/qemu_test.sh` | `tests/installation/bundles/smoke.sh` |
| `tests/installation/qemu_failover_test.sh` | `tests/installation/bundles/failover.sh` |
| `tests/installation/qemu_uninstall_test.sh` | `tests/installation/bundles/uninstall.sh` |

These exist so that CI configuration and documentation written before the bundle structure was introduced continues to work without changes.

---

## Environment Variable Overrides

| Variable | Default | Purpose |
| --- | --- | --- |
| `GUEST_IMAGE` | `ubuntu-24.04` | Path to the guest image, or a shorthand (`ubuntu-24.04`, `ubuntu-26.04`). Preferred over `UBUNTU_IMAGE`. |
| `UBUNTU_IMAGE` | — | Deprecated alias for `GUEST_IMAGE`. Accepted when `GUEST_IMAGE` is unset. |
| `BITPROTECTOR_QEMU_SSH_KEY` | Auto-detected | SSH public key content to inject into the VM |
| `SSH_PORT` | Bundle-specific (2222, 2223, etc.) | Host port for SSH forwarding |
| `API_PORT` | Bundle-specific (18443, 18444, etc.) | Host port for HTTPS forwarding |
| `TIMEOUT` | Bundle-specific (600, 900, etc.) | Maximum seconds to wait for SSH to become available |
| `ALPHA1_DEB` | — | Path to the older `.deb` for the upgrade bundle |

Port overrides are useful when running multiple bundles simultaneously or when another VM is using the default ports.

---

## Failure Diagnostics

All bundle scripts stream serial console output to your terminal as the VM boots. This shows cloud-init progress, package installation, and service startup, which is where most boot-time failures are visible.

If a run fails with `database or disk is full`:

- Check the mount preflight output for `/mnt/bitprotector-db`.
- Check the `df -h` output printed by scale scenarios at the start of each scenario.
- In CI, download the QEMU serial log artifact from the failed run.

If a scenario fails mid-run:

- The bundle prints the scenario name and the last 20 lines of serial console output.
- In CI, the full serial log is uploaded as an artifact.
- For local debugging, SSH into a manually running VM and execute the scenario function interactively.
