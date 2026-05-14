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

**Path validation:** `drives add` with paths that do not exist on disk is rejected with a descriptive error. The `--no-validate` flag bypasses this check. Passing the same path for both primary and secondary is also rejected.

**Show a drive pair:** `drives show <id>` prints the full details of one drive pair, including `Primary media:` and `Secondary media:` labels. Requesting an unknown ID exits with failure.

**Media type:** `drives add --primary-media-type ssd` sets the media type label. Passing an unsupported value (e.g. `nvme`) fails at parse time.

**Update name:** `drives update --name` changes the name visible in `drives list`.

**Delete:** Deleting a drive pair with no tracked files succeeds.

**Planned replacement workflow:** Two full end-to-end replacement tests cover the complete `drives replace mark --role <role>` → `drives replace confirm --role <role>` → `drives replace assign --role <role> <new-path>` sequence:

- *Primary replacement:* After `confirm`, virtual path symlinks are retargeted from the failed primary to the secondary. After `assign` and `sync process`, files are rebuilt onto the replacement drive and symlinks are retargeted again to the new primary path.
- *Secondary replacement:* After `assign` and `sync process`, files are rebuilt onto the replacement secondary. `drives show` confirms `Active Role: primary` and `Secondary State: active` — the active role is unchanged.

---

## cli_files.rs — File Tracking Commands

**File:** `tests/integration/cli_files.rs`

Covers the file tracking commands: adding files to tracking, mirroring them, inspecting them, and removing them.

**Track a file:** `files track` with a drive pair ID and a relative path creates a tracked file record and adds it to the sync queue. The output confirms the file was tracked.

**List tracked files:** `files list` shows tracked files with their IDs, paths, and mirror status.

**Mirror a file:** `files mirror` initiates an immediate mirror copy from the active drive to the standby. The test verifies the output confirms the operation and that the file appears in the secondary path.

**Absolute path input:** `files track` accepts an absolute path that resolves under the drive pair's active root and converts it to the stored relative path. An absolute path outside the active root is rejected.

**Remove a file:** `files untrack` untracks a file. It no longer appears in `files list`.

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

**Set a virtual path:** `virtual-paths set` associates a virtual path with a tracked file and creates a symlink on disk at that path pointing to the actual file. The stored path is persisted to the database.

**Remove a virtual path:** `virtual-paths remove` clears the virtual path association from the database and removes the symlink from disk. Calling `remove` on a file that has no virtual path set returns an error.

**List virtual paths:** `virtual-paths list` prints all tracked files that have a virtual path set.

**Refresh symlinks:** `virtual-paths refresh` scans all tracked files with a stored virtual path and recreates any symlinks that are missing on disk. This handles cases where symlinks were deleted outside the application.

---

## cli_integrity.rs — Integrity Check Commands

**File:** `tests/integration/cli_integrity.rs`

Covers the per-file and bulk integrity check commands.

**Check a specific file:** `integrity check <file_id>` performs an immediate synchronous check on one file. When the primary and mirror match, the output contains `OK` and the command exits successfully. When the mirror is absent, the output contains `MIRROR_MISSING` and the command exits with a failure code.

**Check all files:** `integrity check-all` checks every tracked file and reports the total count (e.g. `2 checked`). Clean files do not generate `integrity_pass` log events.

**Simulated corruption:** Before calling `integrity check`, tests write different bytes to the secondary copy to trigger the `mirror_corrupted` condition.

**Auto-recovery:** `integrity check --recover <file_id>` re-mirrors the file from the primary when corruption is detected. The output contains `Recovery: successful`, the mirror is restored to match the primary, any pending mirror queue item is reconciled to `completed` status, and both `recovery_success` and `sync_completed` log entries are written for the file.

---

## cli_sync.rs — Sync Queue Commands

**File:** `tests/integration/cli_sync.rs`

Covers the sync queue management commands.

**List queue:** `sync list` shows pending items with their file IDs, actions, and statuses.

**Process queue:** `sync process` processes all pending items in the queue, mirroring files to the standby. The output reports the number of items processed.

**Add to queue:** `sync add <file_id> <action>` enqueues a file with a specified action (e.g. `verify`). The item appears in `sync list`.

**Run a named task:** `sync run sync` triggers a full sync pass and mirrors pending files. `sync run integrity-check` runs a full integrity sweep and persists the run results, including per-file issue rows for files with `mirror_missing` status.

