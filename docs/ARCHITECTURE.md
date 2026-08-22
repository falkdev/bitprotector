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
| `integrity.rs` | Orchestrate integrity checks: re-hash the current active and standby copies, compare against the stored baseline, classify the result (ok / master\_corrupted / mirror\_corrupted / both\_corrupted / mirror\_missing / master\_missing / primary\_drive\_unavailable / secondary\_drive\_unavailable), and trigger automatic recovery only when a healthy counterpart exists. |
| `integrity_runs.rs` | Shared full-run service used by API, CLI, and scheduler. Persists run metadata + per-file outcomes, enforces single active run, supports cooperative stop, and processes tracked files in paged batches. Accepts an optional `deadline: Option<Instant>`; when set, processing stops after the deadline is reached and remaining files are left for the next run. Files are always processed in oldest-`last_integrity_check_at`-first order so that every file gets eventual coverage. |
| `sync_queue.rs` | Manage the queue of files awaiting a sync or verify action. Provides logic to process the queue, update item status, and surface failures. `process_all_pending` groups pending items by the drive pair their tracked file belongs to and processes each drive pair's items concurrently on its own thread, while items within the same drive pair stay strictly sequential (so two transfers never hit the same physical disk at once). It accepts an optional `stop_by: Option<Instant>` deadline; before each pass and before each item, it checks both the deadline and the global `sync_settings.queue_paused` flag — if the queue is paused, processing stops and the item is not consumed. |
| `virtual_path.rs` | Map tracked files and folders to literal virtual paths. Creates and removes symlinks exactly at those absolute paths and refreshes them whenever `active_role` changes. |
| `tracker.rs` | Track or untrack individual files and folders. On track/scan: normalise file permissions to world-readable (adds `o+r`, adds `o+x` only if `u+x` is set), compute initial checksum, store metadata, enqueue mirror work by default, and operate on the pair's current active side rather than assuming primary is always live. |
| `scheduler.rs` | Manage background task execution. Provides `run_task` for on-demand execution and a `Scheduler` struct that spawns OS threads running tasks at configured intervals or cron schedules, sleeping in 100 ms increments so the stop flag is responsive. When a schedule has `max_duration_seconds` set, the thread computes an `Instant` deadline before calling `run_task`, which passes it through to the underlying `process_all_pending` or `run_sync` call. Integrity task execution persists run/result records via `integrity_runs.rs`. |
| `change_detection.rs` | Watch the file system for modifications using the `notify` crate. When a tracked file changes, updates the checksum from the active side and enqueues a re-mirror only if the standby slot can currently accept sync. |
| `system.rs` | System introspection helpers. Reads `/proc/meminfo` for total RAM, derives `mmap_threshold_bytes()` (50% of RAM, upper bound for mmap-based hashing on SSD pairs), and `default_ssd_parallel()` (half logical CPUs, minimum 2) used by the checksum parallelism configuration. |

### src/api/

HTTP layer. Translates HTTP requests into calls to `src/core/` and `src/db/`, and serialises results to JSON.

| File | Responsibility |
| --- | --- |
| `server.rs` | Build and start the actix-web server. Mounts all route groups, configures TLS via rustls, and injects shared state (`Repository`, `JwtSecret`) as `web::Data`. |
| `auth.rs` | PAM authentication (`pam` crate) to verify system credentials. Issues and validates JWT tokens (`jsonwebtoken` crate). Provides `JwtAuth` as an actix-web extractor for protecting routes. |
| `models.rs` | Request and response DTOs (`serde::Deserialize` / `Serialize`). Kept separate from the database structs in `src/db/`. |
| `path_resolution.rs` | API-layer helper for validating host paths submitted by the web UI and converting tracked file/folder selections back into drive-relative paths. Rejects traversal and canonicalized escapes before core tracking logic runs. |
| `routes/` | One file per resource group, each registering its own `actix_web::web::ServiceConfig`. This includes the read-only filesystem browse route (`/filesystem/children`), the mixed tracking listing route (`/tracking/items`) with effective virtual-path derivation and folder aggregate status, and lazy virtual-path tree route (`/virtual-paths/tree`) used by the web UI. See [API.md](API.md) for the full endpoint reference. |

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
| `backup.rs` | Create SQLite-aware database backup snapshots, write one canonical `bitprotector.db` per destination, verify/repair configured backups, and stage safe restores for startup. |

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
    primary_media_type   TEXT    NOT NULL DEFAULT 'hdd',
    secondary_media_type TEXT    NOT NULL DEFAULT 'hdd',
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Create-time invariant: primary_path and secondary_path form one shared
-- uniqueness set across all drive pairs. Once a physical path is registered in
-- either slot, later INSERTs may not reuse it in either slot. Replacement and
-- explicit update flows continue to use UPDATE statements and are unchanged.

