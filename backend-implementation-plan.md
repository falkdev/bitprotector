# Backend Implementation Plan
## Distributed File Mirror and Integrity Protection System

> Historical planning note: sections that mention `symlink_base`, `auto_virtual_path`, `default_virtual_base`, or hidden virtual-path roots predate the literal publish-path overhaul. The current behavior is documented in `README.md`, `docs/API.md`, `docs/ARCHITECTURE.md`, and `docs/CONFIGURATION.md`.

---

## 1. Architecture Overview

### 1.1 Technology Stack

| Component         | Technology                        |
|-------------------|-----------------------------------|
| Language          | Rust                              |
| Web Framework     | Actix-web                         |
| Database          | SQLite (via rusqlite)             |
| Checksum          | BLAKE3 (blake3 crate)            |
| CLI Framework     | clap                              |
| Scheduler         | tokio-cron-scheduler              |
| Logging           | tracing + tracing-subscriber      |
| Authentication    | PAM (pam crate) + JWT sessions   |
| TLS               | rustls                           |
| Testing           | cargo test, mockall, assert_cmd  |
| Packaging         | cargo-deb (Debian/Ubuntu .deb)   |

**Rationale:** Rust satisfies the requirement for a memory-efficient, resource-efficient language suitable for resource-constrained environments (Requirement 19).

### 1.2 High-Level Architecture

```
┌──────────────┐   ┌──────────────┐
│  React Web   │   │   CLI Tool   │
│  Frontend    │   │  (bitprotect)│
└──────┬───────┘   └──────┬───────┘
       │  HTTPS           │ direct call
       ▼                  ▼
┌─────────────────────────────────┐
│         REST API (actix-web)    │
│         + JWT Auth Layer        │
├─────────────────────────────────┤
│         Core Library            │
│  ┌───────────┐ ┌─────────────┐ │
│  │ File      │ │ Integrity   │ │
│  │ Tracking  │ │ Engine      │ │
│  ├───────────┤ ├─────────────┤ │
│  │ Mirror    │ │ Virtual     │ │
│  │ Manager   │ │ Path System │ │
│  ├───────────┤ ├─────────────┤ │
│  │ Scheduler │ │ Event       │ │
│  │           │ │ Logger      │ │
│  ├───────────┤ ├─────────────┤ │
│  │ Sync      │ │ Database    │ │
│  │ Queue     │ │ Manager     │ │
│  └───────────┘ └─────────────┘ │
├─────────────────────────────────┤
│         SQLite Database         │
├─────────────────────────────────┤
│     File System (Drives)        │
└─────────────────────────────────┘
```

### 1.3 Project Structure

```
bitprotector/
├── Cargo.toml
├── src/
│   ├── main.rs                  # CLI entry point
│   ├── lib.rs                   # Core library root
│   ├── api/
│   │   ├── mod.rs
│   │   ├── server.rs            # Actix-web server setup + TLS
│   │   ├── auth.rs              # PAM auth + JWT middleware
│   │   ├── routes/
│   │   │   ├── mod.rs
│   │   │   ├── drives.rs        # Drive pair management endpoints
│   │   │   ├── files.rs         # File tracking endpoints
│   │   │   ├── virtual_paths.rs # Virtual path endpoints
│   │   │   ├── integrity.rs     # Integrity check endpoints
│   │   │   ├── sync.rs          # Sync queue endpoints
│   │   │   ├── logs.rs          # Event log endpoints
│   │   │   ├── scheduler.rs     # Schedule config endpoints
│   │   │   ├── folders.rs       # Tracked folder endpoints
│   │   │   └── database.rs      # Database backup endpoints
│   │   └── models.rs            # API request/response types
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── drives.rs
│   │   │   ├── files.rs
│   │   │   ├── virtual_paths.rs
│   │   │   ├── integrity.rs
│   │   │   ├── sync.rs
│   │   │   ├── logs.rs
│   │   │   ├── scheduler.rs
│   │   │   ├── folders.rs
│   │   │   └── database.rs
│   │   └── ssh_status.rs        # SSH login status display
│   ├── core/
│   │   ├── mod.rs
│   │   ├── checksum.rs          # BLAKE3 checksum operations
│   │   ├── mirror.rs            # File mirroring logic
│   │   ├── integrity.rs         # Integrity verification engine
│   │   ├── sync_queue.rs        # Sync queue management
│   │   ├── virtual_path.rs      # Virtual path + symlink logic
│   │   ├── tracker.rs           # File/folder tracking logic
│   │   ├── scheduler.rs         # Cron/interval scheduling
│   │   └── change_detection.rs  # File modification detection
│   ├── db/
│   │   ├── mod.rs
│   │   ├── schema.rs            # Table definitions + migrations
│   │   ├── repository.rs        # Data access layer
│   │   └── backup.rs            # Database backup operations
│   └── logging/
│       ├── mod.rs
│       └── event_logger.rs      # Application event logging
├── tests/
│   ├── unit/                    # Unit tests
│   ├── module/                  # Module-level tests
│   └── integration/             # Integration tests
├── packaging/
│   ├── debian/                  # .deb package config
│   └── qemu/                    # Installation test scripts
└── config/
    └── default.toml             # Default configuration
```

