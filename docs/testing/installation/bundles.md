# QEMU Installation Test Bundles

This document describes every bundle in the QEMU installation test suite, including the purpose of each scenario. Bundles run in a single VM and execute scenarios in the order listed. For infrastructure details, see [README.md](README.md).

---

## Table of Contents

- [Smoke](#smoke)
- [Failover](#failover)
- [Uninstall](#uninstall)
- [Application Workflows](#application-workflows)
- [Resilience](#resilience)
- [Upgrade](#upgrade)
- [Degraded Boot](#degraded-boot)
- [Drive Media Type](#drive-media-type)
- [Scale (nightly)](#scale-nightly)
- [Scale Lowmem (nightly)](#scale-lowmem-nightly)
- [Scheduled Load (nightly)](#scheduled-load-nightly)

---

## Smoke

**Entry point:** `tests/installation/bundles/smoke.sh`  
**Wrapper:** `tests/installation/qemu_test.sh`  
**Scenarios:** `tests/installation/scenarios/smoke/` (15 of 17 files sourced)  
**Default ports:** SSH 2222, HTTPS 18443  
**Purpose:** Validates a successful package installation and the core service behaviors that must work before any other testing is meaningful. This bundle is the first gate in CI and the quickest signal that a build is fundamentally broken.

### Scenarios

**smoke-01 — Package installed**  
Verifies the `bitprotector` binary is on the PATH and responds to `--version`. Confirms the package was installed by cloud-init before any other assertions.

**smoke-02 — Service active with TLS**  
Confirms the `bitprotector.service` systemd unit is `active` (running) and that the HTTPS endpoint responds to a health check. This is the foundation for all API-based scenarios.

**smoke-03 — CLI smoke**  
Runs basic CLI commands (`drives list`, `status`) against the service database to confirm the CLI is functional and the database was initialized correctly by the service.

**smoke-04 — Profile.d installed**  
Verifies the `profile.d` hook script that sets `BITPROTECTOR_DB` is installed at the expected path. This script is what makes the CLI usable without passing `--db` on every invocation.

**smoke-05 — Profile.d execution**  
Sources the profile hook and checks its conditional output behavior: confirms it produces no output when the database file is absent, and produces status output when the database file exists.

**smoke-06 — ldd version sanity**  
Runs `ldd` against the binary and confirms there are no missing shared libraries. Verifies the PAM library is among the linked libraries. Also checks that the binary's `--version` output matches the upstream version recorded in the installed package.

**smoke-07 — Journald integration**  
Restarts the `bitprotector.service` unit and then queries `journalctl` for output from the bitprotector unit. Confirms that at least one log line appears in the system journal, verifying basic journald integration.

**smoke-08 — PAM login**  
Authenticates against the API using the PAM-backed credentials (`testauth` / `hunter2`) that were provisioned via cloud-init. Confirms that the PAM module is correctly installed and the service correctly delegates password validation to PAM.

**smoke-09 — JWT persists across restart**  
Issues a JWT via the login endpoint, restarts the `bitprotector.service` unit, and then uses the same JWT to make an authenticated API request. Confirms that JWTs are valid based on the secret key in the config file rather than in-memory state.

**smoke-10 — TLS cert rotation**  
Retrieves the current TLS fingerprint, regenerates the certificate and key files in place, and runs `systemctl restart bitprotector` to reload the TLS configuration. Confirms the new fingerprint differs from the original, verifying the certificate was rotated.

**smoke-11 — Path traversal rejected**  
Sends API requests with paths containing `..` components designed to escape the drive root. Confirms all such requests are rejected with `400 Bad Request` rather than being resolved to a path outside the intended root.

**smoke-12 — Reboot persistence**  
Reboots the VM and waits for the SSH server to return. After reboot, confirms that the service is running, tracked data is still present in the database, and the database file is at the expected path. This catches data stored in non-persistent locations such as `/tmp`.

**smoke-13 — Database backup, repair, and staged restore**  
Adds two backup destinations and runs a manual `database run`. Verifies backup files exist at both destinations. Corrupts one destination's backup file, then runs `database check-integrity` and verifies the corrupted file was repaired (validated via SQLite `integrity_check`). Stages a restore from the repaired backup, restarts the service, and waits for the health API to return `ok`. Confirms the staged restore is reflected in `bitprotector status` and `database list`.

**smoke-16 — Scheduled sync, integrity, and database backup sweep**  
Configures schedules for sync, integrity check, and database backup with short intervals, then waits for all three to trigger and complete. Confirms the scheduler correctly dispatches each task type and that results are visible in the respective API endpoints afterward.

**smoke-17 — adopt_mirror queue action end-to-end**  
Verifies all three `adopt_mirror` queue action cases: (a) **matching standby** — a file already exists on the standby with identical bytes; no copy is performed and the file is marked mirrored. (b) **stale standby** — the standby copy has different content; a full copy from primary overwrites it. (c) **missing standby** — no file exists at the standby path; the primary file is copied across. After processing the queue, all three files must have `is_mirrored: true` and standby content must match primary.

---

## Failover

**Entry point:** `tests/installation/bundles/failover.sh`  
**Wrapper:** `tests/installation/qemu_failover_test.sh`  
**Scenarios:** `tests/installation/scenarios/failover/` (12 scenarios)  
**Default ports:** SSH 2223, HTTPS 18444  
**VM disks:** Root + 4 extra virtio disks (primary, mirror, replacement-primary, replacement-secondary) + bpdb  
**Purpose:** Validates the drive failover and replacement workflows under realistic hardware-simulation conditions. This is the most complex bundle because it involves disk state changes that must persist across test steps.

### Scenarios

**failover-01 — Planned primary failover and replacement**  
The complete planned replacement lifecycle: track files, mirror them, mark the primary for quiescing, confirm failure, assign a replacement, and verify the tracked files are rebuilt on the replacement. Confirms data integrity is maintained throughout.

**failover-02 — Emergency failover via QMP**  
Simulates an unplanned disk loss by using the QEMU Machine Protocol to hot-remove the replacement-primary disk (the active drive at this point, after failover-01). Runs `bitprotector integrity check 1` to trigger detection of the missing device. Confirms the active role switches to the secondary drive and the virtual path resolves to `/mnt/mirror`.

**failover-03 — Bit-flip corruption and auto-repair**  
Corrupts a single byte in the secondary copy of a tracked file. Runs an integrity check with auto-recovery enabled. Confirms the service detects the mismatch, re-mirrors from the primary, and the repaired secondary file matches the primary.

**failover-04 — Both copies corrupted**  
Corrupts both the primary and secondary copies of a tracked file. Runs an integrity check. Confirms the service reports the file as unrecoverable (both copies differ from the stored checksum) rather than silently re-mirroring a corrupted file.

**failover-05 — Large file streaming**  
Tracks and mirrors a 200 MB file allocated with `fallocate`. Runs `integrity check-all --recover` to confirm the file is consistent. Optionally reads the running service's RSS from `/proc/[pid]/status` and asserts it stays below 350 MB, guarding against memory growth on large files.

**failover-06 — Integrity-triggered auto-recovery**  
Corrupts the primary copy of a tracked file (simulating a bad write to the primary). Runs `integrity check-all --drive-id 1 --recover` manually. Confirms the service detects the mismatch, restores the primary from the mirror copy, and `cmp` verifies both copies are identical.

**failover-07 — Virtual-path folder retarget after failover**  
A folder with a virtual path is tracked on the primary. After failover to the secondary, the folder's virtual path retarget is verified: files exposed at the virtual path now read from the secondary.

**failover-08 — Unicode, whitespace, and long paths**  
Tracks files with names containing Unicode characters, spaces, tabs, and paths at the maximum filesystem path length. Confirms these names are handled correctly through tracking, mirroring, virtual path assignment, and integrity checks.

**failover-09 — Two drive pairs sharing one physical disk**  
Creates two separate drive pairs where the secondary of each pair is on the same physical virtual disk (different subdirectories). Confirms the service handles overlapping physical storage correctly and does not confuse paths between pairs.

**failover-10 — Cross-filesystem matrix (ext4 and xfs)**  
Configures primary and secondary drives on filesystems of different types (ext4 and xfs). Confirms that mirroring works correctly regardless of whether the source and destination filesystems are the same type.

**failover-11 — Device add / hot-insert**  
First uses QMP to hot-remove the replacement-primary device, then confirms that `drives replace assign` correctly fails when the target path is absent. Re-inserts the device via QMP, remounts it, and completes the assignment via the CLI. Runs `sync process` and confirms the tracked file is present on the hot-inserted device.

**failover-12 — QMP hot-remove secondary**  
Uses QMP to hot-remove the secondary (mirror) disk. Runs `integrity check-all --drive-id 1 --recover` after the removal and confirms the output signals that the secondary drive is unavailable. Also confirms `drives show` still reports the active role as `primary`, verifying the service continues to operate in a degraded-but-alive state.

---

## Uninstall

**Entry point:** `tests/installation/bundles/uninstall.sh`  
**Wrapper:** `tests/installation/qemu_uninstall_test.sh`  
**Scenarios:** `tests/installation/scenarios/uninstall/` (4 scenarios)  
**Default ports:** SSH 2226, HTTPS 18447  
**Purpose:** Validates that the package can be cleanly removed without leaving behind service artifacts, and that user data (drive data, custom configs) is preserved after purge.

### Scenarios

**uninstall-01 — Package install verification**  
Confirms the package is correctly installed before any removal steps. Verifies the binary is on the PATH and `bitprotector --version` succeeds.

**uninstall-02 — Package-owned data creation**  
Stops the service, adds a backup destination, and runs `database run` to produce a backup file at the configured path. Verifies the backup file exists. Also creates a user-owned file under `/mnt/primary/docs/` to serve as drive data that must survive the subsequent purge.

**uninstall-03 — Full purge of package-owned assets**  
Runs `apt-get purge bitprotector` and then verifies that all package-owned files are removed: the binary, systemd unit, PAM config, profile hook, default config, and systemd socket. The service is no longer running.

**uninstall-04 — User-drive data preserved after purge**  
After the purge, confirms that the drive directories and their contents (tracked files) still exist on disk. The purge must not delete user data.

---

## Application Workflows

**Entry point:** `tests/installation/bundles/application_workflows.sh`  
**Scenarios:** `tests/installation/scenarios/application-workflows/` (4 scenarios)  
**Purpose:** Validates realistic application-level workflows with a moderate-sized dataset, including scheduled operations over time.

### Scenarios

**application-workflows-01 — Scheduled sync and integrity over a moderate dataset**  
Seeds several hundred files, configures sync and integrity schedules, and waits for both to complete a full cycle. Verifies that all files are mirrored and all integrity results show `ok`.

**application-workflows-02 — Backups during active churn**  
Continuously writes new files and modifies existing ones while a backup is in progress. Confirms the backup completes successfully and is consistent even under active write load.

**application-workflows-03 — Scheduled backup-integrity repair**  
Corrupts one of the two backup files from app-02 (or bootstraps a fresh backup if needed). Enables the backup integrity schedule with a short interval and polls for `last_integrity_status == "repaired"` on the corrupted destination. Verifies the repaired file passes SQLite `integrity_check` and that a `.blake3` hash file is present alongside it.

**application-workflows-04 — Restart schedule persistence**  
Configures several schedules, restarts the service, and confirms the schedules are still present and correctly timed after the restart.

---

## Resilience

**Entry point:** `tests/installation/bundles/resilience.sh`  
**Scenarios:** `tests/installation/scenarios/resilience/` (7 scenarios)  
**Purpose:** Validates that the service handles filesystem-level errors and process signals gracefully rather than crashing or corrupting data.

### Scenarios

**resilience-01 — ENOSPC (disk full)**  
Fills the secondary disk to capacity and then triggers a mirror operation. Confirms the service returns an appropriate error rather than hanging, crashing, or silently producing a truncated file.

**resilience-02 — Read-only mirror**  
Remounts the secondary filesystem as read-only (`mount -o remount,ro`) and triggers a mirror. Confirms the mirror fails cleanly. After remounting the filesystem as read-write, confirms a subsequent mirror succeeds and `cmp` verifies both copies are identical.

**resilience-03 — Permission errors on a tracked file**  
Removes read permission from an already-tracked file on the primary drive. Runs an integrity check and confirms the service reports the file as unavailable rather than crashing. After `chmod 644` restores the permission, re-running the integrity check with `--recover` succeeds.

**resilience-04 — Symlink loop**  
Creates a symlink that points to `.` (the current directory) inside a tracked folder, creating a traversal loop. Runs `folders scan` under `timeout 10`. Confirms the scan exits within the time limit rather than hanging indefinitely.

**resilience-05 — SIGTERM recovery**  
Sends `SIGTERM` to the CLI `bitprotector sync process` subprocess while it is processing a large sync queue. Confirms the subprocess exits cleanly and the queue remains consistent — a subsequent `sync process` call completes all remaining items without errors.

**resilience-06 — SIGKILL recovery**  
Sends `SIGKILL` to the CLI `bitprotector sync process` subprocess (no chance for cleanup). Confirms the database is left in a consistent state and a subsequent `sync process` call completes all remaining items with no pending or in-progress items left in the queue.

**resilience-07 — Panic recovery**  
Sends `SIGSEGV` directly to the running bitprotector service process. Confirms systemd detects the crash, restarts the service, and the service returns to the `active` state. The expected crash signal is whitelisted so the subsequent journal scrape step does not flag it as an unexpected error.

**Journal scrape (shared step)**  
After all resilience scenarios have run, calls the shared `journal_error_scraper` helper from `tests/installation/lib/scenarios.sh`. Asserts that no unexpected error-level journal entries from the service exist since the bundle started, confirming that only the intentional errors produced by each scenario were logged.

---

## Upgrade

**Entry point:** `tests/installation/bundles/upgrade.sh`  
**Scenarios:** `tests/installation/scenarios/upgrade/` (3 scenarios)  
**Environment:** Requires `BASELINE_DEB` pointing to the previous tagged release build  
**Purpose:** Validates that upgrading from an older package version to the current version preserves user data and configuration, keeps the database structurally valid, preserves schema compatibility, and keeps post-upgrade restore behavior functional.

### Scenarios

**upgrade-01 — Baseline to current with live data**  
Installs the baseline package (previous tagged release), creates drive pairs and tracked files, and then upgrades to the current package. Confirms the service starts correctly, tracked data remains visible, and post-upgrade integrity checks still run. Also verifies SQLite integrity before and after upgrade, validates key schema shape expectations (including `sync_queue` support for `adopt_mirror`), and asserts a post-upgrade write path by tracking a newly created file.

**upgrade-02 — Reinstall config preservation**  
Installs and configures the package, then reinstalls (not upgrade — same version) the package. Confirms the user's config file is preserved by the maintainer script and not overwritten with the package default.

**upgrade-03 — Restore-path compatibility after upgrade**  
Creates an upgrade database, snapshots a pre-change restore source, upgrades the package, stages a database restore, restarts the service to apply the staged restore, and verifies restore semantics by confirming post-snapshot data is rolled back while baseline data remains intact. Then performs a post-restore write assertion by tracking a new file and running integrity checks to confirm ongoing write/read compatibility.

---

## Degraded Boot

**Entry point:** `tests/installation/bundles/degraded_boot.sh`  
**Scenarios:** `tests/installation/scenarios/degraded-boot/` (2 scenarios)  
**Purpose:** Validates that the service starts and operates correctly even when expected external resources (drive mount points) are missing at boot time.

### Scenarios

**degraded-boot-01 — Fake mount point**  
Registers a drive pair whose primary path is a plain directory (not backed by a mounted filesystem). Marks the primary as failed through the CLI replacement workflow. Confirms that `drives show` reports the primary state as `failed` and that `bitprotector status` reports a degraded system.

**degraded-boot-02 — Absent device at boot**  
Registers a drive pair whose primary path does not exist on disk (simulating a missing device that was never mounted). Marks the primary as failed through the CLI. Confirms the service is still active and `bitprotector status` reports a degraded system, demonstrating that the service starts and operates correctly when a registered drive path is absent.

---

## Drive Media Type

**Entry point:** `tests/installation/bundles/drive_media_type.sh`  
**Scenarios:** Smoke scenarios 14 and 15  
**Purpose:** Validates the media type classification API and CLI and verifies that parallel integrity runs correctly report `active_workers`.

This bundle reuses existing smoke scenarios rather than adding new scenario files. It provisions the VM identically to the smoke bundle and then executes the two media-type-specific scenarios.

**Scenario 14 — Media type API coverage**  
Creates a drive pair via the CLI with explicit `--primary-media-type ssd` and `--secondary-media-type hdd` flags. Verifies the API returns the correct types. Updates the primary media type via a `PUT` request and confirms the updated value is returned by the API.

**Scenario 15 — Parallel integrity `active_workers`**  
Starts an integrity run on a drive pair with many files so it takes time to complete. While the run is in progress, polls `GET /api/v1/integrity/runs/active` and confirms the `active_workers` field reflects the number of concurrent integrity workers processing files.

---

## Scale (nightly)

**Entry point:** `tests/installation/bundles/scale.sh`  
**Scenarios:** `tests/installation/scenarios/scale/` (2 scenarios)  
**Cadence:** Nightly only — not run on every pull request  
**Purpose:** Validates correct behavior under large-scale data conditions.

### Scenarios

**scale-01 — 100k file scan, sync, and integrity**  
Creates 100,000 files in a tracked folder. Runs a folder scan, then `sync process`, then `integrity check-all --recover`, capturing wall-clock seconds for each phase. Verifies all three operations complete without error, providing a timing baseline that guards against performance regressions at scale.

**scale-02 — Inotify saturation**  
Creates 5,000 subdirectories each containing a file, adds them under a tracked folder, and runs a folder scan. Reads the current `inotify.max_user_watches` kernel parameter as a diagnostic, then confirms the service unit is still active after the scan completes.

---

## Scale Lowmem (nightly)

**Entry point:** `tests/installation/bundles/scale_lowmem.sh`  
**Scenarios:** `tests/installation/scenarios/scale-lowmem/` (1 scenario)  
**Cadence:** Nightly only  
**VM memory:** 1 GB (reduced from the standard 2 GB)  
**Purpose:** Validates that the service can process a large dataset on a memory-constrained host.

**scale-lowmem-01 — 4 GB dataset under 1 GB RAM**  
Seeds 8 × 512 MB files (4 GB total) using `fallocate` and tracks each one. Runs `sync process` and `integrity check-all --recover`. Verifies that no `Killed process` OOM event appears in `dmesg`. Optionally checks the running service's RSS from `/proc/[pid]/status` and asserts it stays below 300 MB.

---

## Scheduled Load (nightly)

**Entry point:** `tests/installation/bundles/scheduled_load.sh`  
**Scenarios:** `tests/installation/scenarios/scheduled-load/` (2 scenarios)  
**Cadence:** Nightly only  
**Purpose:** Validates the scheduler's performance under high task load.

### Scenarios

**scheduled-load-01 — 12k+ file load with sync and integrity timing**  
Seeds 12,000+ files (100 files across 120 subdirectories), adds a sync schedule and an integrity schedule, and measures wall-clock time for scan, sync, and integrity phases. Verifies all phases complete without error, providing a timing baseline for scheduler and integrity throughput under realistic file counts.

**scheduled-load-02 — Backup and repair under load**  
Triggers backup and integrity repair operations while the scheduler is processing a high volume of concurrent tasks. Verifies that the repair and backup operations complete correctly and within a reasonable time even under scheduler pressure.