-- Tracked files
CREATE TABLE tracked_files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id INTEGER NOT NULL REFERENCES drive_pairs(id),
    relative_path TEXT    NOT NULL,
    checksum      TEXT    NOT NULL,       -- BLAKE3 hex of the known-good copy
    file_size     INTEGER NOT NULL,
    virtual_path  TEXT,
    is_mirrored   INTEGER NOT NULL DEFAULT 0,
    tracked_direct INTEGER NOT NULL DEFAULT 1,      -- direct vs folder provenance are runtime-mutually-exclusive
    tracked_via_folder INTEGER NOT NULL DEFAULT 0,  -- legacy dual-source rows are normalized to direct
    last_integrity_check_at TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, relative_path)
);

-- Persisted integrity run summary (full runs only)
CREATE TABLE integrity_runs (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    scope_drive_pair_id INTEGER REFERENCES drive_pairs(id) ON DELETE SET NULL,
    recover            INTEGER NOT NULL DEFAULT 0,
    trigger            TEXT    NOT NULL,
    status             TEXT    NOT NULL CHECK(status IN ('running', 'stopping', 'stopped', 'completed', 'failed')),
    total_files        INTEGER NOT NULL DEFAULT 0,
    processed_files    INTEGER NOT NULL DEFAULT 0,
    attention_files    INTEGER NOT NULL DEFAULT 0,
    recovered_files    INTEGER NOT NULL DEFAULT 0,
    active_workers     INTEGER NOT NULL DEFAULT 0,
    stop_requested     INTEGER NOT NULL DEFAULT 0,
    started_at         TEXT    NOT NULL DEFAULT (datetime('now')),
    ended_at           TEXT,
    error_message      TEXT
);