---

## 2. Database Schema

### 2.1 Tables

```sql
-- Drive pairs
CREATE TABLE drive_pairs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,
    primary_path   TEXT NOT NULL,
    secondary_path TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Tracked files
CREATE TABLE tracked_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
    relative_path   TEXT NOT NULL,
    checksum        TEXT NOT NULL,
    file_size       INTEGER NOT NULL,
    virtual_path    TEXT,
    is_mirrored     INTEGER NOT NULL DEFAULT 0,
    last_verified   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, relative_path)
);

-- Tracked folders
CREATE TABLE tracked_folders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id   INTEGER NOT NULL REFERENCES drive_pairs(id),
    folder_path     TEXT NOT NULL,
    auto_virtual_path INTEGER NOT NULL DEFAULT 0,
    default_virtual_base TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, folder_path)
);

-- Sync queue
CREATE TABLE sync_queue (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tracked_file_id INTEGER NOT NULL REFERENCES tracked_files(id),
    action          TEXT NOT NULL CHECK(action IN (
                        'mirror', 'restore_master', 'restore_mirror',
                        'verify', 'user_action_required'
                    )),
    status          TEXT NOT NULL DEFAULT 'pending' CHECK(status IN (
                        'pending', 'in_progress', 'completed', 'failed'
                    )),
    error_message   TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at    TEXT
);

-- Event log
CREATE TABLE event_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type      TEXT NOT NULL CHECK(event_type IN (
                        'file_created', 'file_edited', 'file_mirrored',
                        'integrity_pass', 'integrity_fail',
                        'recovery_success', 'recovery_fail',
                        'both_corrupted', 'change_detected',
                        'sync_completed', 'sync_failed'
                    )),
    tracked_file_id INTEGER REFERENCES tracked_files(id),
    message         TEXT NOT NULL,
    details         TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Scheduler configuration
CREATE TABLE schedule_config (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    task_type   TEXT NOT NULL CHECK(task_type IN ('sync', 'integrity_check')),
    cron_expr   TEXT,
    interval_seconds INTEGER,
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_run    TEXT,
    next_run    TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Database backup configuration
CREATE TABLE db_backup_config (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_path TEXT NOT NULL,
    drive_label TEXT,
    max_copies  INTEGER NOT NULL DEFAULT 3,
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_backup TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

## 3. Complete API Specification

**Base URL:** `https://<host>:<port>/api/v1`

All endpoints require authentication via JWT bearer token unless otherwise noted.

### 3.1 Authentication

#### POST `/auth/login`
Authenticate using local system (PAM) credentials.

**Request:**
```json
{
    "username": "string",
    "password": "string"
}
```

**Response (200):**
```json
{
    "token": "string (JWT)",
    "expires_at": "ISO 8601 timestamp"
}
```

**Errors:** `401 Unauthorized`

#### POST `/auth/logout`
Invalidate the current session token.

