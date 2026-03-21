# REST API Reference

**Base URL:** `https://<host>:<port>/api/v1`  
**Transport:** HTTPS only (TLS required)  
**Authentication:** JWT bearer token (all endpoints except `POST /auth/login`)

---

## Table of Contents

- [Authentication](#authentication)
- [Drive Pairs](#drive-pairs)
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

```
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
    "expires_at": "2026-03-15T20:00:00Z"
}
```

**Errors:** `401 Unauthorized`

---

### POST `/auth/logout`

Invalidate the current session token.

**Response `200`:**
```json
{ "message": "Logged out" }
```

---

### GET `/auth/status`

Check whether the current token is valid.

**Response `200`:**
```json
{
    "authenticated": true,
    "username": "alice"
}
```

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

## File Tracking

### GET `/files`

List tracked files with optional filtering and pagination.

**Query parameters:**

| Parameter | Type | Description |
|---|---|---|
| `drive_pair_id` | integer | Filter by drive pair |
| `virtual_path` | string | Filter by virtual path prefix |
| `is_mirrored` | boolean | Filter by mirror status |
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
            "last_verified": "2026-03-14T12:00:00Z",
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

Start tracking a file. BitProtector computes its BLAKE3 checksum and enqueues an initial mirror from the pair's current active side.

**Request body:**
```json
{
    "drive_pair_id": 1,
    "relative_path": "documents/report.pdf",
    "virtual_path": "/docs/report.pdf"
}
```

`virtual_path` is optional.

**Response `201`:** Newly created file object.  
**Errors:** `400 Bad Request`, `404 Not Found` (file not on primary drive), `409 Conflict`

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

### POST `/files/{id}/verify`

Trigger an immediate integrity check for one file.

**Response `200`:**
```json
{
    "file_id": 1,
    "master_checksum": "d74981ef...",
    "mirror_checksum": "d74981ef...",
    "stored_checksum": "d74981ef...",
    "master_valid": true,
    "mirror_valid":  true,
    "status": "ok"
}
```

`status` is one of: `ok` | `master_corrupted` | `mirror_corrupted` | `both_corrupted`.

---

## Virtual Paths

Virtual paths expose tracked files through a user-defined path, realised as symlinks under `symlink_base` (see [CONFIGURATION.md](CONFIGURATION.md)).

### GET `/virtual-paths`

List all virtual path mappings.

**Response `200`:**
```json
{
    "virtual_paths": [
        {
            "file_id": 1,
            "virtual_path": "/docs/report.pdf",
            "real_path": "/mnt/primary/documents/report.pdf"
        }
    ]
}
```

---

### PUT `/files/{id}/virtual-path`

Set or update the virtual path for a tracked file.

**Request body:**
```json
{ "virtual_path": "/docs/report.pdf" }
```

**Response `200`:** Updated file object.  
**Errors:** `404 Not Found`, `409 Conflict` (path already assigned to another file)

---

### DELETE `/files/{id}/virtual-path`

Remove the virtual path mapping (and its symlink) from a file.

**Response `204`:** No content.  
**Errors:** `404 Not Found`

---

### POST `/virtual-paths/bulk`

Assign virtual paths to multiple files in a single request.

**Request body:**
```json
{
    "assignments": [
        { "file_id": 1, "virtual_path": "/docs/report.pdf" },
        { "file_id": 2, "virtual_path": "/docs/summary.pdf" }
    ]
}
```

**Response `200`:**
```json
{
    "updated": 2,
    "failed": []
}
```

Failed entries include `{ "file_id": N, "error": "..." }`.

---

### POST `/virtual-paths/bulk-from-real`

Derive virtual paths from real paths by stripping a prefix and prepending a base.

**Request body:**
```json
{
    "drive_pair_id": 1,
    "source_folder": "documents/projects",
    "virtual_base": "/projects",
    "strip_prefix": "documents/"
}
```

**Response `200`:**
```json
{
    "updated": 10,
    "mappings": [
        {
            "file_id": 1,
            "real_path": "documents/projects/alpha/spec.txt",
            "virtual_path": "/projects/alpha/spec.txt"
        }
    ]
}
```

---

### POST `/virtual-paths/refresh-symlinks`

Regenerate all symlinks on disk from the database. Useful after the `symlink_base` directory is lost or moved.

**Response `200`:**
```json
{
    "created": 45,
    "removed": 2,
    "errors": []
}
```

---

## Tracked Folders

Tracked folders let BitProtector automatically discover and track new files added to a directory.

### GET `/folders`

**Response `200`:**
```json
{
    "folders": [
        {
            "id": 1,
            "drive_pair_id": 1,
            "folder_path": "documents/",
            "auto_virtual_path": true,
            "default_virtual_base": "/docs",
            "created_at": "2026-01-01T00:00:00Z"
        }
    ]
}
```

---

### POST `/folders`

**Request body:**
```json
{
    "drive_pair_id": 1,
    "folder_path": "documents/",
    "auto_virtual_path": true,
    "default_virtual_base": "/docs"
}
```

`auto_virtual_path` and `default_virtual_base` are optional.

**Response `201`:** Created folder object.  
**Errors:** `400 Bad Request`, `409 Conflict`

---

### GET `/folders/{id}`

**Response `200`:** Single folder object.  
**Errors:** `404 Not Found`

---

### PUT `/folders/{id}`

**Request body:** any subset of `auto_virtual_path`, `default_virtual_base`.

**Response `200`:** Updated folder object.  
**Errors:** `404 Not Found`

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

### GET `/integrity/check-all`

Run integrity checks across all tracked files, or limit them to one drive pair with `?drive_id=<id>`. Add `&recover=true` to enable auto-recovery where possible.

**Response `200`:**
```json
{
    "results": [
        {
            "file_id": 1,
            "status": "ok",
            "recovered": false
        },
        {
            "file_id": 2,
            "status": "secondary_drive_unavailable",
            "recovered": false
        }
    ]
}
```

When a slot is deliberately failed or rebuilding, integrity checks validate the surviving active side and avoid generating a restore storm for the unavailable slot.

---

## Sync Queue

### GET `/sync/queue`

**Query parameters:**

| Parameter | Type | Description |
|---|---|---|
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
            "relative_path": "data/file.bin",
            "action": "mirror",
            "status": "pending",
            "error_message": null,
            "created_at": "2026-03-15T08:00:00Z",
            "completed_at": null
        }
    ],
    "total": 10,
    "page": 1,
    "per_page": 50
}
```

---

### POST `/sync/run`

Process the sync queue immediately (asynchronous).

**Response `202`:**
```json
{
    "job_id": "...",
    "status": "started",
    "queue_size": 10
}
```

---

### POST `/sync/queue/{id}/resolve`

Manually resolve a queue item with action `user_action_required` (i.e., both master and mirror are corrupted).

**Request body:**
```json
{
    "resolution": "keep_master",
    "new_file_path": null
}
```

`resolution` is one of: `keep_master` | `keep_mirror` | `provide_new`.  
`new_file_path` is required only when `resolution` is `provide_new`.

**Response `200`:** Updated queue item.  
**Errors:** `404 Not Found`, `400 Bad Request`

---

## Scheduler

### GET `/scheduler`

**Response `200`:**
```json
{
    "schedules": [
        {
            "id": 1,
            "task_type": "sync",
            "cron_expr": "0 2 * * *",
            "interval_seconds": null,
            "enabled": true,
            "last_run": "2026-03-15T02:00:00Z",
            "next_run": "2026-03-16T02:00:00Z"
        }
    ]
}
```

---

### POST `/scheduler`

**Request body:**
```json
{
    "task_type": "integrity_check",
    "cron_expr": "0 3 * * 0",
    "interval_seconds": null,
    "enabled": true
}
```

Provide exactly one of `cron_expr` or `interval_seconds`.

**Response `201`:** Created schedule object.  
**Errors:** `400 Bad Request`

---

### PUT `/scheduler/{id}`

**Request body:** any subset of `cron_expr`, `interval_seconds`, `enabled`.

**Response `200`:** Updated schedule object.  
**Errors:** `404 Not Found`

---

### DELETE `/scheduler/{id}`

**Response `204`:** No content.  
**Errors:** `404 Not Found`

---

## Event Logs

### GET `/logs`

**Query parameters:**

| Parameter | Type | Description |
|---|---|---|
| `event_type` | string | Filter by event type (see schema for allowed values) |
| `tracked_file_id` | integer | Filter by file |
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
    "total": 500,
    "page": 1,
    "per_page": 50
}
```

