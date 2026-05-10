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
**Scenarios:** `tests/installation/scenarios/smoke/` (14 of 16 files sourced)  
**Default ports:** SSH 2222, HTTPS 18443  
**Purpose:** Validates a successful package installation and the core service behaviors that must work before any other testing is meaningful. This bundle is the first gate in CI and the quickest signal that a build is fundamentally broken.

### Scenarios

**smoke-01 — Package installed**  
Verifies the `bitprotector` binary is on the PATH and responds to `--version`. Confirms the package was installed by cloud-init before any other assertions.

**smoke-02 — Service active with TLS**  
Confirms the `bitprotector.service` systemd unit is `active` (running) and that the HTTPS endpoint responds to a health check. This is the foundation for all API-based scenarios.

**smoke-03 — CLI smoke**  
Runs basic CLI commands (`drives list`, `files list`) against the service database to confirm the CLI is functional and the database was initialized correctly by the service.

**smoke-04 — Profile.d installed**  
Verifies the `profile.d` hook script that sets `BITPROTECTOR_DB` is installed at the expected path. This script is what makes the CLI usable without passing `--db` on every invocation.

**smoke-05 — Profile.d execution**  
Sources the profile hook in a login shell and confirms that `BITPROTECTOR_DB` is set to the correct database path in the resulting environment.

**smoke-06 — ldd version sanity**  
Runs `ldd` against the binary and confirms it links against the expected glibc version. This guards against accidental cross-compilation to an incompatible glibc version.

**smoke-07 — Journald integration**  
Triggers a service action that produces a structured log entry and then reads it back from the system journal via `journalctl`. Confirms that the service writes to journald and the log entry has the expected fields.

**smoke-08 — PAM login**  
Authenticates against the API using the PAM-backed credentials (`testauth` / `hunter2`) that were provisioned via cloud-init. Confirms that the PAM module is correctly installed and the service correctly delegates password validation to PAM.

**smoke-09 — JWT persists across restart**  
Issues a JWT via the login endpoint, restarts the `bitprotector.service` unit, and then uses the same JWT to make an authenticated API request. Confirms that JWTs are valid based on the secret key in the config file rather than in-memory state.

**smoke-10 — TLS cert rotation**  
Replaces the TLS certificate and key files on disk and sends `SIGHUP` (or restarts the service) to reload the TLS configuration. Confirms the new certificate is served without requiring a full reboot.

**smoke-11 — Path traversal rejected**  
Sends API requests with paths containing `..` components designed to escape the drive root. Confirms all such requests are rejected with `400 Bad Request` rather than being resolved to a path outside the intended root.

**smoke-12 — Reboot persistence**  
Reboots the VM and waits for the SSH server to return. After reboot, confirms that the service is running, tracked data is still present in the database, and the database file is at the expected path. This catches data stored in non-persistent locations such as `/tmp`.

**smoke-13 — Database backup, repair, and staged restore**  
Creates a drive pair and tracks a file, then runs a manual database backup. Verifies the backup file exists at the configured destination. Checks the backup integrity. Stages a restore from the backup file and confirms the staged-restore indicator is visible in the API response.

**smoke-16 — Scheduled sync, integrity, and database backup sweep**  
Configures schedules for sync, integrity check, and database backup with short intervals, then waits for all three to trigger and complete. Confirms the scheduler correctly dispatches each task type and that results are visible in the respective API endpoints afterward.

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
Simulates an unplanned disk loss by using the QEMU Machine Protocol to disconnect the primary disk device while the service is running. Confirms the service detects the loss, switches to the secondary as the active drive, and continues to serve reads from the secondary.

**failover-03 — Bit-flip corruption and auto-repair**  
Corrupts a single byte in the secondary copy of a tracked file. Runs an integrity check with auto-recovery enabled. Confirms the service detects the mismatch, re-mirrors from the primary, and the repaired secondary file matches the primary.

**failover-04 — Both copies corrupted**  
Corrupts both the primary and secondary copies of a tracked file. Runs an integrity check. Confirms the service reports the file as unrecoverable (both copies differ from the stored checksum) rather than silently re-mirroring a corrupted file.

**failover-05 — Large file streaming**  
Tracks and mirrors a large file (multiple gigabytes). Confirms the mirror completes correctly, the checksum matches, and the service does not run out of memory or timeout during the operation.

**failover-06 — Integrity-triggered auto-recovery**  
Configures an integrity schedule to run automatically. Corrupts a secondary file and waits for the scheduled integrity run to execute. Confirms the auto-recovery triggered by the schedule repairs the file without manual intervention.