**Response (200):**
```json
{
    "message": "Logged out"
}
```

#### GET `/auth/status`
Return current authentication status.

**Response (200):**
```json
{
    "authenticated": true,
    "username": "string"
}
```

---

### 3.2 Drive Pairs

#### GET `/drives`
List all configured drive pairs.

**Response (200):**
```json
{
    "drive_pairs": [
        {
            "id": 1,
            "name": "string",
            "primary_path": "/mnt/primary1",
            "secondary_path": "/mnt/mirror1",
            "created_at": "ISO 8601",
            "updated_at": "ISO 8601"
        }
    ]
}
```

#### POST `/drives`
Create a new drive pair.

**Request:**
```json
{
    "name": "string",
    "primary_path": "string",
    "secondary_path": "string"
}
```

**Response (201):**
```json
{
    "id": 1,
    "name": "string",
    "primary_path": "string",
    "secondary_path": "string",
    "created_at": "ISO 8601",
    "updated_at": "ISO 8601"
}
```

**Errors:** `400 Bad Request`, `409 Conflict`

#### GET `/drives/{id}`
Get details of a specific drive pair.

**Response (200):** Single drive pair object.
**Errors:** `404 Not Found`

#### PUT `/drives/{id}`
Update a drive pair.

**Request:**
```json
{
    "name": "string (optional)",
    "primary_path": "string (optional)",
    "secondary_path": "string (optional)"
}
```

**Response (200):** Updated drive pair object.
**Errors:** `404 Not Found`, `400 Bad Request`

#### DELETE `/drives/{id}`
Remove a drive pair (fails if files are still tracked).

**Response (204):** No content.
**Errors:** `404 Not Found`, `409 Conflict` (files still tracked)

---

### 3.3 File Tracking

#### GET `/files`
List tracked files with filtering and pagination.

**Query Parameters:**
| Parameter      | Type    | Description                        |
|---------------|---------|------------------------------------|
| drive_pair_id | integer | Filter by drive pair               |
| virtual_path  | string  | Filter by virtual path prefix      |
| is_mirrored   | boolean | Filter by mirror status            |
| page          | integer | Page number (default: 1)           |
| per_page      | integer | Items per page (default: 50)       |

**Response (200):**
```json
{
    "files": [
        {
            "id": 1,
            "drive_pair_id": 1,
            "relative_path": "documents/report.pdf",
            "checksum": "blake3-hex-string",
            "file_size": 1048576,
            "virtual_path": "/docs/report.pdf",
            "is_mirrored": true,
            "last_verified": "ISO 8601",
            "created_at": "ISO 8601",
            "updated_at": "ISO 8601"
        }
    ],
    "total": 100,
    "page": 1,
    "per_page": 50
}
```

#### POST `/files`
Track a new file.

**Request:**
```json
{
    "drive_pair_id": 1,
    "relative_path": "documents/report.pdf",
    "virtual_path": "/docs/report.pdf (optional)"
}
```

**Response (201):**
```json
{
    "id": 1,
    "drive_pair_id": 1,
    "relative_path": "documents/report.pdf",
    "checksum": "blake3-hex-string",
    "file_size": 1048576,
    "virtual_path": "/docs/report.pdf",
    "is_mirrored": false,
    "created_at": "ISO 8601",
    "updated_at": "ISO 8601"
}
```

**Errors:** `400 Bad Request`, `404 Not Found` (file does not exist on primary drive), `409 Conflict`

#### GET `/files/{id}`
Get details of a tracked file.

**Response (200):** Single file object.
**Errors:** `404 Not Found`

#### DELETE `/files/{id}`
Stop tracking a file (does not delete the actual file).

**Response (204):** No content.
**Errors:** `404 Not Found`

#### POST `/files/{id}/verify`
Trigger an immediate integrity check on a specific file.

**Response (200):**
```json
{
    "file_id": 1,
    "master_checksum": "blake3-hex",
    "mirror_checksum": "blake3-hex",
    "stored_checksum": "blake3-hex",
    "master_valid": true,
    "mirror_valid": true,
    "status": "ok | master_corrupted | mirror_corrupted | both_corrupted"
}
```