Valid `event_type` values: `file_created`, `file_edited`, `file_mirrored`, `integrity_pass`, `integrity_fail`, `recovery_success`, `recovery_fail`, `both_corrupted`, `change_detected`, `sync_completed`, `sync_failed`.

---

## Database Backups

### GET `/database/backups`

List all backup destination configurations.

**Response `200`:**
```json
{
    "backup_configs": [
        {
            "id": 1,
            "backup_path": "/mnt/backup1/db/",
            "drive_label": "backup-drive-1",
            "max_copies": 3,
            "enabled": true,
            "last_backup": "2026-03-15T02:30:00Z"
        }
    ]
}
```

---

### POST `/database/backups`

Add a backup destination.

**Request body:**
```json
{
    "backup_path": "/mnt/backup1/db/",
    "drive_label": "backup-drive-1",
    "max_copies": 3,
    "enabled": true
}
```

**Response `201`:** Created backup config object.  
**Errors:** `400 Bad Request`

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

**Response `200`:**
```json
{
    "results": [
        {
            "backup_config_id": 1,
            "backup_path": "/mnt/backup1/db/bitprotector-2026-03-15T023000.db",
            "status": "success"
        }
    ]
}
```

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
|---|---|
| `400 Bad Request` | Missing or invalid request body / query parameters |
| `401 Unauthorized` | Missing, expired, or invalid JWT |
| `404 Not Found` | Resource does not exist |
| `409 Conflict` | Unique constraint violated (e.g., duplicate name, path already in use) |
| `500 Internal Server Error` | Unexpected server-side failure |
