# Rust Unit Tests

Rust unit tests in BitProtector live as `#[cfg(test)]` modules embedded at the bottom of each source file in `src/`. This placement keeps tests next to the code they exercise, makes it easy to access private functions without re-exporting them, and eliminates the need to manage a separate test file per module.

Run them all with:

```bash
cargo test --lib
```

---

## Table of Contents

- [Why Inline Unit Tests](#why-inline-unit-tests)
- [Mocking Strategy](#mocking-strategy)
- [Modules with Meaningful Unit Coverage](#modules-with-meaningful-unit-coverage)
  - [db/repository.rs](#dbrepositoryrs)
  - [core/drive.rs](#coredrivers)
  - [core/checksum.rs](#corechecksumrs)
  - [core/mirror.rs](#coremirrorrs)
  - [core/virtual_path.rs](#corevirtual_pathrs)
  - [core/tracker.rs](#coretrackerrs)
  - [core/change_detection.rs](#corechange_detectionrs)
  - [core/integrity.rs](#coreintegrityrs)
  - [core/scheduler.rs](#coreschedulerrs)
  - [api/server.rs](#apiserverrs)
  - [api/path_resolution.rs](#apipath_resolutionrs)
  - [logging/event_logger.rs](#loggingevent_loggerrs)

---

## Why Inline Unit Tests

Inline unit tests have direct access to private functions and types without any re-exporting. The alternative — separate files under `tests/` — would require making every function under test public, which would pollute the public API and weaken encapsulation. Keeping tests inline also means that when you read a function, its tests are a scroll away, making it easier to understand intent.

---

## Mocking Strategy

Several modules depend on the `Repository` trait to read and write database state. Rather than spinning up a real SQLite database for every unit test, these tests use `mockall` to generate a mock implementation of the trait at compile time.

The mock is activated through a `#[cfg_attr(test, mockall::automock)]` attribute on the trait definition. This means the mock type is only compiled in test builds and does not appear in production binaries.

Each mock test sets up expectations on specific method calls — specifying the arguments to expect and the return value to produce — and then passes the mock to the function under test. `mockall` verifies at the end of the test that all expected calls were made.

For tests that need a real database (CRUD correctness, schema migrations), an in-memory SQLite pool is used instead. The pool is created fresh for each test, so there is no shared state between tests.

---

## Modules with Meaningful Unit Coverage

### db/repository.rs

The repository module has the most extensive unit test suite. Because every other module ultimately reads and writes through the repository, correctness here is foundational.

**What is tested:**

- **Drive pair CRUD**: creating, listing, fetching, updating name and paths, and deleting. Deletion is tested both in the empty case (succeeds) and when tracked files exist (must fail with an error — the repository enforces referential integrity at the application layer since SQLite foreign key enforcement is explicit).
- **Tracked file CRUD**: creating tracked files, listing with pagination, updating checksum and mirror status, and deleting.
- **Folder CRUD**: creating tracked folders, updating virtual path, scanning (updating last-scanned timestamp), and deleting.
- **Tracking item filters**: `list_tracking_items` is tested with combinations of filters — drive pair ID, mirror status, virtual path prefix, item kind, and source type (direct vs folder-based) — and with pagination.
- **Sync queue**: enqueue, mark completed or failed, clear completed items (verified to preserve pending and in-progress items), and list with status filters.
- **Event log**: appending log entries and retrieving with filters.
- **Schedule config CRUD**: creating, updating (cron expression, interval, enabled flag, max duration), and deleting schedule records.
- **Database backup configs**: CRUD for backup destination records, settings, and integrity status.
- **System status**: the aggregate status query (`get_system_status`) is verified to return the correct counts for tracked files, mirrored files, and drive pairs.
- **Concurrency**: the connection pool is tested under concurrent access from multiple threads to confirm that the pool correctly serializes reads and does not deadlock.
- **Transaction isolation**: inserting a row that violates a foreign key constraint is verified to return an error and leave no row in the table after rollback.

### core/drive.rs

The drive module implements the state-machine logic for drive pair replacement workflows. It sits above the repository and enforces the valid transition sequence: active → quiescing → failed → rebuilding → active.

**What is tested:**

- Role helper methods correctly identify which of the two drives is active and which is standby based on the `active_role` field.
- The quiesce-confirm-assign flow is verified end to end: `mark_drive_quiescing` transitions the drive to quiescing, `confirm_drive_failure` advances to failed and flips the active role to the surviving drive, and `assign_replacement_drive` sets the replacement path, transitions to rebuilding, and returns the queued rebuild count.
- `cancel_drive_quiescing` returns a quiescing drive to active without side effects.
- `load_operational_pair` performs emergency failover when the primary drive root marker is missing: it transitions the pair to failed state, flips active role to secondary, and retargets virtual path symlinks to the secondary drive.
- The missing-root-marker check is tested in isolation: a pair whose primary root marker has been removed triggers failover when checked via `maybe_emergency_failover`.
- `maybe_finalize_rebuild_for_action` is tested with a fully rebuilt pair: once all `restore_master` sync items are completed, the function transitions both drive states back to active and restores the primary active role.

### core/checksum.rs

The checksum module computes BLAKE3 digests of files on disk.

**What is tested:**

- `checksum_bytes`: a known input produces the expected BLAKE3 digest; the empty input produces the correct empty-input digest; two different byte slices produce different digests; the same input always produces the same output.
- Hex output format: the digest is always 64 lowercase hexadecimal characters.
- `checksum_file`: a file with known content produces the same digest as `checksum_bytes` on those bytes, for both small and large files (200 KB tested with chunked streaming). A nonexistent path returns an error.
- `verify_file`: returns `true` when the file's checksum matches the stored value; returns `false` on a mismatch.
- `ChecksumStrategy` selection: HDDs always use `Streaming` regardless of file size; SSDs use `MmapRayon` for small files and `Streaming` for files above the size threshold.
- `copy_with_checksum`: copies a file and returns matching source and destination checksums; the destination bytes equal the source bytes.
- `copy_and_verify_checksum`: copies a file and succeeds when the destination checksum matches the provided expected value; fails and removes the destination file when the checksums do not match.

### core/mirror.rs

The mirror module copies files from the active drive to the standby drive and handles the restore path (copying back from standby to active after a rebuild).

**What is tested:**

- `validate_drive_pair`: a valid pair with distinct existing directories is accepted; a nonexistent primary returns an error; a nonexistent secondary returns an error; primary and secondary pointing to the same path returns an error.
- `mirror_file`: copies a file from the active drive to the standby and returns the BLAKE3 checksum of the copied bytes; the destination content exactly matches the source. For files in nested subdirectories, the full intermediate directory tree is created on the secondary. A nonexistent source file returns an error.
- `restore_from_mirror`: copies a file from secondary back to primary (the rebuild path) and the restored bytes match the original.

### core/virtual_path.rs

Virtual paths are symlinks or aliases that expose tracked files at an application-defined location outside the physical drive root, so consuming applications do not need to know where data physically lives.

**What is tested:**

- `normalize_virtual_path`: paths containing `..` are rejected; double slashes are collapsed; normalization is idempotent (a property test verifies this for arbitrary inputs) and always produces an absolute path that does not end in `/`.
- `set_virtual_path` on a tracked file: creates a symlink on disk pointing to the file's active drive path and records the virtual path in the database. Setting a virtual path to a location occupied by a regular (non-symlink) file is rejected with an error.
- `remove_virtual_path`: deletes the on-disk symlink and clears the virtual path record in the database.
- `set_folder_virtual_path`: creates a directory symlink pointing to the tracked folder's directory on the active drive.
- Overlap validation: assigning a virtual path to a file that falls inside a folder's virtual path subtree is rejected to prevent nested symlinks.
- Symlink refresh after failover: when `load_operational_pair` detects a missing primary root marker and fails over to secondary, all virtual path symlinks for files and folders in the pair are retargeted to the secondary drive paths.

### core/tracker.rs

The tracker module handles file and folder registration on the active drive (metadata capture and optional virtual-path linking). Queueing for initial mirroring is covered by API/CLI handlers and by `auto_track_folder_files`.

**What is tested:**

- `track_file`: creates a database record with the correct relative path, BLAKE3 checksum, and file size; `is_mirrored` is initially false. Providing a virtual path simultaneously calls `set_virtual_path` and creates the symlink. Attempting to track a nonexistent file returns an error.
- `track_folder`: creates the folder record and returns it with no `last_scanned_at` timestamp. Providing a virtual path creates the directory symlink. Attempting to track a nonexistent directory returns an error.
- `auto_track_folder_files`: discovers all files in a folder tree recursively (verified for files nested at arbitrary depth), creates tracked file records, enqueues an `adopt_mirror` item for each, and stamps `last_scanned_at` on the folder. Files that are already tracked are skipped without creating duplicate records or queue entries. A dedicated test (`test_auto_track_folder_queues_adopt_mirror`) confirms the action is `adopt_mirror` and not `mirror`, ensuring files already present on the standby are not unnecessarily re-copied.

### core/sync_queue.rs

The sync queue module processes queue items, dispatching to the appropriate core action (mirror, restore, verify, etc.) and updating item status and file mirror state afterwards.

**What is tested (adopt_mirror behaviour):**

- **Standby already matches** (`test_process_adopt_mirror_standby_matches`): when the standby file exists and its BLAKE3 checksum matches the stored master checksum, `adopt_mirror` marks the item completed and sets `is_mirrored = true` without overwriting the secondary file.
- **Standby is stale** (`test_process_adopt_mirror_standby_stale`): when the standby file exists but has different content, `adopt_mirror` performs a full copy from primary to secondary and marks the file mirrored. The secondary content matches the primary after processing.
- **Standby is missing** (`test_process_adopt_mirror_standby_missing`): when no file exists at the standby path, `adopt_mirror` copies the primary file to the secondary and marks the file mirrored.

---

### core/change_detection.rs

The change detection module watches for files that have changed since their last mirror and re-queues them.

**What is tested:**

- `detect_change`: returns `None` when the on-disk checksum matches the stored value; returns `Some(new_hash)` when the file content has changed; returns `None` (not an error) when the file no longer exists on disk.
- `scan_all_changes`: detects modified files across a drive pair and excludes unchanged files from the result set.
- `scan_and_record_changes`: after a failover to the secondary drive, changes made on the secondary are detected and the tracked file's stored checksum is updated to the new value; the file's `is_mirrored` flag is cleared and no sync queue item is created (the secondary copy is the authoritative source after failover).

### core/integrity.rs

The integrity module verifies that the primary and secondary copies of each tracked file are consistent with each other and with the stored checksum.

**What is tested:**

- `check_file_integrity` status values: `Ok` (both copies match stored checksum); `MirrorCorrupted` (secondary differs from checksum); `MasterCorrupted` (primary differs from checksum); `BothCorrupted` (neither copy matches); `MirrorMissing` (secondary file absent); `MasterMissing` (primary file absent); `PrimaryDriveUnavailable` (primary drive directory is gone). A degraded pair with `primary_state = "failed"` and `active_role = "secondary"` returns `Ok` when the secondary copy is intact, without attempting to read the unavailable primary.
- `attempt_recovery`: restores the mirror from the intact primary when the result is `MirrorCorrupted`; restores the primary from the intact mirror when the result is `MasterCorrupted`; returns `false` (no action) when the result is `Ok` or `BothCorrupted`.

### core/scheduler.rs

The scheduler module manages timed and cron-based task execution (sync sweeps, integrity runs, database backups).

**What is tested:**

- `run_task` with `TaskType::Sync` processes all pending sync queue items: a pending mirror item is executed and the file appears on the secondary drive.
- `run_task` with `TaskType::IntegrityCheck` runs an integrity check across all drive pairs and persists attention rows in the database for files with issues; a `mirror_missing` row is verified in the latest integrity run's results.
- `Scheduler::reload`: spawning a schedule with a short interval (1 second) starts a background thread that fires the task and mirrors a file. Disabling a schedule in the database and calling `reload` again stops the thread for that schedule.
- `next_cron_sleep_ms`: a valid cron expression returns a non-negative sleep duration in milliseconds no greater than 24 hours; an invalid expression returns an error.

### api/server.rs

The API server module contains inline async tests (using `actix_rt::test`) that verify route configuration, authentication, middleware, and the drive replacement flow end to end.

**What is tested:**

- **Route availability**: `GET /api/v1/drives`, `GET /api/v1/logs`, and `GET /api/v1/database/backups` return `200` with a valid token. An unknown path returns `404`.
- **Authentication**: any protected endpoint called without a token returns `401`; a valid JWT on `GET /api/v1/auth/validate` returns `200`.
- **Error response format**: a `GET /api/v1/drives/{id}` with a nonexistent ID returns `404` with a JSON body containing an `error` object with `code` and `message` string fields.
- **CORS**: when the CORS middleware is configured, a request carrying an `Origin` header receives an `access-control-allow-origin` response header.
- **Rate limiter**: `RateLimiter` allows requests up to its configured limit and blocks subsequent requests once the limit is exceeded within the window.
- **API versioning**: a request to `/drives` (without the `/api/v1` prefix) returns `404`; the same request to `/api/v1/drives` with a valid token returns `200`.
- **Status route**: `GET /api/v1/status` returns `200` with a body containing `degraded_pairs` and `active_secondary_pairs` numeric fields.
- **Drive replacement flow**: a combined test exercises `POST /api/v1/drives/{id}/replacement/mark` → `POST .../replacement/cancel` → mark again → `POST .../replacement/confirm` → `POST .../replacement/assign`. After mark the primary state is `quiescing`; after cancel it returns to `active`; after confirm `active_role` is `secondary`; after assign `primary_state` is `rebuilding` and `queued_rebuild_items` is `1`.
- **Frontend static serving**: when a frontend directory is provided to `configure_application`, `GET /` serves `index.html`; unknown client-side routes fall back to `index.html` (SPA fallback); API routes under `/api/v1` are not shadowed by the static file handler.

### api/path_resolution.rs

The path resolution module validates that an absolute path submitted by a client is actually under the drive pair's active root before accepting it as a relative path. This is the boundary check that prevents path traversal.

**What is tested:**

- A relative path and an absolute path inside the drive root are each resolved to the correct relative form.
- A path containing `..` components that would escape the root is rejected.
- A symlink inside the root that resolves to a target outside the root is rejected.
- Paths with leading/trailing whitespace and Unicode characters are accepted; whitespace is trimmed.
- A nonexistent path is rejected.
- The drive root path itself is rejected.
- A path targeting the wrong kind (file path where a directory is expected, or vice versa) is rejected.
- A property test confirms that for any arbitrary input accepted by `resolve_path_within_drive_root`, the resolved absolute path is canonically within the drive root.

### logging/event_logger.rs

The event logger appends structured entries to the database for every significant operation.

**What is tested:**

- `log_event`: a plain log entry is stored and retrievable; entries with a `details` string preserve that string exactly; entries can be filtered by `event_type`.
- **Typed log functions**: `log_file_tracked`, `log_file_mirrored`, `log_integrity_pass`, `log_integrity_fail`, `log_recovery`, `log_sync_completed`, and `log_sync_failed` each produce an entry with the correct `event_type` and are verified in sequence.
- **Additional typed functions**: `log_file_untracked` and `log_folder_tracked`/`log_folder_untracked` store paths and IDs in the message and JSON details; `log_both_corrupted` stores all three checksums in JSON details (null when absent); `log_integrity_run_started` and `log_integrity_run_completed` store run ID, file count, trigger, and issue/recovery counts; `log_drive_created`, `log_drive_updated`, `log_drive_deleted`, and `log_drive_failover` store the relevant drive pair metadata.
- **Filtering by file ID**: `list_event_logs` with a `tracked_file_id` filter returns only entries for that file.
- **`file_path` join**: entries linked to a tracked file have `file_path` populated with the full absolute path via a join with `tracked_files` and `drive_pairs`; system-level entries with no file ID have `file_path` as `null`.
- **JSON details**: all enriched log functions produce a non-null `details` field containing a valid JSON object.
- **Sync message content**: `log_sync_completed` includes the file path and action in its message; `log_sync_failed` includes the file path and error string in message and JSON details.