**failover-07 — Virtual-path folder retarget after failover**  
A folder with a virtual path is tracked on the primary. After failover to the secondary, the folder's virtual path retarget is verified: files exposed at the virtual path now read from the secondary.

**failover-08 — Unicode, whitespace, and long paths**  
Tracks files with names containing Unicode characters, spaces, tabs, and paths at the maximum filesystem path length. Confirms these names are handled correctly through tracking, mirroring, virtual path assignment, and integrity checks.

**failover-09 — Two drive pairs sharing one physical disk**  
Creates two separate drive pairs where the secondary of each pair is on the same physical virtual disk (different subdirectories). Confirms the service handles overlapping physical storage correctly and does not confuse paths between pairs.

**failover-10 — Cross-filesystem matrix (ext4 and xfs)**  
Configures primary and secondary drives on filesystems of different types (ext4 and xfs). Confirms that mirroring works correctly regardless of whether the source and destination filesystems are the same type.

**failover-11 — Device add / hot-insert**  
Uses QMP to hot-insert a new disk device into the running VM. Registers the new device as a replacement drive via the API. Confirms the hot-inserted device is correctly recognized and can be used as a replacement without rebooting.

**failover-12 — QMP hot-remove secondary**  
Uses QMP to hot-remove the secondary disk while a mirror operation is in progress. Confirms the service handles the removal gracefully — the in-progress mirror fails cleanly and the sync queue item is marked for retry rather than leaving the service in a broken state.

---

## Uninstall

**Entry point:** `tests/installation/bundles/uninstall.sh`  
**Wrapper:** `tests/installation/qemu_uninstall_test.sh`  
**Scenarios:** `tests/installation/scenarios/uninstall/` (4 scenarios)  
**Default ports:** SSH 2226, HTTPS 18447  
**Purpose:** Validates that the package can be cleanly removed without leaving behind service artifacts, and that user data (drive data, custom configs) is preserved after purge.

### Scenarios

**uninstall-01 — Package install verification**  
Confirms the package is correctly installed before any removal steps. Verifies the binary, service, config, and profile hook are all present.

**uninstall-02 — Package-owned data creation**  
Creates a tracked drive pair, some tracked files, and a database backup configuration. This establishes user data that should survive the purge.

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
Creates a backup, corrupts the backup file, and triggers a scheduled integrity check on the backup. Confirms the check detects the corruption and reports the backup as invalid.

**application-workflows-04 — Restart schedule persistence**  
Configures several schedules, restarts the service, and confirms the schedules are still present and correctly timed after the restart.

---

## Resilience

**Entry point:** `tests/installation/bundles/resilience.sh`  
**Scenarios:** `tests/installation/scenarios/resilience/` (8 scenarios)  
**Purpose:** Validates that the service handles filesystem-level errors and process signals gracefully rather than crashing or corrupting data.

### Scenarios

**resilience-01 — ENOSPC (disk full)**  
Fills the secondary disk to capacity and then triggers a mirror operation. Confirms the service returns an appropriate error rather than hanging, crashing, or silently producing a truncated file.

**resilience-02 — Read-only mirror**  
Makes the secondary directory read-only (`chmod -R a-w`) and triggers a mirror. Confirms the service correctly reports a permission error for each affected file and does not crash.

**resilience-03 — Permission errors**  
Removes execute permission from a directory on the primary drive, making subdirectory listing impossible. Confirms the service handles the permission error during a folder scan without crashing.

**resilience-04 — Symlink loop**  
Creates a symlink loop in a tracked folder (a symlink pointing to a parent directory). Triggers a folder scan. Confirms the service detects the loop, skips the problematic entry, and does not hang or run indefinitely.

**resilience-05 — SIGTERM recovery**  
Sends `SIGTERM` to the service process while a long-running mirror is in progress. Confirms the service shuts down cleanly, the partial mirror does not corrupt the secondary file, and the sync queue item is correctly marked for retry on next start.

**resilience-06 — SIGKILL recovery**  
Sends `SIGKILL` to the service process (no chance for cleanup). Confirms the service restarts successfully via systemd, detects any partially completed operations, and continues from a consistent state.

**resilience-07 — Panic recovery**  
Triggers an internal panic (via a test-only debug endpoint). Confirms systemd restarts the service and the database is intact after the restart.

**resilience-08 — Journal scrape**  
After running several scenarios that produce errors, scrapes the system journal and confirms that each expected error event was logged with the correct severity level and message. This validates the logging infrastructure under error conditions.

---

## Upgrade