---

### 3.4 Virtual Paths

#### GET `/virtual-paths`
List all virtual path mappings.

**Response (200):**
```json
{
    "virtual_paths": [
        {
            "file_id": 1,
            "virtual_path": "/docs/report.pdf",
            "real_path": "/mnt/primary1/documents/report.pdf"
        }
    ]
}
```

#### PUT `/files/{id}/virtual-path`
Set or update the virtual path for a tracked file.

**Request:**
```json
{
    "virtual_path": "/docs/report.pdf"
}
```

**Response (200):** Updated file object.
**Errors:** `404 Not Found`, `409 Conflict` (path already in use)

#### DELETE `/files/{id}/virtual-path`
Remove virtual path mapping from a tracked file.

**Response (204):** No content.
**Errors:** `404 Not Found`

#### POST `/virtual-paths/bulk`
Bulk-assign virtual paths to multiple files.

**Request:**
```json
{
    "assignments": [
        {
            "file_id": 1,
            "virtual_path": "/docs/report.pdf"
        }
    ]
}
```

**Response (200):**
```json
{
    "updated": 5,
    "failed": [
        {
            "file_id": 3,
            "error": "Virtual path already in use"
        }
    ]
}
```

#### POST `/virtual-paths/bulk-from-real`
Bulk-assign virtual paths by copying portions of the real path.

**Request:**
```json
{
    "drive_pair_id": 1,
    "source_folder": "documents/projects",
    "virtual_base": "/projects",
    "strip_prefix": "documents/"
}
```

**Response (200):**
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

#### POST `/virtual-paths/refresh-symlinks`
Regenerate all symlinks on disk from virtual path database entries.

**Response (200):**
```json
{
    "created": 45,
    "removed": 2,
    "errors": []
}
```

---

### 3.5 Tracked Folders

#### GET `/folders`
List all tracked folders.

**Response (200):**
```json
{
    "folders": [
        {
            "id": 1,
            "drive_pair_id": 1,
            "folder_path": "documents/",
            "auto_virtual_path": true,
            "default_virtual_base": "/docs",
            "created_at": "ISO 8601"
        }
    ]
}
```

#### POST `/folders`
Add a tracked folder.

**Request:**
```json
{
    "drive_pair_id": 1,
    "folder_path": "documents/",
    "auto_virtual_path": true,
    "default_virtual_base": "/docs"
}
```

**Response (201):** Created folder object.
**Errors:** `400 Bad Request`, `409 Conflict`

#### GET `/folders/{id}`
Get a specific tracked folder.

**Response (200):** Single folder object.
**Errors:** `404 Not Found`

#### PUT `/folders/{id}`
Update a tracked folder configuration.

**Request:**
```json
{
    "auto_virtual_path": true,
    "default_virtual_base": "/docs"
}
```

**Response (200):** Updated folder object.
**Errors:** `404 Not Found`

#### DELETE `/folders/{id}`
Stop tracking a folder (does not untrack already-tracked files).

**Response (204):** No content.
**Errors:** `404 Not Found`

---

### 3.6 Integrity & Sync

#### POST `/integrity/check`
Trigger a full integrity check across all tracked files.

**Response (202):**
```json
{
    "job_id": "uuid",
    "status": "started",
    "total_files": 150
}
```

#### GET `/integrity/check/{job_id}`
Get the status and results of an integrity check job.

**Response (200):**
```json
{
    "job_id": "uuid",
    "status": "running | completed | failed",
    "progress": {
        "checked": 75,
        "total": 150
    },
    "results": {
        "passed": 70,
        "master_corrupted": 2,
        "mirror_corrupted": 1,
        "both_corrupted": 0,
        "auto_recovered": 3
    }
}
```

#### GET `/sync/queue`
List items in the sync queue.

**Query Parameters:**
| Parameter | Type   | Description                              |
|-----------|--------|------------------------------------------|
| status    | string | Filter: pending, in_progress, completed, failed |
| page      | integer| Page number                              |
| per_page  | integer| Items per page                           |