-- Persisted per-file results for each run
CREATE TABLE integrity_run_results (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id           INTEGER NOT NULL REFERENCES integrity_runs(id) ON DELETE CASCADE,
    file_id          INTEGER NOT NULL REFERENCES tracked_files(id) ON DELETE CASCADE,
    drive_pair_id    INTEGER NOT NULL REFERENCES drive_pairs(id),
    relative_path    TEXT    NOT NULL,
    status           TEXT    NOT NULL,
    recovered        INTEGER NOT NULL DEFAULT 0,
    needs_attention  INTEGER NOT NULL DEFAULT 0,
    checked_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Tracked folders (auto-discover new files)
CREATE TABLE tracked_folders (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    drive_pair_id        INTEGER NOT NULL REFERENCES drive_pairs(id),
    folder_path          TEXT    NOT NULL,
    virtual_path         TEXT,
    last_scanned_at      TEXT,
    created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(drive_pair_id, folder_path)
);

CREATE INDEX idx_tracked_files_virtual_path
    ON tracked_files(virtual_path);

CREATE INDEX idx_tracked_files_drive_relative
    ON tracked_files(drive_pair_id, relative_path);

CREATE INDEX idx_integrity_runs_active
    ON integrity_runs(status, started_at);

CREATE INDEX idx_integrity_run_results_issue
    ON integrity_run_results(run_id, needs_attention, id);

CREATE INDEX idx_tracked_folders_drive_folder_path
    ON tracked_folders(drive_pair_id, folder_path);

CREATE INDEX idx_tracked_files_integrity_check
    ON tracked_files(last_integrity_check_at);

-- Legacy migrated databases may still contain `auto_virtual_path` and
-- `default_virtual_base`, but they are no longer used by the application.

-- Files awaiting action
CREATE TABLE sync_queue (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    tracked_file_id INTEGER NOT NULL REFERENCES tracked_files(id),
    action          TEXT    NOT NULL CHECK(action IN (
                        'mirror', 'restore_master', 'restore_mirror',
                        'verify', 'user_action_required', 'adopt_mirror'
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
                        'file_untracked',
                        'integrity_pass', 'integrity_fail',
                        'recovery_success', 'recovery_fail',
                        'both_corrupted', 'change_detected',
                        'sync_completed', 'sync_failed',
                        'folder_tracked', 'folder_untracked',
                        'integrity_run_started', 'integrity_run_completed',
                        'drive_created', 'drive_updated', 'drive_deleted',
                        'drive_failover', 'drive_quiescing', 'drive_quiesce_cancelled',
                        'drive_failure_confirmed', 'drive_replacement_assigned',
                        'drive_rebuild_completed'
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
    interval_seconds     INTEGER,
    max_duration_seconds INTEGER,
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
    priority    INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    last_backup TEXT,
    last_integrity_check TEXT,
    last_integrity_status TEXT,
    last_error  TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE db_backup_settings (
    id                         INTEGER PRIMARY KEY CHECK(id = 1),
    backup_enabled             INTEGER NOT NULL DEFAULT 0,
    backup_interval_seconds    INTEGER NOT NULL DEFAULT 86400,
    integrity_enabled          INTEGER NOT NULL DEFAULT 0,
    integrity_interval_seconds INTEGER NOT NULL DEFAULT 86400,
    last_backup_run            TEXT,
    last_integrity_run         TEXT,
    updated_at                 TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Sync queue global settings (single-row)
CREATE TABLE sync_settings (
    id           INTEGER PRIMARY KEY CHECK(id = 1),
    queue_paused INTEGER NOT NULL DEFAULT 0
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

### Queue-first mirroring with explicit immediate actions

Tracking files and folder scans enqueue deduplicated `mirror` work when the standby slot can accept sync. Immediate mirroring is an explicit action (`files mirror`, `POST /files/{id}/mirror`, `folders mirror`, `POST /folders/{id}/mirror`) and reconciles pending mirror-queue rows after success.

### Sync queue processing is parallel across drive pairs, sequential within one

`sync_queue::process_all_pending` fetches a page of pending items together with the `drive_pair_id` of each item's tracked file (`Repository::list_pending_sync_queue_with_drive_pair`) and groups them by drive pair. That fetch is fair across pairs — it ranks each pair's own items by queue id and takes the lowest ranks first across every pending pair (a round-robin ordering), rather than a single `ORDER BY id LIMIT` that would let one pair's deep backlog fill the whole page and starve every other pair until it drains. Per-pair queue order is preserved. `process_all_pending` then spawns one worker thread per drive pair (via `std::thread::scope`) so that, for example, two large files queued on two different mirror drives copy at the same time instead of one full transfer blocking the other. Items belonging to the *same* drive pair are still processed strictly one at a time, in queue order, on that pair's single worker thread — this avoids two transfers contending for the same physical disk's I/O bandwidth. Each worker independently re-checks the `sync_settings.queue_paused` flag (failing closed — treating a read error as paused rather than letting a transfer start unverified) and the scheduler `stop_by` deadline before every item, and the outer loop re-checks both before fetching the next page, so pausing or hitting a time budget still halts new work promptly across all drive pairs.

Only one invocation of `process_all_pending` may drain the queue at a time, enforced by a database-backed lock (`sync_settings.processing_locked_at`), not just an in-process flag. This matters because the scheduler, the CLI, and the API's background worker can all call `process_all_pending` directly, including from separate OS processes sharing the same database file. Without a cross-process lock, a second invocation's unconditional `requeue_in_progress_sync_queue()` could reset an `in_progress` row that a first invocation's worker is still actively transferring, letting two workers write to the same drive pair concurrently. `try_acquire_sync_processing_lock` atomically claims the lock (relying on SQLite serializing the underlying `UPDATE`) and a second, concurrent call is a no-op returning `Ok(0)`. The lock is refreshed each pass and released via an RAII guard on return (including early returns and panics); a lock left behind by a crashed process is reclaimed once it's older than a staleness window. The lock row also records the holder's OS PID (`sync_settings.processing_locked_by_pid`); on Linux, `try_acquire_sync_processing_lock` checks `/proc/<pid>` and reclaims the lock immediately if that PID is no longer running (e.g. `SIGKILL`'d), rather than waiting out the time-based staleness window — the time-based check remains the fallback when liveness can't be verified (non-Linux hosts, or a holder on a different machine sharing the database).

### Full integrity checks are asynchronous and persisted

Batch integrity checking is modeled as persisted runs (`integrity_runs`) with incremental per-file results (`integrity_run_results`). The worker processes tracked files in pages so frontends can poll progress and issue rows without waiting for the full dataset. Only one active run is allowed at a time, and stop behavior is cooperative via `stop_requested`. If `run_sync` returns an error partway through processing, the run is marked `failed` and `active_workers` is reset to 0 so the single-active-run guard does not become permanently locked.

### Direct and folder provenance are mutually exclusive

`tracked_direct` and `tracked_via_folder` are kept mutually exclusive in runtime behavior. If both are present in legacy rows, migration and recomputation normalize to direct tracking (`tracked_direct=1`, `tracked_via_folder=0`).

### Effective virtual paths are derived for folder-origin files

Tracking listings and virtual-path tree queries compute an effective file virtual path as:

1. explicit file `virtual_path`, otherwise
2. closest matching tracked folder virtual path + relative suffix.

This keeps filtering and left-pane tree counts consistent for files originating from tracked folders.

### Virtual paths are symlinks on disk

The virtual path system does not intercept file I/O. Instead it creates regular symlinks directly at the exact absolute virtual paths stored in the database. This makes virtual paths transparent to any tool that can follow symlinks, requires no kernel module or FUSE, and is straightforward to regenerate from the database state. During drive failover, the symlink target is refreshed from the pair's `active_role`, so future opens move to the surviving side.

### Drive failover is stateful, not path-swapping in place

Each drive pair now tracks `primary_state`, `secondary_state`, and `active_role`. Planned replacements move a slot through `active -> quiescing -> failed -> rebuilding -> active`. Unexpected loss of the active root can trigger emergency failover to the healthy side. The logical pair and relative paths stay stable; only the operational source/target root changes.

### Web path pickers browse absolute host paths, but tracking stays drive-relative

The web UI now opens a server-side filesystem browser for selecting files and directories. That browser exposes absolute host paths for navigation, but tracked files and tracked folders are still stored as paths relative to the selected drive pair's active root. Keeping storage relative preserves failover, mirroring, and rebuild behavior even when the active role changes.

### r2d2 connection pool for SQLite

Even though SQLite serialises writes, using a pool with `r2d2_sqlite` allows read operations across actix-web worker threads without acquiring a global lock.

### Background task scheduling via OS threads

Background tasks (scheduled sync, scheduled integrity check) run in dedicated OS threads spawned by the `Scheduler`. Each thread loops: **sleep** for the configured interval first (checked in 100 ms increments so the `stop_flag` is responsive), then run the task, then persist `last_run` / `next_run` in the schedule config row. Tasks can also be triggered on demand via `run_task` without any scheduler involvement. Scheduled integrity executions are persisted through the same run-service path as API/CLI starts.

At service startup, `cleanup_stale_integrity_runs()` is called before the scheduler reloads. Any run left in `running` or `stopping` state from a previous (crashed/killed) process is transitioned to `failed`, preventing a permanent single-active-run deadlock on restart.

When a schedule has `max_duration_seconds` configured, the scheduler thread computes a deadline (`Instant::now() + Duration::from_secs(max_duration_seconds)`) before invoking the task. This deadline is passed as `Option<Instant>` through `run_task` → `process_all_pending` / `process_run`. Each function checks the deadline before processing the next item and exits early if time has elapsed, leaving remaining work for the next scheduled run. Setting `max_duration_seconds` to `null` disables the cap.

### Sync queue pause/resume

The `sync_settings` database table holds a single-row `queue_paused` flag (integer 0/1, default 0). When the queue is paused, `process_all_pending` stops dequeuing further items — no new items start and the queue state is unchanged. This check is per-item, so an in-flight item already being processed on any drive pair's worker thread is not interrupted. The flag is toggled via `POST /sync/pause` and `POST /sync/resume` (API) or `bitprotector sync pause` / `bitprotector sync resume` (CLI).

### Realtime sync queue via SSE

The frontend receives live queue state over a Server-Sent Events stream rather than polling the REST endpoint repeatedly.

**Backend (`SyncEventBus`, `src/core/sync_queue.rs`)**

A `SyncEventBus` is created at server startup (in `run_server`) and registered as `actix_web::web::Data`. It wraps a `tokio::sync::broadcast::Sender<SyncSummary>` plus two atomics:

- `processing_active: Arc<AtomicBool>` — `true` while a background worker is running.
- `revision: Arc<AtomicU64>` — monotonically increasing counter incremented on every publish.

`SyncSummary` carries queue counts (total, pending, in_progress, completed, failed, active), pause state, processing state, aggregated folder-scan progress (scanning, scan_total_files, scan_scanned_files, scan_active_folders), and folder-delete activity (deleting, delete_active_folders). Folding folder operation progress into the queue summary eliminates the separate `/folders` poll.

Every mutating route handler (`add_queue_item`, `resolve_queue_item`, `clear_completed_queue`, `pause_queue`, `resume_queue`) calls `bus.publish_snapshot(&repo)` after its DB mutation. `process_item` inside `process_all_pending` calls `publish_snapshot` twice per item (once when marking `in_progress`, once when marking `completed`/`failed`), giving granular progress updates.

**`GET /sync/queue/stream`** builds an SSE response: an initial `once` stream (the current snapshot) chained with an `unfold` stream (new broadcast receiver, blocking on `recv` until the next event), serialised as `data: <json>\n\n` lines.

**`POST /sync/process`** now returns `202 Accepted` immediately and calls `process_all_pending_async`. An `AtomicBool` idempotency guard ensures only one background worker runs at a time; further calls while a worker is active are safe no-ops.

**Frontend**

`frontend/src/api/sync-stream.ts` opens the SSE stream with a native `fetch` call (Bearer token in header). It reads the response body as a `ReadableStream`, splits lines on `\n`, parses `data:` lines as `SyncSummary` JSON, and calls the provided `onSummary` callback. Automatic reconnect with exponential back-off (1 s → 2 s → … → 30 s) is built in. A `401` response triggers logout.

`frontend/src/stores/sync-store.ts` holds `summary: SyncSummary | null` as the live source of truth for all queue counts, paused state, processing state, and scan state. When `summary.revision` changes, the store issues one `fetchItems` call (with stale-response guard) to refresh the item list for the current filter/page. Counts displayed in tab labels and indicators are always derived from `summary`, so they update as soon as the SSE event arrives.

`frontend/src/pages/SyncQueuePage.tsx` starts the SSE stream on mount (`useEffect` → `openSyncStream`) and stops it on unmount. While `summary.scanning` is true, interactive queue controls are disabled (`Process Queue`, `Pause/Resume Queue`, filter tabs, and `Next`) and re-enable immediately when scanning becomes false. The "Processing…" button label and indicator are driven by `summary.processing_active`. The "Updating… N / M files" indicator is driven by `summary.scanning`.

`frontend/src/pages/TrackingWorkspacePage.tsx` also consumes the same stream. It re-fetches tracked folders on `summary.revision` changes and derives per-folder scan/mirror/delete UI state (`scanning`, `mirroring`, `deleting`) from those folder rows rather than polling `/folders/{id}/scan/active` and `/folders/{id}/mirror/active`.



### Integrity run file ordering

`integrity_runs.rs` always retrieves tracked files ordered by `last_integrity_check_at ASC NULLS FIRST`. Files that have never been checked are prioritised, followed by files checked longest ago. This ensures every file receives periodic coverage even when runs are cut short by a `max_duration_seconds` deadline or a cooperative stop request.