**Entry point:** `tests/installation/bundles/upgrade.sh`  
**Scenarios:** `tests/installation/scenarios/upgrade/` (2 scenarios)  
**Environment:** Requires `ALPHA1_DEB` pointing to an older build  
**Purpose:** Validates that upgrading from an older package version to the current version preserves user data and configuration.

### Scenarios

**upgrade-01 — Alpha1 to current with live data**  
Installs the older alpha package, creates drive pairs and tracked files, and then upgrades to the current package. Confirms all tracked data is still present and accessible after the upgrade, and the service starts correctly with the upgraded binary.

**upgrade-02 — Reinstall config preservation**  
Installs and configures the package, then reinstalls (not upgrade — same version) the package. Confirms the user's config file is preserved by the maintainer script and not overwritten with the package default.

---

## Degraded Boot

**Entry point:** `tests/installation/bundles/degraded_boot.sh`  
**Scenarios:** `tests/installation/scenarios/degraded-boot/` (2 scenarios)  
**Purpose:** Validates that the service starts and operates correctly even when expected external resources (drive mount points) are missing at boot time.

### Scenarios

**degraded-boot-01 — Missing mount points at boot**  
Boots the VM without the drive mount points that the tracked drive pairs expect. Confirms the service starts successfully and reports the missing mounts in the API status rather than failing to start.

**degraded-boot-02 — Fake mount points at boot**  
Creates the expected mount point directories but does not mount actual filesystems on them. Confirms the service starts and correctly identifies that the drives are not properly mounted.

---

## Drive Media Type

**Entry point:** `tests/installation/bundles/drive_media_type.sh`  
**Scenarios:** Smoke scenarios 14 and 15  
**Purpose:** Validates the media type classification API and CLI and verifies that parallel integrity runs correctly report `active_workers`.

This bundle reuses existing smoke scenarios rather than adding new scenario files. It provisions the VM identically to the smoke bundle and then executes the two media-type-specific scenarios.

**Scenario 14 — Media type API and CLI coverage**  
Creates drive pairs with explicit `primary_media_type` and `secondary_media_type` values. Verifies the API returns the correct types, the CLI displays them, and the types are persisted correctly when updated.

**Scenario 15 — Parallel integrity `active_workers`**  
Starts an integrity run on a drive pair with many files so it takes time to complete. While the run is in progress, polls `GET /api/v1/integrity/runs/active` and confirms the `active_workers` field reflects the number of concurrent integrity workers processing files.

---

## Scale (nightly)

**Entry point:** `tests/installation/bundles/scale.sh`  
**Scenarios:** `tests/installation/scenarios/scale/` (2 scenarios)  
**Cadence:** Nightly only — not run on every pull request  
**Purpose:** Validates correct behavior under large-scale data conditions.

### Scenarios

**scale-01 — 100k file scan**  
Creates 100,000 files in a tracked folder and runs a folder scan. Verifies the scan completes within a reasonable time budget, all files are discovered, and the tracking endpoint correctly serves paginated results across the full dataset.

**scale-02 — Inotify saturation**  
Exceeds the default `inotify.max_user_watches` limit to verify the service handles inotify exhaustion gracefully rather than crashing or silently stopping to watch files.

---

## Scale Lowmem (nightly)

**Entry point:** `tests/installation/bundles/scale_lowmem.sh`  
**Scenarios:** `tests/installation/scenarios/scale-lowmem/` (1 scenario)  
**Cadence:** Nightly only  
**VM memory:** 1 GB (reduced from the standard 2 GB)  
**Purpose:** Validates that the service can process a large dataset on a memory-constrained host.

**scale-lowmem-01 — Large dataset under 1 GB RAM**  
Seeds a large number of tracked files and runs sync and integrity operations. Verifies these operations complete correctly without triggering the OOM killer. Monitors memory usage during the run and confirms it stays within the 1 GB limit.

---

## Scheduled Load (nightly)

**Entry point:** `tests/installation/bundles/scheduled_load.sh`  
**Scenarios:** `tests/installation/scenarios/scheduled-load/` (2 scenarios)  
**Cadence:** Nightly only  
**Purpose:** Validates the scheduler's performance under high task load.

### Scenarios

**scheduled-load-01 — 10k+ scheduler load timing**  
Configures a large number of schedules (10,000+) and measures how long the scheduler takes to evaluate all schedules in a single cycle. Verifies the evaluation time stays within a reasonable budget, guarding against O(n) or worse scheduler complexity.

**scheduled-load-02 — Backup and repair under load**  
Triggers backup and integrity repair operations while the scheduler is processing a high volume of concurrent tasks. Verifies that the repair and backup operations complete correctly and within a reasonable time even under scheduler pressure.
