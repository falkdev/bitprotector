# Architecture

This document describes the overall system design, module responsibilities, database schema, and key design decisions.

---

## Table of Contents

- [System Overview](#system-overview)
- [Module Breakdown](#module-breakdown)
  - [src/core/](#srccore)
  - [src/api/](#srcapi)
  - [src/cli/](#srccli)
  - [src/db/](#srcdb)
  - [src/logging/](#srclogging)
- [Database Schema](#database-schema)
- [Key Design Decisions](#key-design-decisions)

---

## System Overview

BitProtector is compiled as a single binary (`bitprotector`) that exposes two interfaces from the same core library:

```text
┌─────────────────────────┐  ┌────────────────────────────┐
│   React Web Frontend    │  │      CLI (bitprotector)    │
│   (HTTPS / Axios)       │  │      clap subcommands      │
└────────────┬────────────┘  └─────────────┬──────────────┘
             │ HTTPS                       │ direct function call
             └──────────────┬──────────────┘
                            ▼
             ┌──────────────────────────────┐
             │   REST API   (actix-web)     │
             │   JWT auth middleware        │
             │   src/api/                   │
             └──────────────┬───────────────┘
                            │
             ┌──────────────▼───────────────┐
             │   Core Library               │
             │   src/core/  src/db/         │
             │   src/logging/               │
             └──────────────┬───────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
   SQLite database    Primary drives      Mirror drives
   (single .db file)  /mnt/primary*/      /mnt/mirror*/
```

The Rust crate compiles both a **library** (`bitprotector_lib`, `src/lib.rs`) and a **binary** (`bitprotector`, `src/main.rs`). The CLI entry point and the API server both import from the library. Integration tests also import directly from the library crate.

---

## Module Breakdown

### src/core/

Business logic and algorithms. Has no knowledge of HTTP or CLI argument parsing.

| File | Responsibility |
| --- | --- |
| `checksum.rs` | Compute and verify BLAKE3 hashes for files. Provides the canonical checksum function used everywhere. |
| `drive.rs` | Drive-role state machine and path resolution helpers. Owns planned quiesce/failover, emergency failover, replacement assignment, rebuild completion, and virtual-path retargeting. |
| `mirror.rs` | Copy a file from the primary path to the secondary path, verifying the copy with a post-write checksum. Also responsible for restoring a corrupted copy from its healthy counterpart. |
| `integrity.rs` | Orchestrate integrity checks: re-hash the current active and standby copies, compare against the stored baseline, classify the result (ok / master\_corrupted / mirror\_corrupted / both\_corrupted / drive\_unavailable), and trigger automatic recovery only when a healthy counterpart exists. |
| `sync_queue.rs` | Manage the queue of files awaiting a sync or verify action. Provides logic to process the queue, update item status, and surface failures. |
| `virtual_path.rs` | Map a tracked file to a user-visible virtual path. Creates and removes symlinks in the `symlink_base` directory and refreshes them whenever `active_role` changes. |
| `tracker.rs` | Track or untrack individual files and folders. On track: compute initial checksum, store metadata, enqueue mirroring, and operate on the pair's current active side rather than assuming primary is always live. |
| `scheduler.rs` | Manage background task execution. Provides `run_task` for on-demand execution and a `Scheduler` struct that spawns OS threads running tasks at fixed intervals using `thread::sleep`. |
| `change_detection.rs` | Watch the file system for modifications using the `notify` crate. When a tracked file changes, updates the checksum from the active side and enqueues a re-mirror only if the standby slot can currently accept sync. |

### src/api/

HTTP layer. Translates HTTP requests into calls to `src/core/` and `src/db/`, and serialises results to JSON.

| File | Responsibility |
| --- | --- |
| `server.rs` | Build and start the actix-web server. Mounts all route groups, configures TLS via rustls, and injects shared state (`Repository`, `JwtSecret`) as `web::Data`. |
| `auth.rs` | PAM authentication (`pam` crate) to verify system credentials. Issues and validates JWT tokens (`jsonwebtoken` crate). Provides `JwtAuth` as an actix-web extractor for protecting routes. |
| `models.rs` | Request and response DTOs (`serde::Deserialize` / `Serialize`). Kept separate from the database structs in `src/db/`. |
| `routes/` | One file per resource group, each registering its own `actix_web::web::ServiceConfig`. See [API.md](API.md) for the full endpoint reference. |

### src/cli/

Command-line interface. Parses arguments with `clap` (derive macros) and calls the same core functions as the API routes.

| File | Responsibility |
| --- | --- |
| `mod.rs` | Defines the root `Cli` struct and the `Commands` enum that clap dispatches on. |
| `ssh_status.rs` | Logic for the `bitprotector status` command — queries the database and formats a one-screen summary suitable for display on SSH login (via `/etc/profile.d/bitprotector-status.sh`). |
| `commands/` | One file per subcommand group (drives, files, folders, virtual\_paths, integrity, sync, logs, scheduler, database). Each matches the structure of the corresponding API route file. |

### src/db/

Data persistence. All SQLite access goes through this module.

| File | Responsibility |
| --- | --- |
| `schema.rs` | `CREATE TABLE` statements and schema migration logic. Runs at startup to ensure the database is up to date. |
| `repository.rs` | Data access object (DAO). One method per database operation. The trait is annotated with `#[cfg_attr(test, mockall::automock)]` so unit tests can inject a mock. |
| `backup.rs` | Copy the live database file to each configured backup destination, rotating old copies when `max_copies` is exceeded. |

Connection pooling is provided by `r2d2` + `r2d2_sqlite`. The pool is wrapped inside `Repository` which is shared across all actix-web workers via `web::Data<Repository>`.

### src/logging/

| File | Responsibility |
| --- | --- |
| `event_logger.rs` | Write structured events to the `event_log` table and simultaneously emit `tracing` spans. Events include file lifecycle changes, integrity outcomes, sync results, and recovery actions. |

---

## Database Schema

Single SQLite database file (default: `/var/lib/bitprotector/bitprotector.db`).

```sql
-- Drive pairs
CREATE TABLE drive_pairs (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL UNIQUE,
    primary_path   TEXT    NOT NULL,
    secondary_path TEXT    NOT NULL,
    primary_state  TEXT    NOT NULL DEFAULT 'active',
    secondary_state TEXT   NOT NULL DEFAULT 'active',
    active_role    TEXT    NOT NULL DEFAULT 'primary',
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Tracked files
CREATE TABLE tracked_files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id INTEGER NOT NULL REFERENCES drive_pairs(id),
    relative_path TEXT    NOT NULL,
    checksum      TEXT    NOT NULL,       -- BLAKE3 hex of the known-good copy
    file_size     INTEGER NOT NULL,
    virtual_path  TEXT,
    is_mirrored   INTEGER NOT NULL DEFAULT 0,
    last_verified TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, relative_path)
);

-- Tracked folders (auto-discover new files)
CREATE TABLE tracked_folders (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id        INTEGER NOT NULL REFERENCES drive_pairs(id),
    folder_path          TEXT    NOT NULL,
    auto_virtual_path    INTEGER NOT NULL DEFAULT 0,
    default_virtual_base TEXT,
    created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, folder_path)
);

-- Files awaiting action
CREATE TABLE sync_queue (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tracked_file_id INTEGER NOT NULL REFERENCES tracked_files(id),
    action          TEXT    NOT NULL CHECK(action IN (
                        'mirror', 'restore_master', 'restore_mirror',
                        'verify', 'user_action_required'
                    )),
    status          TEXT    NOT NULL DEFAULT 'pending' CHECK(status IN (
                        'pending', 'in_progress', 'completed', 'failed'
                    )),
    error_message   TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
    completed_at    TEXT
);

-- System events
CREATE TABLE event_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type      TEXT    NOT NULL CHECK(event_type IN (
                        'file_created', 'file_edited', 'file_mirrored',
                        'integrity_pass', 'integrity_fail',
                        'recovery_success', 'recovery_fail',
                        'both_corrupted', 'change_detected',
                        'sync_completed', 'sync_failed'
                    )),
    tracked_file_id INTEGER REFERENCES tracked_files(id) ON DELETE SET NULL,
    message         TEXT    NOT NULL,
    details         TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Scheduled tasks
CREATE TABLE schedule_config (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    task_type        TEXT    NOT NULL CHECK(task_type IN ('sync', 'integrity_check')),
    cron_expr        TEXT,
    interval_seconds INTEGER,
    enabled          INTEGER NOT NULL DEFAULT 1,
    last_run         TEXT,
    next_run         TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Database backup destinations
CREATE TABLE db_backup_config (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    backup_path TEXT    NOT NULL,
    drive_label TEXT,
    max_copies  INTEGER NOT NULL DEFAULT 5,
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_backup TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
```

---

## Key Design Decisions

### Single binary, shared core

The CLI and the API server are two entry points into the same library crate. Test code also imports the library directly. This avoids duplicating business logic and ensures CLI and API behave identically for the same operation.

### PAM authentication — no separate user database

The REST API authenticates against the host's system accounts via PAM. There is no user table in the database. This means any valid system user can be granted access, and password management remains with the OS. The JWT `sub` field records the authenticated username.

### BLAKE3 for checksums

BLAKE3 is faster than SHA-256/SHA-512 on modern hardware and provides sufficient collision resistance. The stored checksum is a hex-encoded 256-bit (32-byte) hash.

### Virtual paths are symlinks on disk

The virtual path system does not intercept file I/O. Instead it creates regular symlinks in a dedicated directory (`symlink_base`). This makes virtual paths transparent to any tool that can follow symlinks, requires no kernel module or FUSE, and is straightforward to regenerate from the database (`POST /virtual-paths/refresh`). During drive failover, the symlink target is refreshed from the pair's `active_role`, so future opens move to the surviving side.

### Drive failover is stateful, not path-swapping in place

Each drive pair now tracks `primary_state`, `secondary_state`, and `active_role`. Planned replacements move a slot through `active -> quiescing -> failed -> rebuilding -> active`. Unexpected loss of the active root can trigger emergency failover to the healthy side. The logical pair and relative paths stay stable; only the operational source/target root changes.

### r2d2 connection pool for SQLite

Even though SQLite serialises writes, using a pool with `r2d2_sqlite` allows read operations across actix-web worker threads without acquiring a global lock.

### Background task scheduling via OS threads

Background tasks (scheduled sync, scheduled integrity check) run in dedicated OS threads spawned by `Scheduler::schedule`. Each thread loops: run the task, then sleep for the configured interval (checked in 100 ms increments so the `stop_flag` is responsive). Tasks can also be triggered on demand via `run_task` without any scheduler involvement.
