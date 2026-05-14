# Core, Scaling, and Artifact Integration Tests

This document covers the integration test files that do not target CLI commands or REST API endpoints directly, but instead exercise core mechanics, performance characteristics, and packaging artifacts.

---

## Table of Contents

- [core_mirror.rs — File Mirroring Mechanics](#core_mirrorrs--file-mirroring-mechanics)
- [core_change_detection.rs — Change Detection and Re-Mirroring](#core_change_detectionrs--change-detection-and-re-mirroring)
- [core_scheduler.rs — Background Task Scheduling](#core_schedulerrs--background-task-scheduling)
- [core_checksum_strategy.rs — Checksum Strategy Selection](#core_checksum_strategyrs--checksum-strategy-selection)
- [scaling_100k.rs — 100k-Row Performance Budgets](#scaling_100krs--100k-row-performance-budgets)
- [packaging.rs — Packaging Artifact Verification](#packagingrs--packaging-artifact-verification)

---

## core_mirror.rs — File Mirroring Mechanics

**File:** `tests/integration/core_mirror.rs`

This test file exercises the two mirroring functions directly with real files on disk, verifying both the happy path and all error conditions.

**`restore_mirror_from_master` — copy primary → secondary:**

- *Happy path:* A file on the primary is copied to the matching path on the secondary. After the call the secondary file exists and its content matches the primary.
- *Missing primary:* Attempting to mirror a file that has been removed from the primary after tracking returns an error whose message contains `does not exist`.
- *Checksum mismatch on source:* If the primary file has been tampered with (content differs from the stored checksum), the call returns an error whose message contains `checksum mismatch`. No stale copy is written to the secondary.
- *Subdirectory creation:* A file nested under a multi-level path (e.g. `a/b/nested.txt`) is mirrored correctly and the directory tree is created on the secondary if it does not yet exist.
- *Secondary drive failed:* When the secondary has been put through `mark_drive_quiescing` → `confirm_drive_failure`, the call returns an error. Mirroring to a failed drive is refused.

**`restore_from_mirror` — copy secondary → primary:**

- *Happy path:* Given a secondary copy with matching content, the function restores the primary file from the mirror.
- *Mirror missing:* If no file exists on the secondary, the call returns an error.
- *Checksum mismatch on mirror:* If the secondary file is corrupted (content differs from the stored checksum), the call returns an error whose message contains `checksum mismatch`.
- *Primary drive failed:* When the primary has been put through `mark_drive_quiescing` → `confirm_drive_failure`, the call returns an error.

**`mirror_file` — standby readiness guard:**

- When the secondary drive is in the `quiescing` state (not yet failed but no longer accepting syncs), `mirror_file` returns an error rather than attempting the copy.

---

## core_change_detection.rs — Filesystem Watcher

**File:** `tests/integration/core_change_detection.rs`

This file tests the `watch_folder` function, which wraps the OS inotify/FSEvents interface and delivers `notify::Event` values to a caller-supplied callback. Tests use a real temporary directory and wait up to 2 seconds for events to arrive.

**New file detected:** Writing a new file into a watched directory causes at least one event to be delivered to the callback.

**Modification detected:** Overwriting an existing file in the watched directory triggers an event.

**Deletion detected:** Removing a file from the watched directory triggers an event.

**Invalid path returns error:** Calling `watch_folder` with a path that does not exist returns an error immediately rather than silently succeeding.

**Drop stops delivery:** Dropping the watcher handle (the value returned by `watch_folder`) stops event delivery cleanly without panicking, even if filesystem activity continues after the drop.

---

## core_scheduler.rs — Background Task Scheduling

**File:** `tests/integration/core_scheduler.rs`

The scheduler manages a set of background threads, one per enabled schedule. Tests exercise the lifecycle of those threads through the `reload` and `stop_all` methods.

**Lifecycle — empty database:** `reload` on a freshly initialized repo (no schedules) succeeds without starting any threads. `stop_all` on a freshly constructed scheduler (no threads) does not panic.

**Lifecycle — start thread on reload:** After inserting an enabled schedule, `reload` starts exactly one background thread for it.

**No duplicate threads:** Calling `reload` twice for the same active schedule does not start a second thread.

**Reload stops thread when disabled:** If a schedule is updated to `enabled: false` in the database and `reload` is called, the running thread for that schedule is stopped.

**Reload stops thread when deleted:** If a schedule is deleted from the database and `reload` is called, the thread for that schedule is cleaned up.

**Thread fires and processes work:** A schedule with `interval_seconds: 1` fires after approximately 1 second. The test creates a pending mirror queue item and waits 1.5 seconds; after `stop_all` the queue item is `completed` and the file exists on the secondary drive.

**Mixed enabled/disabled:** When two schedules are configured — one enabled, one disabled — `reload` starts exactly one thread (for the enabled schedule) and `stop_all` cleans up without error.

**`max_duration_seconds` respected:** A schedule with a `max_duration_seconds` constraint is accepted and the thread starts and stops cleanly.

---

## core_checksum_strategy.rs — Checksum Strategy Selection

**File:** `tests/integration/core_checksum_strategy.rs`

BitProtector has two checksum strategies: `Streaming` (sequential read) and `MmapRayon` (memory-mapped parallel read). The strategy used for a given drive pair depends on the media types involved. These tests verify that strategy selection and the copy-with-checksum helper are correct.

**Strategy parity:** `checksum_file` called with `ChecksumStrategy::Streaming` and then with `ChecksumStrategy::MmapRayon` on the same file produces identical hashes. This confirms the two code paths are computing the same value.

**`copy_with_checksum` correctness:** `copy_with_checksum` returns a `(src_hash, dst_hash)` pair. The test verifies that both values match independently-computed checksums of the source and destination files, and that source and destination hashes are equal after a copy.

**Pool size — HDD/HDD:** `pool_size_for_pair(Hdd, Hdd, &cfg)` returns the HDD concurrency limit from the configuration.

**Pool size — SSD/SSD:** `pool_size_for_pair(Ssd, Ssd, &cfg)` returns the SSD concurrency limit from the configuration.

**Pool size — mixed pair:** When one drive is HDD and the other is SSD, `pool_size_for_pair` returns the lower HDD limit regardless of which side is HDD and which is SSD.

---

## scaling_100k.rs — 100k-Row Performance Budgets

**File:** `tests/integration/scaling_100k.rs`

This test seeds 100,000 tracked file rows into an in-memory SQLite database and verifies that the tracking API endpoint meets time budgets for common query patterns. It exists to catch regressions where a schema change, missing index, or query refactor causes query time to grow unbounded at realistic data sizes.

**Setup:** 100,000 tracked file records are inserted into a single drive pair using a batch transaction. The records include a range of virtual paths and source types to exercise index selectivity.

**Single combined test:** All timing and correctness assertions live in one test function (`test_tracking_items_scales_to_100k_with_pagination_filters_and_budgets`). Within it, four sequential queries are timed:

1. **Unfiltered first page:** `GET /api/v1/tracking/items?drive_id=…&item_kind=file&page=1&per_page=500` — asserts `total` equals 100,000, `per_page` in the response is 200 (the server-side cap applied regardless of the requested value), and 200 items are returned.

2. **Virtual prefix + `has_virtual_path` filter:** Adding `has_virtual_path=true&virtual_prefix=/virtual/docs` — asserts 16,667 results returned and every item's `virtual_path` starts with `/virtual/docs/`.

3. **`has_virtual_path` + `source=folder` filter:** Adding `has_virtual_path=true&source=folder` — asserts 13,334 results returned within budget.

4. **Targeted text search:** `q=photo-000001` — asserts exactly 1 result returned and its `path` is `media/photo-000001.jpg`.

---

## packaging.rs — Packaging Artifact Verification

**File:** `tests/integration/packaging.rs`

This file uses only the standard library's filesystem functions — no binary invocations and no database. It verifies that all files required for a valid `.deb` package installation are present and contain the expected content markers.

**Why this exists:** The `cargo deb` build tool assembles the `.deb` from a set of declared source files. If a file is accidentally removed from the repository or its path is changed without updating the `Cargo.toml` packaging config, the package will build but the installed system will be broken. These tests catch that before the artifact is uploaded.

**Systemd service file:** `packaging/bitprotector.service` exists and contains all four required sections: `[Unit]`, `[Service]`, `[Install]`, and an `ExecStart=` directive.

**Default config:** `packaging/config.toml` exists and contains a `[server]` section, a `[database]` section, and a `jwt_secret` field.

**Profile.d hook:** `scripts/bitprotector-status.sh` exists and its content contains both the string `bitprotector` and the string `status`, confirming it invokes the status subcommand.

**QEMU wrapper scripts and bundle content:** Each wrapper is checked both for the `exec` delegation line and for the name of the bundle file it delegates to. The bundle files themselves are also read and checked for key content markers:
- `tests/installation/qemu_test.sh` → delegates to `bundles/smoke.sh`; `smoke.sh` contains `qemu-system-x86_64` and `bitprotector*.deb`.
- `tests/installation/qemu_failover_test.sh` → delegates to `bundles/failover.sh`; `failover.sh` contains `qmp`; the planned-failover scenario contains `drives replace confirm`; the emergency scenario contains `qmp_device_del`.
- `tests/installation/qemu_uninstall_test.sh` → delegates to `bundles/uninstall.sh`; the purge scenario contains `apt-get purge -y bitprotector` and `/var/lib/bitprotector`; the create-data scenario contains `database run`.

**Cargo.toml deb metadata:** `Cargo.toml` contains a `[package.metadata.deb]` section referencing `bitprotector.service`, `bitprotector-status.sh`, `config.toml`, `frontend/dist/**/*`, and the install path `var/lib/bitprotector/frontend`.

**Maintainer scripts:** `packaging/scripts/postinst` exists and contains references to the `bitprotector` user, `/var/lib/bitprotector`, and `/var/lib/bitprotector/frontend`. `packaging/scripts/postrm` exists and handles the `purge` action, removing `/var/lib/bitprotector`, `/var/log/bitprotector`, and `/etc/bitprotector`.