**Manual conflict resolution:** The `resolve_queue_item` helper is tested directly for all three resolution strategies: `keep_master` (restores the mirror from the primary), `keep_mirror` (restores the primary from the mirror), and `provide_new` (copies a supplied file to both sides). Passing a non-existent path with `provide_new` returns an error. Passing an unknown resolution string returns an error. Calling resolve on an item whose action is not `user_action_required` returns an error.

**Process skips manual items:** `process_all_pending` skips `user_action_required` items, returning zero processed, and leaves those items pending.

---

## cli_logs.rs — Event Log Commands

**File:** `tests/integration/cli_logs.rs`

Covers reading the event log from the CLI.

**List entries:** `logs list` returns log entries. Supported filters include `--event-type <type>`, `--file-id <id>`, `--from <ISO-8601>`, and `--to <ISO-8601>`. Pagination is controlled by `--page` and `--per-page`.

**Show a single entry:** `logs show <id>` prints one log entry by ID.

**Filter by event type:** `logs list --event-type integrity_pass` returns only entries with that type, confirmed against a log containing multiple event types.

**Automatic log creation:** Tracking a file via `tracker::track_file` automatically creates a `file_created` log entry containing the file name. Processing a sync item automatically creates a `sync_completed` entry. These side-effects are verified in dedicated tests.

**File ID filter:** `logs list --file-id <id>` returns only entries associated with the specified tracked file, confirmed after a sync operation that logs a `sync_completed` event against a known file ID.

---

## cli_database.rs — Database Backup Commands

**File:** `tests/integration/cli_database.rs`

Covers the database backup management commands.

**Add a backup destination:** `database add` registers a backup path with an optional drive label. The destination appears in `database list` with `priority: 0` and `enabled: true`.

**List and show destinations:** `database list` shows all configured backup destinations. `database show <id>` shows one destination by ID.

**Remove a destination:** `database remove` deletes the configuration. The destination no longer appears in `database list`.

**Run a backup:** `database run` copies the current database file to all enabled destinations as the canonical file `bitprotector.db`. Repeated runs overwrite the single canonical file rather than accumulating copies.

**Check integrity:** `database check-integrity` verifies the integrity of each destination's backup. When one copy is corrupt and another is valid, the corrupt copy is repaired from the healthy peer, confirmed by SQLite's `PRAGMA integrity_check` returning `ok`.

**Stage a restore:** `database restore <path>` stages a backup file for restore on next service restart. Passing a corrupt file is rejected before staging occurs.

**Update settings:** `database settings` updates the automatic backup and integrity check configuration (enabled flags and interval seconds). The changes are persisted and verified by reading them back.

---

## cli_status.rs — SSH Status Display

**File:** `tests/integration/cli_status.rs`

Covers the SSH status display subcommand, which is used by the `bitprotector-status.sh` script to display system health in SSH login banners.

**Basic output:** The `status` subcommand produces output headed with `BitProtector Status` and containing lines for `Drives: N`, `Files: N`, `Sync queue empty` (when nothing is pending), `No integrity failures`, and `No backups configured` (when none are set up). Each of these strings is asserted individually.

**With a drive pair:** After adding a real drive pair, the output shows `Drives: 1`.

**No drives registered:** On a fresh database the output shows `Drives: 0` and `Files: 0` rather than erroring.

---

## cli_auth.rs — JWT Middleware and Token Lifecycle

**File:** `tests/integration/cli_auth.rs`

This file uses the in-process actix-web test harness rather than spawning the binary, because it is testing the HTTP authentication middleware rather than CLI commands.

**Valid token accepted:** A request to a protected route with a correctly signed JWT in the `Authorization: Bearer` header is accepted with a `200` response.

**No token rejected:** A request to a protected route with no `Authorization` header is rejected with `401`.

**Malformed token rejected:** A request with a token that has been tampered with (altered signature) is rejected with `401`.

**Expired token rejected:** A token constructed with a negative TTL (already past its expiry at creation time) is rejected with `401`.

**Full lifecycle:** The `test_full_token_lifecycle` test calls `issue_token` and `validate_token` from the library directly, confirming that the `sub` claim matches the input, that `exp > iat`, that a token verified with the wrong secret fails, and that an expired token fails validation.