**Response (200):**
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
            "created_at": "ISO 8601",
            "completed_at": null
        }
    ],
    "total": 10,
    "page": 1,
    "per_page": 50
}
```

#### POST `/sync/run`
Trigger immediate sync queue processing.

**Response (202):**
```json
{
    "job_id": "uuid",
    "status": "started",
    "queue_size": 10
}
```

#### POST `/sync/queue/{id}/resolve`
Manually resolve a sync queue item requiring user action (e.g., both-corrupted).

**Request:**
```json
{
    "resolution": "keep_master | keep_mirror | provide_new",
    "new_file_path": "string (optional, only for provide_new)"
}
```

**Response (200):** Updated queue item.
**Errors:** `404 Not Found`, `400 Bad Request`

---

### 3.7 Scheduling

#### GET `/scheduler`
List all schedule configurations.

**Response (200):**
```json
{
    "schedules": [
        {
            "id": 1,
            "task_type": "sync",
            "cron_expr": "0 2 * * *",
            "interval_seconds": null,
            "enabled": true,
            "last_run": "ISO 8601",
            "next_run": "ISO 8601"
        }
    ]
}
```

#### POST `/scheduler`
Create a new schedule.

**Request:**
```json
{
    "task_type": "sync | integrity_check",
    "cron_expr": "0 2 * * * (optional)",
    "interval_seconds": 86400,
    "enabled": true
}
```

**Response (201):** Created schedule object.
**Errors:** `400 Bad Request`

#### PUT `/scheduler/{id}`
Update a schedule configuration.

**Request:**
```json
{
    "cron_expr": "string (optional)",
    "interval_seconds": "integer (optional)",
    "enabled": "boolean (optional)"
}
```

**Response (200):** Updated schedule object.
**Errors:** `404 Not Found`

#### DELETE `/scheduler/{id}`
Remove a schedule.

**Response (204):** No content.
**Errors:** `404 Not Found`

---

### 3.8 Event Logs

#### GET `/logs`
Retrieve event logs with filtering.

**Query Parameters:**
| Parameter       | Type    | Description                          |
|----------------|---------|--------------------------------------|
| event_type     | string  | Filter by event type                 |
| tracked_file_id| integer | Filter by file                       |
| from           | string  | ISO 8601 start date                  |
| to             | string  | ISO 8601 end date                    |
| page           | integer | Page number                          |
| per_page       | integer | Items per page                       |

**Response (200):**
```json
{
    "logs": [
        {
            "id": 1,
            "event_type": "integrity_pass",
            "tracked_file_id": 5,
            "message": "Integrity check passed",
            "details": "master=abc123 mirror=abc123",
            "created_at": "ISO 8601"
        }
    ],
    "total": 500,
    "page": 1,
    "per_page": 50
}
```

---

### 3.9 Database Backup

#### GET `/database/backups`
List database backup configurations and statuses.

**Response (200):**
```json
{
    "backup_configs": [
        {
            "id": 1,
            "backup_path": "/mnt/backup1/db/",
            "drive_label": "backup-drive-1",
            "max_copies": 3,
            "enabled": true,
            "last_backup": "ISO 8601"
        }
    ]
}
```

#### POST `/database/backups`
Add a new backup destination.

**Request:**
```json
{
    "backup_path": "/mnt/backup1/db/",
    "drive_label": "backup-drive-1",
    "max_copies": 3,
    "enabled": true
}
```

**Response (201):** Created backup config object.
**Errors:** `400 Bad Request`

#### PUT `/database/backups/{id}`
Update a backup destination configuration.

**Request:**
```json
{
    "max_copies": 5,
    "enabled": false
}
```

**Response (200):** Updated backup config.
**Errors:** `404 Not Found`

#### DELETE `/database/backups/{id}`
Remove a backup destination.

**Response (204):** No content.
**Errors:** `404 Not Found`

#### POST `/database/backups/run`
Trigger an immediate database backup to all enabled destinations.

**Response (200):**
```json
{
    "results": [
        {
            "backup_config_id": 1,
            "backup_path": "/mnt/backup1/db/bitprotector-2026-03-13T120000.db",
            "status": "success"
        }
    ]
}
```

---

### 3.10 System Status

#### GET `/status`
Get overall system status (also used for SSH login message).

**Response (200):**
```json
{
    "files_tracked": 150,
    "files_mirrored": 148,
    "pending_sync": 2,
    "integrity_issues": 0,
    "files_changed": 1,
    "last_integrity_check": "ISO 8601",
    "last_sync": "ISO 8601",
    "drive_pairs": 3
}
```

---

## 4. Implementation Milestones

### Milestone 1: Project Foundation
**Objective:** Set up project structure, dependencies, database, and BLAKE3 checksum module.

**Steps:**
1. Initialize Rust project with Cargo workspace
2. Configure dependencies in `Cargo.toml`
3. Implement SQLite database schema and migrations (`db/schema.rs`)
4. Implement database connection pool and access layer (`db/repository.rs`)
5. Implement BLAKE3 checksum module (`core/checksum.rs`)
6. Write unit tests for checksum and database modules

**Tests:**
- Unit: BLAKE3 checksum computation for known inputs
- Unit: Database schema creation and migration
- Unit: CRUD operations on all tables via repository

**Commit:** `feat: project foundation with database schema and BLAKE3 checksum module`

---

### Milestone 2: Drive Pair Management
**Objective:** Implement drive pair registration and validation.

**Steps:**
1. Implement drive pair creation, listing, update, deletion (`core/mirror.rs` — pair logic)
2. Validate that primary/secondary paths exist and are writable
3. Implement CLI commands for drive pair management
4. Implement API endpoints for drive pair management

**Tests:**
- Unit: Drive pair validation logic (path exists, writable, not same path)
- Module: Drive pair CRUD through repository
- Integration: CLI drive pair commands end-to-end

**Commit:** `feat: drive pair management with CLI and API support`

---

### Milestone 3: File Tracking & Mirroring
**Objective:** Implement file tracking, checksum recording, and automatic mirroring.

**Steps:**
1. Implement file tracking logic (`core/tracker.rs`)
2. On track: compute BLAKE3 checksum, store in database
3. Implement automatic file mirroring to secondary drive (`core/mirror.rs`)
4. Implement CLI commands for file tracking
5. Implement API endpoints for file tracking

**Tests:**
- Unit: File tracking records correct metadata
- Unit: Mirror copy is byte-identical (checksum verified)
- Module: Track → Mirror → Verify round trip
- Integration: CLI file tracking with actual temp directories

**Commit:** `feat: file tracking with automatic BLAKE3-verified mirroring`

---

### Milestone 4: Integrity Verification Engine
**Objective:** Implement integrity checking with corruption detection and automatic recovery.

**Steps:**
1. Implement integrity check logic (`core/integrity.rs`)
   - Compare master checksum vs stored
   - Compare mirror checksum vs stored
   - Detect Case 1, 2, and 3 corruption scenarios
2. Implement automatic recovery for Case 1 and Case 2
3. Implement failure reporting for Case 3 (both corrupted)
4. Implement single-file and batch integrity check
5. Implement CLI and API endpoints for integrity checks

**Tests:**
- Unit: Detect master-corrupted scenario, verify recovery
- Unit: Detect mirror-corrupted scenario, verify recovery
- Unit: Detect both-corrupted scenario, verify user-action-required flagging
- Module: Full integrity check across multiple files
- Integration: Corrupt test files and verify detection + recovery

**Commit:** `feat: integrity verification engine with automatic corruption recovery`

---

### Milestone 5: Virtual Path System
**Objective:** Implement virtual path mapping with symlink support.

**Steps:**
1. Implement virtual path assignment (`core/virtual_path.rs`)
2. Implement symlink creation and management
3. Implement bulk virtual path assignment (folder-based, path-copy)
4. Implement CLI and API endpoints for virtual paths
5. Implement symlink refresh operation

**Tests:**
- Unit: Virtual path to real path mapping
- Unit: Symlink creation and validation
- Unit: Bulk assignment with path prefix stripping
- Module: End-to-end virtual path with symlink verification
- Integration: CLI virtual path commands with temp directories

**Commit:** `feat: virtual path system with symlink support and bulk assignment`

---

### Milestone 6: Folder Tracking & Change Detection
**Objective:** Implement tracked folders with auto-tracking and file change detection.

**Steps:**
1. Implement tracked folder logic (`core/tracker.rs` — folder operations)
2. Implement filesystem watcher for tracked folders (notify crate)
3. Auto-track new files in tracked folders
4. Apply default virtual path configuration for auto-tracked files
5. Implement file change detection (`core/change_detection.rs`)
6. Record changes and create notifications
7. Implement CLI and API endpoints

**Tests:**
- Unit: New file in tracked folder gets auto-tracked
- Unit: Changed file detected and flagged
- Unit: Default virtual path applied on auto-track
- Module: Folder watching → auto-track → mirror chain
- Integration: CLI folder tracking with filesystem operations

**Commit:** `feat: folder tracking with auto-detection and change notification`

---

### Milestone 7: Sync Queue & Scheduler
**Objective:** Implement sync queue processing and scheduled tasks.

**Steps:**
1. Implement sync queue management (`core/sync_queue.rs`)
2. Queue items auto-created from integrity failures and change detections
3. Implement sync queue processing (mirror, restore, verify)
4. Implement user-action resolution for both-corrupted items
5. Implement scheduler (`core/scheduler.rs`) with cron and interval support
6. Schedule sync and integrity check tasks
7. Implement CLI and API endpoints

**Tests:**
- Unit: Queue item creation from integrity failure
- Unit: Queue processing handles each action type
- Unit: Scheduler fires at configured times (mocked clock)
- Module: Integrity failure → queue → resolve cycle
- Integration: Full scheduled sync with tempdir setup

**Commit:** `feat: sync queue management and task scheduler`

---

### Milestone 8: Event Logging
**Objective:** Implement comprehensive event logging system.

**Steps:**
1. Implement event logger (`logging/event_logger.rs`)
2. Integrate logging into all core operations:
   - File creation / edit / mirror
   - Integrity checks (pass/fail)
   - Recovery actions
3. Implement log retrieval with filtering
4. Implement CLI and API endpoints for log access

**Tests:**
- Unit: Events recorded for each operation type
- Unit: Log filtering by type, date, file
- Module: Operation → log creation → log retrieval
- Integration: CLI log viewing

**Commit:** `feat: comprehensive event logging system`

---

### Milestone 9: Database Backup
**Objective:** Implement database backup configuration and execution.

**Steps:**
1. Implement backup configuration management (`db/backup.rs`)
2. Implement backup execution (copy master database to configured paths)
3. Implement backup rotation (respect max_copies)
4. Implement CLI and API endpoints for backup management

**Tests:**
- Unit: Backup copy creates exact duplicate
- Unit: Rotation deletes oldest when max exceeded
- Unit: Backup to multiple destinations
- Module: Configure → backup → verify → rotate
- Integration: CLI backup commands with temp directories

**Commit:** `feat: database backup with rotation and multi-destination support`

---

### Milestone 10: Authentication & Secure Communication
**Objective:** Implement PAM authentication, JWT sessions, and TLS.

**Steps:**
1. Implement PAM authentication (`api/auth.rs`)
2. Implement JWT token issuance and validation
3. Implement auth middleware for API routes
4. Configure TLS with rustls for HTTPS
5. Implement CLI authentication (for remote API access)

**Tests:**
- Unit: JWT token creation and validation
- Unit: Auth middleware rejects unauthenticated requests
- Module: Login → token → authenticated request cycle
- Integration: Full TLS + auth flow with test certificates

**Commit:** `feat: PAM authentication with JWT sessions and TLS encryption`

---

### Milestone 11: SSH Login Status
**Objective:** Implement SSH login status message display.

**Steps:**
1. Implement status message generator (`cli/ssh_status.rs`)
2. Query system status (changed files, integrity issues, pending sync)
3. Format concise status message for terminal display
4. Provide installation hook (MOTD or profile.d script)

**Tests:**
- Unit: Status message formatting with various states
- Unit: Correct flags for changes, issues, pending items
- Integration: SSH status script output with mock data

**Commit:** `feat: SSH login status notification with system health summary`

---

### Milestone 12: Web Server & API Integration
**Objective:** Finalize the Actix-web server with all routes, middleware, and error handling.

**Steps:**
1. Set up Actix-web server with TLS configuration (`api/server.rs`)
2. Wire all route handlers
3. Implement consistent error response format
4. Implement request validation and rate limiting
5. Add CORS configuration for frontend
6. Implement API versioning (v1 prefix)

**Tests:**
- Integration: All API endpoints respond correctly
- Integration: Error responses have consistent format
- Integration: CORS headers present
- Integration: Unauthenticated requests rejected

**Commit:** `feat: complete REST API server with all endpoints and middleware`

---

### Milestone 13: Packaging & Installation Testing
**Objective:** Package the application for Ubuntu 24 and verify installation.

**Steps:**
1. Configure `cargo-deb` for .deb package generation
2. Include systemd service file for background daemon
3. Include default configuration file
4. Include SSH status script (profile.d hook)
5. Create QEMU-based installation test:
   - Boot Ubuntu 24 cloud image
   - Install .deb package
   - Verify service starts
   - Run basic smoke tests via CLI
   - Verify API is accessible

**Tests:**
- Integration: .deb package builds successfully
- Integration: Package installs on clean Ubuntu 24
- Integration: Service starts and API responds
- Integration: CLI commands function post-install
- Integration: SSH login displays status message

**Commit:** `feat: Ubuntu 24 .deb packaging with QEMU installation tests`

---

## 5. Testing Strategy

### 5.1 Test Levels

| Level       | Scope                                    | Tools                        |
|-------------|------------------------------------------|------------------------------|
| Unit        | Individual functions and structs          | `#[cfg(test)]`, mockall      |
| Module      | Component interaction within a module    | `tests/module/`, tempdir     |
| Integration | Full system behavior, CLI, API           | `tests/integration/`, assert_cmd, reqwest |
| Installation| Package install on real Ubuntu 24 VM     | QEMU, cloud-init, shell scripts |

