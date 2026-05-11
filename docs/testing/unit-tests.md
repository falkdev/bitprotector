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
- **Tracked file CRUD**: creating tracked files, listing with various filters (drive pair, source, virtual path prefix), updating checksum and mirror status, and deleting.
- **Folder CRUD**: creating tracked folders, listing, updating virtual path, scanning (updating last-scanned timestamp), and deleting.
- **Virtual path management**: setting, clearing, and resolving effective virtual paths — which differ between files tracked directly versus files discovered through folder scans.
- **Sync queue**: enqueue, dequeue, mark completed or failed, list pending items, pause and resume the queue.
- **Event log**: appending log entries with and without associated file IDs, and retrieving entries with pagination and filters.
- **Drive state transitions**: the full lifecycle of drive pair states — active, quiescing, failed, rebuilding, and back to active — is verified at the repository level to confirm the state machine is enforced by the update methods.
- **Concurrency**: the connection pool is tested under concurrent access from multiple threads to confirm that the pool correctly serializes writes and does not deadlock.
- **Database backup configs**: CRUD for backup destination records and settings.
- **Integrity run lifecycle**: creating a run, updating processed/attention counts, completing, and listing results.

### core/drive.rs

The drive module implements the state-machine logic for drive pair replacement workflows. It sits above the repository and enforces the valid transition sequence: active → quiescing → failed → rebuilding → active.

**What is tested:**

- Role helper methods correctly identify which of the two drives is active and which is standby based on the `active_role` field.
- `mark_drive_quiescing` sets the correct state on the correct drive side.
- `confirm_drive_failure` advances to the failed state and flips the active role to the surviving drive.
- `assign_replacement_drive` sets the replacement path, transitions to rebuilding, and returns the number of files queued for rebuild.
- `cancel_drive_quiescing` returns a quiescing drive to active without side effects.
- Invalid transitions (e.g., confirming failure on a drive that is not quiescing) return errors rather than silently corrupting state.

### core/checksum.rs

The checksum module computes BLAKE3 digests of files on disk.

**What is tested:**

- A known input produces the known expected BLAKE3 digest — confirming the algorithm is wired correctly and not accidentally swapped.
- An empty file produces the correct known digest for an empty input.
- A file that is modified between two compute calls produces different digests.
- A path that does not exist returns an error rather than a digest.

### core/mirror.rs

The mirror module copies files from the active drive to the standby drive and handles the restore path (copying back from standby to active after a rebuild).

**What is tested:**

- Mirroring a file that exists on the primary creates it on the secondary at the expected relative path.
- Mirroring a file that does not exist on the primary returns an error and does not create a partial file on the secondary.
- Restoring from secondary to primary (during rebuild) correctly reads from the standby path and writes to the active path.
- The relative path derivation — stripping the drive root from the absolute path — is correct for nested subdirectories.
- A mirror call with the wrong drive pair ID returns an error rather than writing to the wrong location.

### core/virtual_path.rs

Virtual paths are symlinks or aliases that expose tracked files at an application-defined location outside the physical drive root, so consuming applications do not need to know where data physically lives.

**What is tested:**

- Setting a virtual path on a directly tracked file updates the record and is readable back.
- Clearing a virtual path removes the association.
- For files tracked through a folder scan, the effective virtual path is derived from the folder's virtual path plus the file's relative sub-path; this derivation is verified against known inputs.
- The effective virtual path of a file with no virtual path on itself or its parent folder is empty.

### core/tracker.rs

The tracker module handles the queue-first semantics for file and folder tracking: when a file or folder is added, it is recorded and enqueued for mirroring, but the mirror does not happen synchronously.

**What is tested:**

- Tracking a file creates the database record and adds a sync queue entry.
- Attempting to track the same file twice returns an error and does not add a duplicate queue entry.
- Tracking a folder creates the folder record without immediately scanning the directory.
- Scanning a folder discovers new files, creates tracked file records, and enqueues them — but does not copy them.

### core/change_detection.rs

The change detection module watches for files that have changed since their last mirror and re-queues them.

**What is tested:**

- A file whose on-disk checksum still matches the stored checksum is not re-queued.
- A file whose on-disk content has changed is detected and added to the sync queue.
- A file that has been deleted from disk is detected and its record updated accordingly.
- Change detection across multiple files in a folder is correct — changed files are queued, unchanged files are not.

### core/integrity.rs

The integrity module verifies that the primary and secondary copies of each tracked file are consistent with each other and with the stored checksum.

**What is tested:**

- A file whose primary and secondary copies both match the stored checksum produces a passing result.
- A file whose secondary copy has been modified (simulating bit-flip corruption) is flagged as `mirror_corrupted`.
- A file whose primary copy has been modified is flagged as `primary_corrupted`.
- A file missing from the secondary is flagged as `mirror_missing`.
- When auto-recovery is enabled, a `mirror_corrupted` result triggers a re-mirror from primary.

### core/scheduler.rs

The scheduler module manages timed and cron-based task execution (sync sweeps, integrity runs, database backups).

**What is tested:**

- A schedule that has never run is immediately eligible.
- A schedule that ran recently is not re-triggered until its interval elapses.
- A cron schedule is next-triggered at the correct wall-clock time according to the cron expression.
- A disabled schedule is never triggered regardless of elapsed time.
- Concurrent schedule evaluation (multiple schedules due simultaneously) does not produce duplicate triggers.

### api/server.rs

The API server module includes inline tests for the drive replacement HTTP endpoints (`mark`, `cancel`, `confirm`, `assign`). These are the operations that the CLI also exercises, but the inline tests verify the HTTP response shapes and status codes directly without going through the binary.

**What is tested:**

- `POST /api/v1/drives/{id}/mark` returns the updated drive pair with the new state.
- `POST /api/v1/drives/{id}/cancel` restores the pair to active state.
- `POST /api/v1/drives/{id}/confirm` advances to failed state and returns the surviving active role.
- `POST /api/v1/drives/{id}/assign` with a valid replacement path returns the rebuilding pair.
- Calling these endpoints out of sequence (e.g., confirm without mark) returns a `409 Conflict`.

### api/path_resolution.rs

The path resolution module validates that an absolute path submitted by a client is actually under the drive pair's active root before accepting it as a relative path. This is the boundary check that prevents path traversal.

**What is tested:**

- A path that is a subdirectory of the active root is accepted, and the derived relative path is correct.
- A path that is outside the active root is rejected with an appropriate error.
- A path containing `..` components that would escape the root is rejected.
- The root path itself (without a file component) is rejected when a file path is required.

### logging/event_logger.rs

The event logger appends structured entries to the database for every significant operation.

**What is tested:**

- Logging a mirror completion creates a retrievable entry with the correct message, file ID, and timestamp.
- Logging an integrity result creates an entry linked to the correct file.
- Querying entries filtered by file ID returns only entries for that file.
- Pagination of log entries is correct for both the first and subsequent pages.
