# CLI Integration Tests

These tests invoke the compiled `bitprotector` binary through the `assert_cmd` harness and assert on `stdout`, `stderr`, and exit codes. Each test file targets a distinct command group. Every test creates its own isolated temporary database — no state is shared between tests.

For an explanation of the harness and isolation strategy, see [README.md](README.md).

---

## Table of Contents

- [cli_drives.rs — Drive Pair Commands](#cli_drivesrs--drive-pair-commands)
- [cli_files.rs — File Tracking Commands](#cli_filesrs--file-tracking-commands)
- [cli_folders.rs — Tracked Folder Commands](#cli_foldersrs--tracked-folder-commands)
- [cli_virtual_paths.rs — Virtual Path Commands](#cli_virtual_pathsrs--virtual-path-commands)
- [cli_integrity.rs — Integrity Check Commands](#cli_integrityrs--integrity-check-commands)
- [cli_sync.rs — Sync Queue Commands](#cli_syncrs--sync-queue-commands)
- [cli_logs.rs — Event Log Commands](#cli_logsrs--event-log-commands)
- [cli_database.rs — Database Backup Commands](#cli_databasers--database-backup-commands)
- [cli_status.rs — SSH Status Display](#cli_statusrs--ssh-status-display)
- [cli_auth.rs — JWT Middleware and Token Lifecycle](#cli_authrs--jwt-middleware-and-token-lifecycle)

---

## cli_drives.rs — Drive Pair Commands

**File:** `tests/integration/cli_drives.rs`

This file covers the full lifecycle of drive pairs through the CLI, including the planned replacement workflow.

**Empty list:** `drives list` on a fresh database produces the "no drive pairs registered" message and exits cleanly.

**Add and list:** `drives add` with a name, primary path, and secondary path creates a drive pair. The output contains the assigned ID. A subsequent `drives list` shows the pair by name.

**Path validation:** `drives add` with paths that do not exist on disk is rejected with a descriptive error. Tests create real temporary directories to satisfy this validation.

**Rename:** `drives rename` changes the name visible in `drives list`.

**Delete — empty pair:** Deleting a drive pair with no tracked files succeeds.

**Delete — non-empty pair:** Deleting a drive pair that has tracked files is rejected. This ensures the user cannot accidentally lose the only record of which files were tracked.

**Planned replacement workflow:** The sequence `drives mark`, `drives confirm`, `drives assign` is exercised in order, with assertions at each step on the displayed drive state (`quiescing` → `failed` → `rebuilding`). This is the primary CLI coverage for the drive replacement state machine.

**Cancel quiescing:** `drives cancel` on a quiescing pair restores it to `active`. The test verifies the state is correctly reset before the pair would be treated as failed.

**Rebuild completion:** After a `drives assign`, the sync queue is processed and `drives list` is asserted to show the rebuilt pair in the active state. This confirms the end-to-end rebuild path from CLI trigger to completed state.

---

## cli_files.rs — File Tracking Commands

**File:** `tests/integration/cli_files.rs`

Covers the file tracking commands: adding files to tracking, mirroring them, inspecting them, and removing them.

**Track a file:** `files track` with a drive pair ID and a relative path creates a tracked file record and adds it to the sync queue. The output confirms the file was tracked.

**List tracked files:** `files list` shows tracked files with their IDs, paths, and mirror status.

**Mirror a file:** `files mirror` initiates an immediate mirror copy from the active drive to the standby. The test verifies the output confirms the operation and that the file appears in the secondary path.

**Absolute path input:** `files track` accepts an absolute path that resolves under the drive pair's active root and converts it to the stored relative path. An absolute path outside the active root is rejected.

**Remove a file:** `files remove` untracks a file. It no longer appears in `files list`.

**Integrity check result on a file:** The `files list` output includes the `last_integrity_check_at` timestamp for files that have been integrity-checked.

---

## cli_folders.rs — Tracked Folder Commands

**File:** `tests/integration/cli_folders.rs`

Covers the folder tracking commands, including the queue-first scan behavior and active-secondary scanning.

**Add a folder:** `folders add` creates a folder tracking record. The folder is not scanned immediately.

**Scan a folder:** `folders scan` discovers files in the folder's directory, creates tracked file records, and adds them to the sync queue without immediately mirroring. The output reports the number of new and changed files found.

**Mirror a folder:** `folders mirror` processes the sync queue for all files in the folder, copying them to the standby. The output reports how many files were mirrored.

**Active-secondary scanning:** When the active drive is the secondary (after a failover), folder scanning reads from the secondary path. Tests verify this by creating test files on the secondary path before scanning.

**Change detection during scan:** A file that has changed on disk since its last scan is detected as changed and re-added to the sync queue. An unchanged file is not re-queued.

**Virtual path on a folder:** `folders add` accepts an optional `--virtual-path` argument. When set, files discovered in the folder inherit effective virtual paths derived from the folder's virtual root.

**Delete a folder:** `folders remove` removes the folder tracking record. Files previously discovered through the folder remain tracked individually.

---

## cli_virtual_paths.rs — Virtual Path Commands

**File:** `tests/integration/cli_virtual_paths.rs`

Covers setting and clearing virtual paths on directly tracked files.

**Set a virtual path:** `virtual-paths set` associates an absolute virtual path with a tracked file. The path is visible in `files list`.

**Clear a virtual path:** `virtual-paths clear` removes the virtual path association from a file.

**Effective path derivation:** For files discovered through folder scans, the effective virtual path is derived from the folder's virtual root plus the file's relative sub-path. This derivation is confirmed by listing files after setting a virtual path on the parent folder.

**Invalid path format:** Setting a virtual path that does not start with `/` is rejected.

---

## cli_integrity.rs — Integrity Check Commands

**File:** `tests/integration/cli_integrity.rs`

Covers the integrity check commands and result inspection.

**Run an integrity check on all pairs:** `integrity run` triggers a full integrity sweep. The output confirms the run completed and reports the total count of files checked and any issues found.

**Run on a specific pair:** `integrity run --pair-id N` checks only the specified drive pair.

**Results listing:** `integrity results` shows the outcome for each file, including the status (`ok`, `mirror_corrupted`, `mirror_missing`, etc.) and the timestamp.

**Issues-only filter:** `integrity results --issues-only` omits passing files and shows only those that need attention.

**Simulated corruption:** Tests write a different byte sequence to the secondary copy of a file before running integrity. The result for that file is `mirror_corrupted`.

**Auto-recovery:** When `integrity run --recover` is passed and a `mirror_corrupted` file is found, the secondary is re-mirrored from the primary. The subsequent result for that file is `ok`.

---

## cli_sync.rs — Sync Queue Commands

**File:** `tests/integration/cli_sync.rs`

Covers the sync queue management commands.

**List queue:** `sync list` shows pending items with their file IDs, actions, and statuses.

**Process queue:** `sync process` processes all pending items in the queue, mirroring files to the standby. The output reports the number of items processed.

**Pause and resume:** `sync pause` prevents new items from being processed. `sync resume` re-enables processing. Tests verify that `sync process` respects the paused state.

**Manual action items:** Items in `user_action_required` status are listed but not automatically processed. The test verifies the item appears in `sync list` with the correct status.

---

## cli_logs.rs — Event Log Commands

**File:** `tests/integration/cli_logs.rs`

Covers reading the event log from the CLI.

**List all entries:** `logs list` returns recent log entries with timestamps, messages, and associated file IDs where applicable.

**Filter by file ID:** `logs list --file-id N` returns only entries associated with the specified tracked file.

**Pagination:** `logs list --limit N --offset M` returns the correct page of results. Tests verify both the returned entries and the total count.

**Empty log:** On a fresh database, `logs list` returns the "no entries" message and exits cleanly.

---

## cli_database.rs — Database Backup Commands

**File:** `tests/integration/cli_database.rs`

Covers the database backup management commands.

**Add a backup destination:** `database add` registers a backup path and drive label. The destination appears in `database list`.

**List destinations:** `database list` shows all configured backup destinations with their paths and labels.

**Remove a destination:** `database remove` deletes the configuration. The destination no longer appears in `database list`.

**Run a backup:** `database backup` copies the current database file to all enabled destinations. The output reports the paths written.

**Check integrity:** `database check` verifies the integrity of the most recent backup at each destination. The output reports the status per destination.

**Stage a restore:** `database restore --file /path/to/backup.db` stages a backup file for restore on next service restart. The output confirms the restore is staged.

---

## cli_status.rs — SSH Status Display

**File:** `tests/integration/cli_status.rs`

Covers the SSH status display subcommand, which is used by the `bitprotector-status.sh` script to display system health in SSH login banners.

**Basic output:** The status command produces a text summary of system health including drive pair states, tracked file counts, recent sync activity, and any active integrity runs.

**No drives registered:** The output on a fresh database indicates no drive pairs are configured rather than erroring.

**Formatting:** The output is plain text suitable for display in a terminal banner. The test verifies it does not contain any ANSI escape codes that would corrupt a minimal SSH environment.

---

## cli_auth.rs — JWT Middleware and Token Lifecycle

**File:** `tests/integration/cli_auth.rs`

This file uses the in-process actix-web test harness rather than spawning the binary, because it is testing the HTTP authentication middleware rather than CLI commands.

**Valid token accepted:** A request to a protected route with a correctly signed JWT in the `Authorization: Bearer` header is accepted with a `200` response.

**No token rejected:** A request to a protected route with no `Authorization` header is rejected with `401`.

**Malformed token rejected:** A request with a token that has been tampered with (altered signature) is rejected with `401`.

**Expired token rejected:** A token with an `exp` claim in the past is rejected with `401`. The test constructs a token with a very short expiry and waits for it to elapse.

**Token issued for login:** The `POST /api/v1/auth/login` endpoint issues a valid JWT when correct credentials are provided. The issued token can be used to authenticate subsequent requests.

**Logout:** After a `POST /api/v1/auth/logout` with a valid token, the same token is rejected for subsequent requests. This verifies the server-side revocation list.

**Token survives service restart:** A token issued before a service restart remains valid after restart, because validity is derived from the cryptographic secret in the config file rather than in-memory state.