### 5.2 Test Conventions

- **TDD workflow:** Write failing test → implement → pass → refactor
- All tests run with `cargo test`
- Integration tests use temporary directories for drive simulation
- Database tests use in-memory SQLite or temp file databases
- API integration tests spin up a test server instance
- File corruption tests create real files and modify bytes

### 5.3 Test Coverage Targets

| Module                | Minimum Coverage |
|-----------------------|-----------------|
| core/checksum         | 100%            |
| core/integrity        | 95%             |
| core/mirror           | 95%             |
| core/virtual_path     | 90%             |
| core/sync_queue       | 90%             |
| db/repository         | 90%             |
| api/routes/*          | 85%             |
| cli/commands/*        | 80%             |

---

## 6. Configuration

Default configuration file (`/etc/bitprotector/config.toml`):

```toml
[server]
host = "0.0.0.0"
port = 8443
tls_cert = "/etc/bitprotector/cert.pem"
tls_key = "/etc/bitprotector/key.pem"

[database]
path = "/var/lib/bitprotector/bitprotector.db"

[virtual_paths]
symlink_base = "/var/lib/bitprotector/virtual/"

[logging]
level = "info"
file = "/var/log/bitprotector/bitprotector.log"

[scheduler]
enabled = true
```

---

## 7. Error Response Format

All API errors use a consistent JSON structure:

```json
{
    "error": {
        "code": "RESOURCE_NOT_FOUND",
        "message": "Drive pair with id 99 not found",
        "details": null
    }
}
```

Standard error codes:
- `VALIDATION_ERROR` (400)
- `UNAUTHORIZED` (401)
- `FORBIDDEN` (403)
- `RESOURCE_NOT_FOUND` (404)
- `CONFLICT` (409)
- `INTERNAL_ERROR` (500)
