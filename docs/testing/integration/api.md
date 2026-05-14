# REST API Integration Tests

These tests use the in-process `actix_web::test` harness to send HTTP requests to a fully configured application instance and assert on response status codes, JSON body shapes, and database side-effects. Each test creates a fresh in-memory repository so there is no shared state between tests.

For an explanation of the harness and isolation strategy, see [README.md](README.md).

---

## Table of Contents

- [api_drives.rs — Drive Pair Endpoints](#api_drivesrs--drive-pair-endpoints)
- [api_files.rs — File Tracking Endpoints](#api_filesrs--file-tracking-endpoints)
- [api_folders.rs — Folder Tracking Endpoints](#api_foldersrs--folder-tracking-endpoints)
- [api_virtual_paths.rs — Virtual Path Endpoints](#api_virtual_pathsrs--virtual-path-endpoints)
- [api_integrity.rs — Integrity Run Endpoints](#api_integrityrs--integrity-run-endpoints)
- [api_scheduler.rs — Scheduler Endpoints](#api_schedulerrs--scheduler-endpoints)
- [api_sync.rs — Sync Queue Endpoints](#api_syncrs--sync-queue-endpoints)
- [api_logs.rs — Event Log Endpoints](#api_logsrs--event-log-endpoints)
- [api_database.rs — Database Backup Endpoints](#api_databasers--database-backup-endpoints)
- [api_routes.rs — Cross-Route and Status Coverage](#api_routesrs--cross-route-and-status-coverage)
- [api_filesystem_browser.rs — Filesystem Browser Endpoint](#api_filesystem_browserrs--filesystem-browser-endpoint)

---

## api_drives.rs — Drive Pair Endpoints

**File:** `tests/integration/api_drives.rs`

**`GET /api/v1/drives`** — Returns an empty array when no drive pairs exist. Returns the full list when pairs have been created, with each pair's ID, name, paths, media types, states, and active role.

**`POST /api/v1/drives`** — Creating a drive pair with `skip_validation: true` bypasses the on-disk path check and returns `201 Created` with the full pair object including default media types (`hdd`). Creating without `skip_validation` when the paths do not exist returns `400 Bad Request`.

**`PUT /api/v1/drives/{id}`** — Updates name, paths, or media types. The response contains the updated pair. Updating a non-existent ID returns `404`.

**`DELETE /api/v1/drives/{id}`** — Deletes an empty drive pair and returns `204 No Content`. Deleting a pair that has tracked files returns `409 Conflict` to prevent accidental data loss.

**Drive state endpoints** — `POST /api/v1/drives/{id}/mark`, `/cancel`, `/confirm`, and `/assign` are covered by inline unit tests in `src/api/server.rs`. The integration file covers the `mark → cancel` path to verify the replacement workflow state machine through the HTTP layer. Sending an invalid replacement role returns `400 Bad Request`.

**Media type update** — `PUT /api/v1/drives/{id}` with `primary_media_type: "ssd"` persists the change, and a subsequent `GET` returns the updated type. This guards against media type being silently discarded.

---

## api_files.rs — File Tracking Endpoints

**File:** `tests/integration/api_files.rs`

This file is the most extensive API test suite because the file tracking semantics have several non-obvious behaviors.

**Queue-first tracking:** `POST /api/v1/files` adds a file to tracking and enqueues it for mirroring, but does not immediately copy the file to the secondary. The response contains `is_mirrored: false` even when the physical file exists on the primary. A subsequent `GET` of the sync queue shows a pending item for the file. This is the intentional design: the user controls when mirroring happens.

**Immediate mirror:** `POST /api/v1/files/{id}/mirror` triggers an immediate mirror copy outside the sync queue. The response confirms the operation and the file's `is_mirrored` field becomes `true`.

**Absolute path input:** The create endpoint accepts an absolute path as `relative_path` when it resolves under the drive pair's active root. The stored value is the derived relative path, not the absolute one. An absolute path that does not resolve under the active root is rejected with `400`.

**Virtual path derivation for folder-origin files:** A file that was added by a folder scan (not tracked directly) has an effective virtual path derived from the folder's virtual root plus the file's sub-path within the folder. The API returns this effective path in `virtual_path` even though the file record itself has no explicit `virtual_path` set.

**Source filtering:** `GET /api/v1/tracking/items?source=direct` returns only directly tracked files. `source=folder` returns only folder-origin files. `source=both` is rejected with `400 Bad Request` — the valid values are `direct`, `folder`, and `all`.

**Virtual-prefix filtering:** `GET /api/v1/tracking/items?virtual_prefix=/docs` returns only items whose effective virtual path starts with `/docs`. This is tested against a mix of files with and without virtual paths to confirm the filter correctly uses effective rather than stored paths.

**`has_virtual_path` filtering:** `GET /api/v1/tracking/items?has_virtual_path=true` returns only items that have an effective virtual path set (either direct or inherited from a folder).

**Pagination:** The tracking list endpoint enforces a maximum page size of 200. Requesting `per_page=500` returns at most 200 results.

**`DELETE /api/v1/files/{id}`** — Removes the file from tracking. The file remains on disk; only the tracking record is deleted.

---

## api_folders.rs — Folder Tracking Endpoints

**File:** `tests/integration/api_folders.rs`

**Queue-first folder scan:** `POST /api/v1/folders/{id}/scan` discovers files in the folder, creates tracked file records, and enqueues them for mirroring — but does not immediately copy any files. The response body contains `new_files` and `changed_files` counts. Sync queue inspection confirms items are pending.

**Folder aggregate status:** `GET /api/v1/tracking/items` returns folder entries with aggregate status fields: `folder_status` (`not_scanned`, `empty`, `tracked`, `mirrored`, `partial`), `folder_total_files`, and `folder_mirrored_files`. These fields are verified for each status variant:

- A freshly added folder before any scan: `not_scanned`.
- A scanned folder where the directory is empty: `empty`.
- A scanned folder with files that have not been mirrored: `tracked`.
- A scanned folder where all files are mirrored: `mirrored`.
- A scanned folder where some but not all files are mirrored: `partial`.

**Immediate mirror:** `POST /api/v1/folders/{id}/mirror` mirrors all tracked files in the folder immediately, bypassing the queue. The response reports how many files were mirrored.

**Change detection:** After a folder has been scanned and files have been modified on disk, a second scan detects the changed files and re-enqueues only those files. Unchanged files are not re-queued.

**Virtual path on folder:** Creating a folder with a `virtual_path` causes files discovered in that folder to report effective virtual paths derived from the folder's virtual root. This is verified by scanning the folder and then inspecting the returned tracking items.

**`DELETE /api/v1/folders/{id}`** — Removes the folder tracking record. Files previously discovered through the folder remain tracked individually as direct files.

---

## api_virtual_paths.rs — Virtual Path Endpoints

**File:** `tests/integration/api_virtual_paths.rs`

**`PUT /api/v1/files/{id}/virtual-path`** — Sets a virtual path on a directly tracked file. The updated file's `virtual_path` field is returned in the response and persisted.

**`DELETE /api/v1/files/{id}/virtual-path`** — Clears the virtual path. The field becomes `null` in subsequent reads.

**Prefix filtering after set:** After setting a virtual path of `/docs/report.pdf`, a `GET /api/v1/tracking/items?virtual_prefix=/docs` includes the file, while `virtual_prefix=/other` does not. This confirms the filter uses the stored virtual path correctly.

**Folder virtual path update:** `PUT /api/v1/folders/{id}/virtual-path` sets or updates the virtual root for a folder. Files already discovered through the folder immediately report updated effective virtual paths in tracking queries, because effective paths are derived on read rather than stored per file.

**Invalid virtual path format:** A `virtual_path` that does not start with `/` is rejected with `400 Bad Request`.

---

## api_integrity.rs — Integrity Run Endpoints

**File:** `tests/integration/api_integrity.rs`

This file covers the asynchronous integrity run lifecycle, which is more complex than synchronous endpoints because an integrity run happens in the background.

**Start a run:** `POST /api/v1/integrity/runs` initiates a run and returns `202 Accepted` with the run object including its ID, status (`running`), and the set of drive pairs being checked.

**Single active run conflict:** Calling `POST /api/v1/integrity/runs` while a run is already in progress returns `409 Conflict`. Only one run may be active at a time.

**Progress polling:** `GET /api/v1/integrity/runs/active` returns the current run's progress: total files, processed files, files with attention needed, and elapsed time. When no run is active, it returns `{ "run": null }`.

**Cooperative stop:** `POST /api/v1/integrity/runs/{id}/stop` signals the run to stop. The run finishes its current file and then halts. The response confirms the stop was requested, and a subsequent poll of the active run shows no active run.

**Latest run results:** `GET /api/v1/integrity/runs/latest` returns the results of the most recently completed run, including the list of files checked with their status. Results are paginated.

**Per-run results:** `GET /api/v1/integrity/runs/{id}/results` returns results for a specific historical run.

**Issues-only filter:** `GET /api/v1/integrity/runs/latest?issues_only=true` returns only the results where `needs_attention` is `true`. Files that passed are omitted. This is verified against a run with a mix of passing and failing files.

---

## api_scheduler.rs — Scheduler Endpoints

**File:** `tests/integration/api_scheduler.rs`

**`GET /api/v1/scheduler/schedules`** — Returns the list of configured schedules. Empty on a fresh database.

**`POST /api/v1/scheduler/schedules`** — Creates a schedule with either an `interval_seconds` value or a `cron_expression`. The response includes the assigned ID, task type, timing, and enabled state.

**`PUT /api/v1/scheduler/schedules/{id}`** — Updates timing or enabled state. Returns the updated schedule.

**`DELETE /api/v1/scheduler/schedules/{id}`** — Removes the schedule. It no longer appears in the list.

**Enable/disable:** A schedule with `enabled: false` is listed but the scheduler does not trigger it. Updating `enabled: true` re-enables triggering. The test verifies both states are persisted correctly.

**Cron expression validation:** A schedule created with an invalid cron expression is rejected with `400 Bad Request`.

---

## api_sync.rs — Sync Queue Endpoints

**File:** `tests/integration/api_sync.rs`

**`GET /api/v1/sync/queue`** — Returns pending items with their file ID, action, and status. Includes the `queue_paused` boolean.

**`POST /api/v1/sync/process`** — Processes all pending items. The response reports how many items were completed. After processing, the queue is empty.

**Pause and resume:** `POST /api/v1/sync/pause` sets `queue_paused: true`. A subsequent process call returns immediately with zero items processed. `POST /api/v1/sync/resume` re-enables processing.

**Manual action resolution:** Items in `user_action_required` status are not automatically processed. `POST /api/v1/sync/queue/{id}/resolve` with `{ "resolution": "keep_master" }` or `"keep_mirror"` marks the item resolved and returns the updated item.

**`DELETE /api/v1/sync/queue/completed`** — Bulk-removes all completed items from the queue. The response body contains a `deleted` count. Items with other statuses (pending, failed, etc.) are not affected.

---

## api_logs.rs — Event Log Endpoints

**File:** `tests/integration/api_logs.rs`

**`GET /api/v1/logs`** — Returns log entries in reverse chronological order inside a `logs` array. Each entry has an `event_type`, ID, optional file ID, and timestamp. The `event_type` discriminates entries (e.g. `sync_completed`, `file_created`, `recovery_success`).

**`event_type` filter:** `GET /api/v1/logs?event_type=sync_completed` returns only entries with the given event type. Entries with other types are excluded.

**Date range filter:** `GET /api/v1/logs?from=<ISO-8601>` returns only entries created on or after the given timestamp. Passing a future date returns an empty array.

**Single entry lookup:** `GET /api/v1/logs/{id}` returns one entry by ID. An unknown ID returns `404 Not Found`.

**Pagination:** The `page` and `per_page` parameters control which slice of entries is returned. The response includes `total` so callers can compute the number of pages.

**Empty log:** On a fresh database, the endpoint returns an empty array and `total: 0` rather than an error.

---

## api_database.rs — Database Backup Endpoints

**File:** `tests/integration/api_database.rs`

**`GET /api/v1/database/backups`** — Lists configured backup destinations.

**`POST /api/v1/database/backups`** — Creates a backup destination with a `backup_path` and `drive_label`. The response includes the assigned ID and `enabled: true`.

**`PUT /api/v1/database/backups/{id}`** — Updates path, label, or enabled state.

**`DELETE /api/v1/database/backups/{id}`** — Removes the destination configuration.

**`POST /api/v1/database/backups/run`** — Runs an immediate backup to all enabled destinations. The response is an array of per-destination results, each with the path written and success status.

**`POST /api/v1/database/backups/integrity-check`** — Verifies the integrity of the most recent backup at each destination. Each destination result carries a `status` of either `repaired` (corrupt backup replaced from a healthy peer) or `corrupt` (no healthy peer available). The response is an array of per-destination results.

**`POST /api/v1/database/backups/restore`** — Stages a specific backup file for restore on next service restart. The response includes `restart_required: true` and a `safety_backup_path` field giving the path of the safety copy made of the live database before staging. Passing a corrupt file is rejected before staging occurs.

**`GET /api/v1/database/backups/settings`** — Returns the automatic backup settings (interval, enabled, integrity check interval).

**`PUT /api/v1/database/backups/settings`** — Updates automatic backup settings. The updated values are returned and persist across reads.

---

## api_routes.rs — Cross-Route and Status Coverage

**File:** `tests/integration/api_routes.rs`

This file catches coverage that does not fit neatly into any single feature area.

**`GET /api/v1/status`** — Returns a system health summary. The shape of the response is verified: `files_tracked`, `drive_pairs`, `degraded_pairs`, `active_secondary_pairs`, `rebuilding_pairs`, and `quiescing_pairs`. The value of `degraded_pairs` increases when a pair is placed in a degraded state, confirming the field reflects live data.

---

## api_filesystem_browser.rs — Filesystem Browser Endpoint

**File:** `tests/integration/api_filesystem_browser.rs`

This file covers the read-only endpoint that powers the path picker dialog in the web UI. The endpoint allows the user to browse the server's filesystem to select drive roots and backup destinations.

**Default root browsing:** `GET /api/v1/filesystem/children` without parameters returns the top-level directory entries visible to the service user. The response has a `path` field and an `entries` array. Each entry has `name` and `kind` fields; hidden entries additionally carry `is_hidden: true` and selectable entries carry `is_selectable: true`.

**Nested directory loading:** `GET /api/v1/filesystem/children?path=/some/directory` returns the children of the specified directory. This is how the path picker lazily loads subdirectories.

**Hidden-file toggle:** `GET /api/v1/filesystem/children?include_hidden=true` includes entries whose names start with `.`. The default (without the parameter) excludes them. The test verifies both behaviors against a directory containing hidden entries.

**Invalid path handling:** Requesting a path that does not exist returns `400 Bad Request` with an error message rather than a `500` or an empty array.

**Unreadable path handling:** Requesting a path for which the service user has no read permission returns an error response rather than crashing.

**Directory-only filtering:** `GET /api/v1/filesystem/children?directories_only=true` returns only entries where `kind` is `directory`. File entries are excluded. This is used by the drive path picker, which must select a directory rather than a file.
