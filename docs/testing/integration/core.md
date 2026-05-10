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

This test file exercises the mirroring module end-to-end with real files on disk, as opposed to the unit tests which verify internal logic with mocked repositories.

**Basic mirror:** A file created in the primary directory is mirrored to the corresponding path in the secondary directory. After mirroring, the secondary file exists and its content matches the primary.

**Subdirectory handling:** A file nested in a subdirectory of the primary (e.g., `docs/reports/file.txt`) is mirrored to the same relative path under the secondary root. The subdirectory is created on the secondary if it does not already exist.

**Restore from secondary:** After a failover where the secondary becomes the active drive, `restore_from_secondary` copies a file from the secondary path back to the primary path. This is used during the rebuild phase when a replacement primary is assigned.

**Mirror overwrites stale secondary:** If the secondary already contains an older version of the file, mirroring overwrites it with the current primary version. The content of the secondary matches the primary after the mirror.

**Missing source file:** Attempting to mirror a file that does not exist on the primary returns an error. No partial file is created on the secondary.

**Checksum verification after mirror:** After a mirror operation, the checksum stored in the database for the file matches the checksum of the secondary copy. This confirms the integrity of the copy operation rather than just checking that a file was created.

**Large file correctness:** A test with a multi-megabyte file confirms that the mirror is complete and produces the correct checksum, guarding against truncation or partial writes.

---

## core_change_detection.rs — Change Detection and Re-Mirroring

**File:** `tests/integration/core_change_detection.rs`

**Unchanged file is not re-queued:** A file that has been mirrored and whose on-disk content has not changed is not added to the sync queue when change detection runs. This avoids unnecessary mirror operations.

**Modified file is detected:** A file whose content has changed since its last mirror has a different checksum than the stored value. Change detection identifies this and adds the file to the sync queue. After the queue is processed, the secondary reflects the updated content.

**Deleted file is detected:** A file that has been removed from the primary disk is detected as deleted. The tracking record is updated to reflect the deletion.

**Folder-level change detection:** Running change detection on a tracked folder checks all files discovered in that folder. Only the files that have changed are re-queued; unchanged files are skipped.

**Re-mirror after detection:** After change detection identifies a changed file and enqueues it, processing the sync queue mirrors the updated file. The secondary then contains the new version.

---

## core_scheduler.rs — Background Task Scheduling

**File:** `tests/integration/core_scheduler.rs`

**Interval schedule triggers:** A schedule configured with `interval_seconds: 5` is triggered on the first evaluation (since it has never run) and then is not triggered again until the interval elapses. The test uses a controlled clock to advance time and verify trigger behavior at exact intervals.

**Cron schedule triggers:** A schedule with a cron expression is triggered at the next wall-clock time matching the expression. The test verifies that a `0 * * * *` schedule (top of every hour) is not triggered immediately when the current time is not at the top of the hour.

**Disabled schedule is not triggered:** A schedule with `enabled: false` is never triggered regardless of elapsed time.

**Multiple simultaneous schedules:** When two schedules are both due at the same instant, both are triggered in the same evaluation cycle. No schedule is skipped.

**Last-run timestamp update:** After a schedule triggers, its `last_run_at` timestamp is updated. This prevents immediate re-triggering on the next evaluation cycle.

**Task type dispatch:** The scheduler dispatches to the correct handler for each task type: `sync`, `integrity_check`, and `database_backup`. The test verifies that triggering a `sync` schedule results in a sync queue being processed, not an integrity run.

---

## core_checksum_strategy.rs — Checksum Strategy Selection

**File:** `tests/integration/core_checksum_strategy.rs`

BitProtector uses BLAKE3 as its checksum algorithm. This test file verifies that the correct algorithm is selected and applied consistently across different scenarios.

**Consistent output:** Computing the checksum of the same file twice in the same process produces identical results. This confirms the algorithm is deterministic and not seeded with randomtime-dependent values.

**Different files produce different checksums:** Two files with different content produce different checksums. The probability of a collision with BLAKE3 is negligible, so this is a sanity check against accidental identity.

**Checksum matches known reference:** A fixed input produces the expected known BLAKE3 digest, confirming the algorithm is not accidentally substituted or misconfigured.

**Empty file checksum:** An empty file produces a specific known digest. This guards against edge cases in the hashing path for zero-byte files.

**Large file efficiency:** A large file (multiple megabytes) is checksummed within a reasonable time budget. This is not a strict performance test — it is a check that the hash does not block indefinitely or read the file repeatedly.

**Binary file handling:** A file containing arbitrary binary data (including null bytes) is checksummed correctly. This confirms the hasher does not treat the data as text or stop at a null terminator.

---

## scaling_100k.rs — 100k-Row Performance Budgets

**File:** `tests/integration/scaling_100k.rs`

This test seeds 100,000 tracked file rows into an in-memory SQLite database and verifies that the tracking API endpoint meets time budgets for common query patterns. It exists to catch regressions where a schema change, missing index, or query refactor causes query time to grow unbounded at realistic data sizes.

**Setup:** 100,000 tracked file records are inserted into a single drive pair using a batch transaction. The records include a range of virtual paths and source types to exercise index selectivity.

**Baseline list query:** `GET /api/v1/tracking/items` with no filters must return the first page within 3,000 ms. This is the most common query path and the one most likely to be hit by a full-table scan.

**Virtual path prefix filter:** Filtering by a `virtual_prefix` value that matches a subset of records must return the correct results within 3,000 ms. The test verifies both the count and the timing.

**Source filter:** Filtering by `source=direct` and `source=folder` separately must each return within 3,000 ms. The test verifies that the index on the source column is used effectively.

**Targeted search:** Filtering by a specific search string that matches a small number of records must return within 3,000 ms.

**Pagination cap:** Requesting `per_page=500` returns at most 200 results — the server-side cap — regardless of the requested value. The test verifies the cap is enforced even when 100,000 results are available.

**Why 3,000 ms:** The budget is set conservatively for the test environment (in-memory SQLite without query caching warm-up), giving headroom for CI runner variability while still catching significant regressions. A query that takes longer than 3,000 ms almost certainly indicates a missing index or a full-table scan.

---

## packaging.rs — Packaging Artifact Verification

**File:** `tests/integration/packaging.rs`

This file uses only the standard library's filesystem functions — no binary invocations and no database. It verifies that all files required for a valid `.deb` package installation are present and contain the expected content markers.

**Why this exists:** The `cargo deb` build tool assembles the `.deb` from a set of declared source files. If a file is accidentally removed from the repository or its path is changed without updating the `Cargo.toml` packaging config, the package will build but the installed system will be broken. These tests catch that before the artifact is uploaded.

**Systemd service file:** `packaging/bitprotector.service` exists, contains an `[Unit]` section, and references the correct binary path.

**Default config:** `packaging/config.toml` exists and contains the expected section headers. It is the template that cloud-init installs during QEMU tests.

**Profile.d hook:** The shell script that sets `BITPROTECTOR_DB` in the user's environment exists and is not empty.

**QEMU bundle scripts:** Each bundle script under `tests/installation/bundles/` exists and is non-empty. The test catches the case where a bundle script is accidentally deleted or moved to a different path.

**Wrapper scripts:** The backward-compatible wrappers (`qemu_test.sh`, `qemu_failover_test.sh`, `qemu_uninstall_test.sh`) exist and contain the expected `exec` delegation line.

**Maintainer scripts:** `packaging/scripts/postinst`, `prerm`, and `postrm` exist and contain the expected shell function signatures that the Debian packaging system calls during install and removal.
