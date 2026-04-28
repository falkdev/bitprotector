# REST API Reference

**Base URL:** `https://<host>:<port>/api/v1`  
**Transport:** HTTPS only (TLS required)  
**Authentication:** JWT bearer token (all endpoints except `POST /auth/login` and `GET /health`)

---

## Table of Contents

- [Authentication](#authentication)
- [Health](#health)
- [Drive Pairs](#drive-pairs)
- [Filesystem Browser](#filesystem-browser)
- [Tracking Workspace](#tracking-workspace)
- [File Tracking](#file-tracking)
- [Virtual Paths](#virtual-paths)
- [Tracked Folders](#tracked-folders)
- [Integrity Checks](#integrity-checks)
- [Sync Queue](#sync-queue)
- [Scheduler](#scheduler)
- [Event Logs](#event-logs)
- [Database Backups](#database-backups)
- [System Status](#system-status)
- [Error Responses](#error-responses)

---

## Authentication

The API uses [PAM](https://en.wikipedia.org/wiki/Linux_PAM) to verify credentials against the host's system accounts, then issues a short-lived JWT. Include the token in every subsequent request:

```http
Authorization: Bearer <token>
```

### POST `/auth/login`

Authenticate with a local system account.

**Request body:**

```json
{
    "username": "alice",
    "password": "secret"
}
```

**Response `200`:**

```json
{
    "token": "<jwt>",
    "username": "alice",
    "expires_at": "2026-01-02T00:00:00Z"
}
```

`expires_at` is an RFC3339 UTC timestamp. Token lifetime is fixed at 24 hours.

**Errors:** `401 Unauthorized`

---

### GET `/auth/validate`

Check whether the current token is valid. Requires a valid JWT.

**Response `200`:**

```json
{
    "username": "alice",
    "valid": true
}
```

### POST `/auth/logout`

Invalidate the current JWT immediately. After this call the token is rejected by the server even if it has not yet expired.

**Response `200`:**

```json
{ "message": "Logged out" }
```

**Errors:** `401 Unauthorized` (token already invalid)

---

## Health

### GET `/health`

Liveness probe. Does not require authentication. Returns `200` whenever the process is running and accepting connections.

**Response `200`:**

```json
{ "status": "ok" }
```

This endpoint is intended for monitoring systems, load balancers, and systemd watchdogs.

---

## Drive Pairs

A drive pair binds a **primary** directory to a **secondary** (mirror) directory. All file tracking and mirroring is scoped to a drive pair. Each pair also exposes runtime state so clients can tell whether the system is active, quiescing for replacement, failed over, or rebuilding.

### GET `/drives`

List all configured drive pairs.

**Response `200`:**

```json
[
    {
        "id": 1,
        "name": "mybackup",
        "primary_path": "/mnt/primary",
        "secondary_path": "/mnt/mirror",
        "primary_state": "active",
        "secondary_state": "active",
        "active_role": "primary",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    }
]
```

`primary_state` and `secondary_state` are one of `active`, `quiescing`, `failed`, or `rebuilding`. `active_role` is either `primary` or `secondary`.

---

### POST `/drives`

Create a new drive pair.

**Request body:**

```json
{
    "name": "mybackup",
    "primary_path": "/mnt/primary",
    "secondary_path": "/mnt/mirror"
}
```

**Response `201`:** Created drive pair object.  
**Errors:** `400 Bad Request`, `409 Conflict` (name already in use)

---

### GET `/drives/{id}`

Get a specific drive pair.

**Response `200`:** Single drive pair object.  
**Errors:** `404 Not Found`

---

### PUT `/drives/{id}`

Update a drive pair. All fields are optional.

**Request body:**

```json
{
    "name": "renamed",
    "primary_path": "/new/primary",
    "secondary_path": "/new/mirror"
}
```

**Response `200`:** Updated drive pair object.  
**Errors:** `404 Not Found`, `400 Bad Request`

---

### DELETE `/drives/{id}`

Remove a drive pair. Fails if any tracked files still reference it.

**Response `204`:** No content.  
**Errors:** `404 Not Found`, `409 Conflict`

---

### POST `/drives/{id}/replacement/mark`

Start a planned replacement workflow by moving one slot into `quiescing`.

**Request body:**

```json
{
    "role": "primary"
}
```

`role` is `primary` or `secondary`.

**Response `200`:** Updated drive pair object.  
**Errors:** `400 Bad Request`, `404 Not Found`

---

### POST `/drives/{id}/replacement/cancel`

Cancel a planned replacement and return a `quiescing` slot to `active`.

**Request body:**

```json
{
    "role": "primary"
}
```

**Response `200`:** Updated drive pair object.  
**Errors:** `400 Bad Request`, `404 Not Found`

---

### POST `/drives/{id}/replacement/confirm`

Confirm that the quiesced drive is now failed. If the failed slot was active, BitProtector switches `active_role` to the surviving side and refreshes virtual paths.

**Request body:**

```json
{
    "role": "primary"
}
```

**Response `200`:** Updated drive pair object.  
**Errors:** `400 Bad Request`, `404 Not Found`

---

### POST `/drives/{id}/replacement/assign`

Assign a new mounted path to a failed slot and queue rebuild work.

**Request body:**

```json
{
    "role": "primary",
    "new_path": "/mnt/replacement-primary",
    "skip_validation": false
}
```

**Response `200`:**

```json
{
    "drive_pair": {
        "id": 1,
        "name": "mybackup",
        "primary_path": "/mnt/replacement-primary",
        "secondary_path": "/mnt/mirror",
        "primary_state": "rebuilding",
        "secondary_state": "active",
        "active_role": "secondary",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:05:00Z"
    },
    "queued_rebuild_items": 42
}
```

BitProtector queues rebuild work but does not process it automatically. Run the sync processor or let the scheduler handle it later.

---

### POST `/drives/{id}/failover`

Trigger an emergency failover for a drive pair immediately. If the currently active drive root is unavailable and the standby slot is healthy and `active`, the server switches `active_role` to the standby and retargets all virtual-path symlinks.

If no failover is needed (both sides are reachable) the request succeeds with `"failover_performed": false` and the pair is returned unchanged.

**Response `200`:**

```json
{
    "drive_pair": { ... },
    "failover_performed": true
}
```

**Errors:** `404 Not Found`, `500 Internal Server Error`

---

## Filesystem Browser

The web UI uses a read-only server-side filesystem browser to populate path picker dialogs. This endpoint browses the BitProtector host filesystem, not the end user's local machine.

### GET `/filesystem/children`

List one directory level of children for a host path.

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `path` | string | Absolute directory path to browse. Defaults to `/`. |
| `include_hidden` | boolean | Include dotfiles and dot-directories when `true`. Defaults to `false`. |
| `directories_only` | boolean | Return only directories when `true`. Defaults to `false`. |

**Response `200`:**

```json
{
    "path": "/mnt",
    "canonical_path": "/mnt",
    "parent_path": "/",
    "entries": [
        {
            "name": "primary",
            "path": "/mnt/primary",
            "kind": "directory",
            "is_hidden": false,
            "is_selectable": true,
            "has_children": true
        },
        {
            "name": "report.pdf",
            "path": "/mnt/report.pdf",
            "kind": "file",
            "is_hidden": false,
            "is_selectable": true,
            "has_children": false
        }
    ]
}
```

**Errors:** `400 Bad Request` (invalid path, unreadable path, or path is not a directory), `401 Unauthorized`

---

## Tracking Workspace

The web UI unified tracking page reads from a mixed file+folder listing endpoint designed for server-side filtering and pagination.

### GET `/tracking/items`

List tracking items across files and folders in one response.

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `drive_id` | integer | Filter by drive pair ID |
| `q` | string | Case-insensitive contains match against tracked path and virtual path |
| `virtual_prefix` | string | Filter to virtual paths under this absolute prefix |
| `has_virtual_path` | boolean | `true` = has virtual path, `false` = no virtual path |
| `item_kind` | string | `file`, `folder`, or `all` (default) |
| `source` | string | `direct`, `folder`, or `all` (default) |
| `page` | integer | Page number (default: 1) |
| `per_page` | integer | Items per page (default: 50, max: 200) |

For file rows, `virtual_prefix` and `has_virtual_path` are evaluated against the effective file virtual path:

- explicit file `virtual_path`, otherwise
- a folder-derived virtual path from the closest matching tracked folder virtual path.

`source` filtering applies to file rows by provenance flags. Folder rows are included when `item_kind=folder`, or when `item_kind=all` with `source=all|folder`. `source=both` is invalid and returns `400`.

**Response `200`:**

```json
{
    "items": [
        {
            "kind": "file",
            "id": 1,
            "drive_pair_id": 1,
            "path": "docs/report.pdf",
            "virtual_path": "/virtual/docs/report.pdf",
            "is_mirrored": false,
            "tracked_direct": true,
            "tracked_via_folder": false,
            "source": "direct",
            "folder_status": null,
            "folder_total_files": null,
            "folder_mirrored_files": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        },
        {
            "kind": "folder",
            "id": 2,
            "drive_pair_id": 1,
            "path": "docs",
            "virtual_path": "/virtual/docs",
            "is_mirrored": null,
            "tracked_direct": null,
            "tracked_via_folder": null,
            "source": "folder",
            "folder_status": "partial",
            "folder_total_files": 3,
            "folder_mirrored_files": 1,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }
    ],
    "total": 2,
    "page": 1,
    "per_page": 50
}
```

Folder rows include aggregate status fields:

- `folder_status`: `not_scanned` | `empty` | `tracked` | `mirrored` | `partial`
- `folder_total_files`: tracked files currently under that folder path
- `folder_mirrored_files`: mirrored tracked files currently under that folder path

`not_scanned` means the folder has zero tracked files and has not completed a scan yet. `empty` means the folder has zero tracked files after at least one successful scan.

**Errors:** `400 Bad Request` (invalid `source` or `item_kind`)

---

## File Tracking

### GET `/files`

List tracked files with optional filtering and pagination.

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `drive_id` | integer | Filter by drive pair |
| `virtual_prefix` | string | Filter by virtual path prefix |
| `mirrored` | boolean | Filter by mirror status |
| `page` | integer | Page number (default: 1) |
| `per_page` | integer | Items per page (default: 50) |

**Response `200`:**

```json
{
    "files": [
        {
            "id": 1,
            "drive_pair_id": 1,
            "relative_path": "documents/report.pdf",
            "checksum": "d74981efa70a0c880b...",
            "file_size": 1048576,
            "virtual_path": "/docs/report.pdf",
            "is_mirrored": true,
            "tracked_direct": true,
            "tracked_via_folder": false,
            "last_integrity_check_at": "2026-03-14T12:00:00Z",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-03-14T12:00:00Z"
        }
    ],
    "total": 100,
    "page": 1,
    "per_page": 50
}
```

---

### POST `/files`

Track a file by path. BitProtector computes its BLAKE3 checksum and records provenance/source flags.

**Request body:**

```json
{
    "drive_pair_id": 1,
    "relative_path": "documents/report.pdf",
    "virtual_path": "/docs/report.pdf"
}
```

`virtual_path` is optional.

Newly tracked files enqueue deduplicated `mirror` work in `sync_queue` when the standby slot can accept sync. Tracking does not mirror immediately by default.

`mirror` is still accepted as an optional legacy compatibility field, but is ignored for immediate copy semantics. Use [`POST /files/{id}/mirror`](#post-filesidmirror) for immediate mirroring, or process queue work through [`POST /sync/process`](#post-syncprocess).

`relative_path` remains the stored path relative to the selected drive pair's current active root. The web UI path picker may submit an absolute host path, but the server validates that it resolves under the active root and converts it back to a relative path before storing it.

Inputs that contain parent-directory traversal (`..`) or canonicalize outside the selected active root are rejected with `400 Bad Request`.

If the file path is already tracked in the same drive pair, this endpoint is idempotent: it updates the existing row to `tracked_direct=true` and returns the existing file with `200 OK` (no duplicate row is created).

**Response `201`:** Newly created file object.  
**Response `200`:** Existing tracked file promoted to direct tracking.  
**Errors:** `400 Bad Request`, `404 Not Found` (drive pair not found)

---

### GET `/files/{id}`

Get a tracked file.

**Response `200`:** Single file object.  
**Errors:** `404 Not Found`

---

### DELETE `/files/{id}`

Stop tracking a file. Does **not** delete the file from disk.

**Response `204`:** No content.  
**Errors:** `404 Not Found`

---

### POST `/files/{id}/mirror`

Trigger an immediate mirror of the file to the secondary path. Requires the standby slot to be available.

**Response `200`:**

```json
{ "mirrored": true }
```

**Errors:** `404 Not Found`, `500 Internal Server Error` (mirror failed)

After a successful immediate mirror, pending/in-progress `mirror` queue rows for that file are marked `completed`.

> **Note:** To verify file integrity, use [`POST /integrity/check/{id}`](#post-integritycheckid).

---

## Virtual Paths

Virtual paths expose tracked files through exact absolute filesystem paths. Setting a virtual path creates a symlink directly at that path.

### PUT `/virtual-paths/{file_id}`

Set or update the virtual path for a tracked file.

**Request body:**

```json
{
    "virtual_path": "/docs/report.pdf"
}
```

`virtual_path` must be an absolute path. BitProtector creates parent directories as needed, refuses using `/` as a virtual path, and will not overwrite non-BitProtector filesystem entries.

**Response `200`:** Plain text confirmation.  
**Errors:** `404 Not Found`, `500 Internal Server Error`

---

### DELETE `/virtual-paths/{file_id}`

Remove the virtual path mapping (and its symlink) from a file.

**Response `200`:** Plain text confirmation.  
**Errors:** `404 Not Found`, `400 Bad Request` (file has no virtual path)

---

### GET `/virtual-paths/tree`

Return one lazy tree level of virtual-path children under an absolute parent prefix.

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `parent` | string | Absolute parent prefix (default: `/`) |

**Response `200`:**

```json
{
    "parent": "/virtual",
    "children": [
        {
            "name": "docs",
            "path": "/virtual/docs",
            "item_count": 2,
            "has_children": true
        },
        {
            "name": "media",
            "path": "/virtual/media",
            "item_count": 1,
            "has_children": true
        }
    ]
}
```

`item_count` is the number of file/folder virtual-path entries under that immediate child segment, including file virtual paths derived from tracked folder virtual paths.

**Errors:** `400 Bad Request` (non-absolute `parent` or contains `..`)

---

## Tracked Folders

Tracked folders let BitProtector automatically discover and track new files added to a directory.

### GET `/folders`

**Response `200`:** Flat JSON array of folder objects:

```json
[
    {
        "id": 1,
        "drive_pair_id": 1,
        "folder_path": "documents/",
        "virtual_path": "/docs",
        "last_scanned_at": null,
        "created_at": "2026-01-01T00:00:00Z"
    }
]
```

---

### POST `/folders`

**Request body:**

```json
{
    "drive_pair_id": 1,
    "folder_path": "documents/",
    "virtual_path": "/docs"
}
```

`virtual_path` is optional. When present, BitProtector creates a directory symlink exactly at that absolute path.

`folder_path` remains the stored path relative to the selected drive pair's current active root. The web UI path picker may submit an absolute host path, but the server validates that it resolves under the active root and converts it back to a relative path before storing it.

Inputs that contain parent-directory traversal (`..`) or canonicalize outside the selected active root are rejected with `400 Bad Request`.

**Response `201`:** Created folder object.  
**Errors:** `400 Bad Request`, `409 Conflict`

---

### GET `/folders/{id}`

**Response `200`:** Single folder object.  
**Errors:** `404 Not Found`

---

### PUT `/folders/{id}`

Update a tracked folder's configuration. All fields are optional.

**Request body:**

```json
{
    "virtual_path": "/virtual/documents"
}
```

To clear `virtual_path`, pass it explicitly as `null`. Omitting the field leaves the current value unchanged.

**Response `200`:** Updated folder object.  
**Errors:** `404 Not Found`

---

### POST `/folders/{id}/scan`

Scan a tracked folder for new and changed files. Newly discovered files are tracked automatically. Changed files are detected by re-hashing.

Scan tracks files and enqueues mirror work when possible; it does not mirror immediately.
On success, the folder's `last_scanned_at` is updated.

**Response `200`:**

```json
{
    "new_files": 3,
    "changed_files": 1
}
```

**Errors:** `404 Not Found`, `500 Internal Server Error`

---

### POST `/folders/{id}/mirror`

Immediately mirror all unmirrored tracked files under a tracked folder.

**Response `200`:**

```json
{
    "mirrored_files": 2
}
```

`mirrored_files` counts files mirrored during this request.

After a successful mirror, pending/in-progress `mirror` queue rows for mirrored files are marked `completed`.

**Errors:** `404 Not Found`, `400 Bad Request` (standby slot unavailable), `500 Internal Server Error`

---

### DELETE `/folders/{id}`

Stop tracking the folder. Already-tracked files are **not** removed.

**Response `204`:** No content.  
**Errors:** `404 Not Found`

---

## Integrity Checks

### POST `/integrity/check/{id}`

Run an integrity check for one tracked file. Add `?recover=true` to attempt automatic recovery where the healthy counterpart still exists.

**Response `200`:**

```json
{
    "file_id": 1,
    "status": "ok",
    "master_valid": true,
    "mirror_valid": true,
    "recovered": false
}
```

This endpoint updates `tracked_files.last_integrity_check_at` after each check attempt.

When `recover=true` and auto-recovery succeeds, the file is marked mirrored, pending/in-progress
`mirror` queue rows for that file are marked `completed`, and recovery activity is recorded in
event logs (`recovery_success`; plus `sync_completed` when queue rows were reconciled).

`status` is one of:

- `ok`
- `master_corrupted`
- `mirror_corrupted`
- `both_corrupted`
- `master_missing`
- `mirror_missing`
- `primary_drive_unavailable`
- `secondary_drive_unavailable`

---

### POST `/integrity/runs`

Start an asynchronous integrity run. Request body is optional.

**Request body (optional):**

```json
{
    "drive_id": 1,
    "recover": false
}
```

- `drive_id`: optional scope to one drive pair (omit for all pairs)
- `recover`: optional (default `false`)

When `recover=true`, each successfully recovered file is marked mirrored, pending/in-progress
`mirror` queue rows for that file are marked `completed`, and recovery actions are logged
(`recovery_success`; plus `sync_completed` when queue rows were reconciled).

**Response `202`:** run summary object.

```json
{
    "id": 101,
    "scope_drive_pair_id": null,
    "recover": false,
    "trigger": "api",
    "status": "running",
    "total_files": 100000,
    "processed_files": 0,
    "attention_files": 0,
    "recovered_files": 0,
    "stop_requested": false,
    "started_at": "2026-04-06 09:00:00",
    "ended_at": null,
    "error_message": null
}
```

**Errors:** `409 Conflict` (another run is active)

---

### GET `/integrity/runs/active`

Get the currently active run (status `running` or `stopping`).

**Response `200`:**

```json
{
    "run": {
        "id": 101,
        "scope_drive_pair_id": null,
        "recover": false,
        "trigger": "api",
        "status": "running",
        "total_files": 100000,
        "processed_files": 2500,
        "attention_files": 3,
        "recovered_files": 1,
        "stop_requested": false,
        "started_at": "2026-04-06 09:00:00",
        "ended_at": null,
        "error_message": null
    }
}
```

When no run is active:

```json
{
    "run": null
}
```

---

### POST `/integrity/runs/{id}/stop`

Request cooperative stop for a run by setting `stop_requested=true`.  
The run transitions to `stopping`, then finishes as `stopped` after the current file completes.

**Response `200`:** updated run summary object.  
**Errors:** `404 Not Found`

---

### GET `/integrity/runs/latest`

Get the latest persisted run summary plus paged results.

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `issues_only` | boolean | Defaults to `true` (return only rows with `needs_attention=true`) |
| `page` | integer | Page number (default: `1`) |
| `per_page` | integer | Page size (default: `50`, max: `200`) |

**Response `200`:**

```json
{
    "run": {
        "id": 101,
        "scope_drive_pair_id": null,
        "recover": false,
        "trigger": "api",
        "status": "completed",
        "total_files": 100000,
        "processed_files": 100000,
        "attention_files": 4,
        "recovered_files": 2,
        "stop_requested": false,
        "started_at": "2026-04-06 09:00:00",
        "ended_at": "2026-04-06 09:18:00",
        "error_message": null
    },
    "results": [
        {
            "id": 555,
            "run_id": 101,
            "file_id": 18,
            "drive_pair_id": 1,
            "relative_path": "docs/broken.txt",
            "status": "mirror_corrupted",
            "recovered": false,
            "needs_attention": true,
            "checked_at": "2026-04-06 09:04:23"
        }
    ],
    "total": 4,
    "page": 1,
    "per_page": 50
}
```

When no run exists yet:

```json
{
    "run": null,
    "results": [],
    "total": 0,
    "page": 1,
    "per_page": 50
}
```

`needs_attention` is precomputed as `status != ok AND recovered == false` for fast issue-only retrieval.

`status` in run results is one of:

- `ok`
- `master_corrupted`
- `mirror_corrupted`
- `both_corrupted`
- `master_missing`
- `mirror_missing`
- `primary_drive_unavailable`
- `secondary_drive_unavailable`
- `internal_error`

---

### GET `/integrity/runs/{id}/results`

Get paged results for a specific run.

**Query parameters:** same as [`GET /integrity/runs/latest`](#get-integrityrunslatest)

**Response `200`:** same shape as latest endpoint (`run`, `results`, `total`, `page`, `per_page`).  
**Errors:** `404 Not Found`

---

## Sync Queue

### GET `/sync/queue`

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `status` | string | `pending` \| `in_progress` \| `completed` \| `failed` |
| `page` | integer | Page number |
| `per_page` | integer | Items per page |

**Response `200`:**

```json
{
    "queue": [
        {
            "id": 1,
            "tracked_file_id": 5,
            "action": "mirror",
            "status": "pending",
            "error_message": null,
            "created_at": "2026-03-15T08:00:00Z",
            "completed_at": null
        }
    ],
    "total": 42,
    "page": 1,
    "per_page": 50,
    "queue_paused": false
}
```

`queue_paused` is `true` when automatic queue processing has been suspended via `POST /sync/pause`. The queue contents are unchanged; no new items will be processed until `POST /sync/resume` is called.

---

### POST `/sync/queue`

Manually enqueue a sync action for a tracked file.

**Request body:**

```json
{
    "tracked_file_id": 5,
    "action": "mirror"
}
```

`action` is one of: `mirror` | `restore_master` | `restore_mirror` | `verify` | `user_action_required`.

**Response `201`:** Created queue item.  
**Errors:** `400 Bad Request`

---

### GET `/sync/queue/{id}`

Get a single sync queue item.

**Response `200`:** Queue item object.  
**Errors:** `404 Not Found`

---

### POST `/sync/queue/{id}/resolve`

Resolve a `user_action_required` sync queue item. This is required when an integrity check finds both copies corrupted and no automatic recovery is possible.

**Request body:**

```json
{
    "resolution": "keep_master"
}
```

`resolution` is one of:

| Value | Meaning |
| --- | --- |
| `keep_master` | Treat the master copy as authoritative; re-mirror it to the secondary |
| `keep_mirror` | Treat the mirror copy as authoritative; restore it to the primary |
| `provide_new` | Supply a replacement file; requires `new_file_path` |

When `resolution` is `provide_new`, also include:

```json
{
    "resolution": "provide_new",
    "new_file_path": "/path/to/replacement.dat"
}
```

**Response `200`:** Updated queue item object.  
**Errors:** `400 Bad Request` (invalid resolution or file not found), `404 Not Found`

---

### DELETE `/sync/queue/completed`

Delete all sync queue rows with status `completed`.

**Response `200`:**

```json
{ "deleted": 12 }
```

`deleted` is the number of completed queue rows removed (may be `0`).

---

### POST `/sync/process`

Process all pending sync queue items immediately (synchronous). If the queue is paused (`queue_paused: true`), this call returns `{ "processed": 0 }` without touching any items.

**Response `200`:**

```json
{ "processed": 10 }
```

---

### POST `/sync/pause`

Suspend automatic sync queue processing. Items already in progress are not interrupted, but no new items will be picked up by the scheduler or `POST /sync/process` until the queue is resumed.

**Response `200`:**

```json
{ "queue_paused": true }
```

---

### POST `/sync/resume`

Resume automatic sync queue processing after a pause.

**Response `200`:**

```json
{ "queue_paused": false }
```

---

### POST `/sync/run/{task}`

Run a named task immediately. `{task}` is `sync` or `integrity-check`.

**Response `200`:**

```json
{
    "task": "sync",
    "count": 5
}
```

`count` is the number of items processed or queued by the task.  
**Errors:** `400 Bad Request` (unknown task name)

---

## Scheduler

Scheduled tasks (periodic sync and integrity checks) are managed through this resource.

### GET `/scheduler/schedules`

List all configured schedules.

**Response `200`:**

```json
[
    {
        "id": 1,
        "task_type": "sync",
        "cron_expr": null,
        "interval_seconds": 3600,
        "max_duration_seconds": null,
        "enabled": true,
        "last_run": "2026-03-29T02:00:00Z",
        "next_run": "2026-03-29T03:00:00Z",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    }
]
```

`task_type` is `sync` or `integrity_check`.

`max_duration_seconds` is an optional integer. When set, the scheduler will stop processing after that many seconds and resume remaining items on the next scheduled run. `null` means no limit.

---

### POST `/scheduler/schedules`

Create a new schedule. At least one of `cron_expr` or `interval_seconds` must be provided.

**Request body:**

```json
{
    "task_type": "integrity_check",
    "interval_seconds": 86400,
    "max_duration_seconds": 3600,
    "enabled": true
}
```

`max_duration_seconds` is optional. Omit or set to `null` for unlimited duration.

**Response `201`:** Created schedule object.  
**Errors:** `400 Bad Request`

---

### GET `/scheduler/schedules/{id}`

**Response `200`:** Single schedule object.  
**Errors:** `404 Not Found`

---

### PUT `/scheduler/schedules/{id}`

Update an existing schedule. All fields are optional.

**Request body:**

```json
{
    "interval_seconds": 43200,
    "max_duration_seconds": 1800,
    "enabled": false
}
```

To clear `max_duration_seconds` (set to unlimited), send `"max_duration_seconds": null` explicitly. Omitting the field leaves the existing value unchanged.

**Response `200`:** Updated schedule object.  
**Errors:** `404 Not Found`

---

### DELETE `/scheduler/schedules/{id}`

**Response `204`:** No content.  
**Errors:** `404 Not Found`

Changes to schedules take effect immediately — the running `Scheduler` reloads after every create, update, or delete.

---

## Event Logs

### GET `/logs`

**Query parameters:**

| Parameter | Type | Description |
| --- | --- | --- |
| `event_type` | string | Filter by event type (see schema for allowed values) |
| `file_id` | integer | Filter by tracked file |
| `from` | string | ISO 8601 start timestamp |
| `to` | string | ISO 8601 end timestamp |
| `page` | integer | Page number |
| `per_page` | integer | Items per page |

**Response `200`:**

```json
{
    "logs": [
        {
            "id": 1,
            "event_type": "integrity_pass",
            "tracked_file_id": 5,
            "message": "Integrity check passed",
            "details": "master=abc123 mirror=abc123",
            "created_at": "2026-03-15T12:00:00Z"
        }
    ],
    "total": 200,
    "page": 1,
    "per_page": 50
}
```

Valid `event_type` values: `file_created`, `file_edited`, `file_mirrored`, `integrity_pass`, `integrity_fail`, `recovery_success`, `recovery_fail`, `both_corrupted`, `change_detected`, `sync_completed`, `sync_failed`.

---

### GET `/logs/{id}`

Get a single event log entry.

**Response `200`:** Event log object.  
**Errors:** `404 Not Found`

---

## Database Backups

### GET `/database/backups`

List all backup destination configurations.

**Response `200`:** Flat JSON array of backup config objects:

```json
[
    {
        "id": 1,
        "backup_path": "/mnt/backup1/db/",
        "drive_label": "backup-drive-1",
        "max_copies": 5,
        "enabled": true,
        "last_backup": "2026-03-15T02:30:00Z",
        "created_at": "2026-01-01T00:00:00Z"
    }
]
```

---

### POST `/database/backups`

Add a backup destination.

**Request body:**

```json
{
    "backup_path": "/mnt/backup1/db/",
    "drive_label": "backup-drive-1",
    "max_copies": 5,
    "enabled": true
}
```

`drive_label`, `max_copies` (default: `5`), and `enabled` (default: `true`) are optional.

**Response `201`:** Created backup config object.  
**Errors:** `400 Bad Request`

---

### GET `/database/backups/{id}`

Get a single backup config.

**Response `200`:** Backup config object.  
**Errors:** `404 Not Found`

---

### PUT `/database/backups/{id}`

**Request body:** any subset of `max_copies`, `enabled`.

**Response `200`:** Updated config object.  
**Errors:** `404 Not Found`

---

### DELETE `/database/backups/{id}`

**Response `204`:** No content.  
**Errors:** `404 Not Found`

---

### POST `/database/backups/run`

Trigger an immediate backup to all enabled destinations.

**Required query parameter:**

| Parameter | Type | Description |
| --- | --- | --- |
| `db_path` | string | Absolute path to the live database file to copy |

**Response `200`:** Flat JSON array of per-destination results:

```json
[
    {
        "backup_config_id": 1,
        "backup_path": "/mnt/backup1/db/bitprotector-2026-03-15T023000.db",
        "status": "success",
        "error": null
    }
]
```

**Errors:** `500 Internal Server Error`

---

## System Status

### GET `/status`

Return a summary of the system state. This endpoint is also consumed by the SSH login hook (`bitprotector status`).

**Response `200`:**

```json
{
    "files_tracked": 150,
    "files_mirrored": 148,
    "pending_sync": 2,
    "integrity_issues": 0,
    "drive_pairs": 3,
    "degraded_pairs": 1,
    "active_secondary_pairs": 1,
    "rebuilding_pairs": 0,
    "quiescing_pairs": 0
}
```

---

## Error Responses

All error responses follow the same shape:

```json
{
    "error": {
        "code": "VALIDATION_ERROR",
        "message": "Short description"
    }
}
```

| HTTP status | Meaning |
| --- | --- |
| `400 Bad Request` | Missing or invalid request body / query parameters |
| `401 Unauthorized` | Missing, expired, or invalid JWT |
| `404 Not Found` | Resource does not exist |
| `409 Conflict` | Unique constraint violated (e.g., duplicate name, path already in use) |
| `500 Internal Server Error` | Unexpected server-side failure |
