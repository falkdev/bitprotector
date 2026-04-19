# Frontend Implementation Plan

## Distributed File Mirror and Integrity Protection System

> Historical planning note: sections that mention `symlink_base`, `auto_virtual_path`, `default_virtual_base`, or hidden virtual-path roots predate the literal virtual-path overhaul. The current behavior is documented in `README.md`, `docs/API.md`, `docs/ARCHITECTURE.md`, and `docs/CONFIGURATION.md`.

---

## 1. Architecture Overview

### 1.1 Technology Stack

| Component            | Technology                        |
| --- | --- |
| Framework            | React 18+                         |
| Language             | TypeScript                        |
| Build Tool           | Vite                              |
| State Management     | Zustand                           |
| Routing              | React Router v6                   |
| HTTP Client          | Axios                             |
| UI Components        | Shadcn/ui (Radix + Tailwind CSS)  |
| File Browser         | Custom component (tree + grid)    |
| Icons                | Lucide React                      |
| Forms                | React Hook Form + Zod validation  |
| Notifications        | Sonner (toast notifications)      |
| Testing              | Vitest + React Testing Library    |
| E2E Testing          | Playwright                        |
| Linting              | ESLint + Prettier                 |

### 1.2 High-Level Architecture

```text
┌─────────────────────────────────────────────┐
│                React Frontend               │
├─────────────────────────────────────────────┤
│  Pages / Views                              │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │  Login   │ │  File    │ │  Dashboard  │ │
│  │  Page    │ │  Browser │ │  (Status)   │ │
│  ├──────────┤ ├──────────┤ ├─────────────┤ │
│  │  Drive   │ │ Integrity│ │  Sync Queue │ │
│  │  Config  │ │  View    │ │  View       │ │
│  ├──────────┤ ├──────────┤ ├─────────────┤ │
│  │  Folder  │ │  Event   │ │  Scheduler  │ │
│  │  Config  │ │  Logs    │ │  Config     │ │
│  ├──────────┤ ├──────────┤ ├─────────────┤ │
│  │  DB      │ │ Virtual  │ │  Settings   │ │
│  │  Backups │ │ Path Mgr │ │             │ │
│  └──────────┘ └──────────┘ └─────────────┘ │
├─────────────────────────────────────────────┤
│  Shared Layer                               │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │  API     │ │  Auth    │ │  Zustand    │ │
│  │  Client  │ │  Context │ │  Stores     │ │
│  └──────────┘ └──────────┘ └─────────────┘ │
├─────────────────────────────────────────────┤
│  HTTPS  ←→  Backend REST API (v1)          │
│             + Static File Serving           │
└─────────────────────────────────────────────┘

```

### 1.3 Static File Serving

The production frontend is served directly by the **actix-web backend** using the `actix-files` crate — no separate web server (nginx, caddy, etc.) is used.

**Rationale:**

- Preserves the single-binary, single-process design principle of the project.
- No additional memory overhead or process management on resource-constrained hardware.
- The backend already terminates TLS via rustls on port `8443`; static files are served over the same listener with no extra TLS configuration.
- Frontend and API share the same origin (`https://<host>:8443`), so CORS headers are unnecessary.
- Simplifies `.deb` packaging to a single systemd service with no nginx dependency.

**How it works in `src/api/server.rs`:**

1. All `/api/v1/` routes are mounted first and take precedence.
2. `actix_files::Files::new("/", "/var/lib/bitprotector/frontend").index_file("index.html")` is mounted as a fallback to serve the Vite `dist/` output.
3. A catch-all `GET /{tail:.*}` handler returns `index.html` for any unmatched path, enabling React Router v6 client-side navigation (e.g. a direct browser request to `/drives` still receives `index.html`).

**Deployment path:** The `.deb` package installs the Vite production build to `/var/lib/bitprotector/frontend/`.

### 1.4 Project Structure

```text
frontend/
├── index.html
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── playwright.config.ts
├── vitest.config.ts
├── public/
│   └── favicon.ico
├── src/
│   ├── main.tsx                     # App entry point
│   ├── App.tsx                      # Root component + router
│   ├── api/
│   │   ├── client.ts               # Axios instance + interceptors
│   │   ├── auth.ts                  # Auth API calls (login, validate)
│   │   ├── drives.ts               # Drive pair CRUD + replacement workflow
│   │   ├── files.ts                # File tracking + mirror API calls
│   │   ├── virtual-paths.ts        # Virtual path set/remove/tree
│   │   ├── folders.ts              # Tracked folder CRUD + scan
│   │   ├── integrity.ts            # Integrity check (single + batch)
│   │   ├── sync.ts                 # Sync queue + process + run task
│   │   ├── scheduler.ts            # Scheduler schedule CRUD
│   │   ├── logs.ts                 # Event log listing
│   │   ├── database.ts             # Database backup config CRUD + run
│   │   └── status.ts               # System status API call
│   ├── stores/
│   │   ├── auth-store.ts           # Auth state (token, user)
│   │   ├── drives-store.ts         # Drive pairs state
│   │   ├── files-store.ts          # Tracked files state
│   │   ├── sync-store.ts           # Sync queue state
│   │   ├── logs-store.ts           # Event logs state
│   │   └── status-store.ts         # System status state
│   ├── pages/
│   │   ├── LoginPage.tsx
│   │   ├── DashboardPage.tsx
│   │   ├── FileBrowserPage.tsx
│   │   ├── DrivesPage.tsx
│   │   ├── FoldersPage.tsx
│   │   ├── IntegrityPage.tsx
│   │   ├── SyncQueuePage.tsx
│   │   ├── SchedulerPage.tsx
│   │   ├── LogsPage.tsx
│   │   └── DatabaseBackupsPage.tsx
│   ├── components/
│   │   ├── layout/
│   │   │   ├── AppLayout.tsx        # Main layout with sidebar
│   │   │   ├── Sidebar.tsx          # Navigation sidebar
│   │   │   ├── Header.tsx           # Top header bar
│   │   │   └── ProtectedRoute.tsx   # Auth guard component
│   │   ├── file-browser/
│   │   │   ├── FileBrowser.tsx      # Main file browser component
│   │   │   ├── FileTree.tsx         # Tree sidebar (virtual paths)
│   │   │   ├── FileGrid.tsx         # File grid/list view
│   │   │   ├── FileRow.tsx          # Single file row display
│   │   │   ├── FileDetails.tsx      # File detail panel
│   │   │   ├── BreadcrumbNav.tsx    # Path breadcrumb navigation
│   │   │   └── FileActions.tsx      # File context actions
│   │   ├── drives/
│   │   │   ├── DriveList.tsx
│   │   │   ├── DriveForm.tsx
│   │   │   ├── DriveCard.tsx
│   │   │   └── ReplacementWorkflow.tsx
│   │   ├── folders/
│   │   │   ├── FolderList.tsx
│   │   │   └── FolderForm.tsx
│   │   ├── integrity/
│   │   │   ├── IntegrityStatus.tsx
│   │   │   ├── IntegrityResults.tsx
│   │   │   └── CorruptionAlert.tsx
│   │   ├── sync/
│   │   │   ├── SyncQueueTable.tsx
│   │   │   ├── SyncQueueItem.tsx
│   │   │   └── ResolveDialog.tsx
│   │   ├── scheduler/
│   │   │   ├── ScheduleList.tsx
│   │   │   └── ScheduleForm.tsx
│   │   ├── logs/
│   │   │   ├── LogTable.tsx
│   │   │   └── LogFilter.tsx
│   │   ├── dashboard/
│   │   │   ├── StatusOverview.tsx
│   │   │   ├── QuickActions.tsx
│   │   │   └── RecentActivity.tsx
│   │   └── shared/
│   │       ├── DataTable.tsx        # Reusable sortable/paginated table
│   │       ├── ConfirmDialog.tsx    # Confirmation modal
│   │       ├── EmptyState.tsx       # Empty state placeholder
│   │       ├── LoadingSpinner.tsx
│   │       ├── ErrorBoundary.tsx
│   │       └── Pagination.tsx
│   ├── hooks/
│   │   ├── useAuth.ts
│   │   ├── usePagination.ts
│   │   ├── usePolling.ts           # Poll for job status updates
│   │   └── useDebounce.ts
│   ├── types/
│   │   ├── api.ts                   # API response/error types
│   │   ├── auth.ts                  # Login response (token, username, expires_at)
│   │   ├── drive.ts                 # Drive pair with states + replacement types
│   │   ├── file.ts                  # Tracked file (relative_path, checksum, etc.)
│   │   ├── folder.ts                # Tracked folder + scan result
│   │   ├── virtual-path.ts          # Virtual path request/response types
│   │   ├── integrity.ts             # Integrity result + all status values
│   │   ├── sync.ts                  # Sync queue item + resolve request
│   │   ├── scheduler.ts             # Schedule config types
│   │   ├── log.ts                   # Event log entry + event types
│   │   └── status.ts                # System status (with degraded/rebuilding fields)
│   ├── lib/
│   │   ├── utils.ts                 # General utilities
│   │   └── format.ts               # Date, size, path formatters
│   └── styles/
│       └── globals.css              # Tailwind base styles
├── tests/
│   ├── unit/                        # Vitest unit tests
│   │   ├── api/
│   │   ├── stores/
│   │   ├── hooks/
│   │   └── lib/
│   ├── component/                   # Component render tests
│   │   ├── file-browser/
│   │   ├── drives/
│   │   ├── integrity/
│   │   ├── sync/
│   │   ├── logs/
│   │   └── shared/
│   └── e2e/                         # Playwright E2E tests
│       ├── auth.spec.ts
│       ├── file-browser.spec.ts
│       ├── drives.spec.ts
│       ├── integrity.spec.ts
│       ├── sync-queue.spec.ts
│       ├── scheduler.spec.ts
│       ├── logs.spec.ts
│       └── database-backups.spec.ts
└── .env.example

```

---

## 2. Page & Component Specifications

### 2.1 Login Page (`LoginPage.tsx`)

**Purpose:** Authenticate users via local system accounts.

**UI Elements:**

- Application logo and title
- Username input field
- Password input field
- "Log In" submit button
- Error message display area

**Behavior:**

- POST to `/api/v1/auth/login`
- Response includes `token`, `username`, and `expires_at` (ISO 8601 timestamp)
- On success: store JWT in auth store, redirect to Dashboard
- On failure: display error message
- Redirect to Dashboard if already authenticated

---

### 2.2 Dashboard Page (`DashboardPage.tsx`)

**Purpose:** Provide an at-a-glance system overview.

**Components:**

- `StatusOverview` — displays key metrics:
  - `files_tracked` — total tracked files
  - `files_mirrored` — mirrored files count
  - `pending_sync` — pending sync items
  - `integrity_issues` — integrity issues count
  - `drive_pairs` — number of drive pairs
  - `degraded_pairs` — pairs with a failed or unavailable slot
  - `active_secondary_pairs` — pairs running from the secondary side
  - `rebuilding_pairs` — pairs with a slot being rebuilt
  - `quiescing_pairs` — pairs with a slot being quiesced for replacement
- `QuickActions` — buttons for common operations:
  - Run integrity check (all files)
  - Process sync queue
  - Trigger database backup
- `RecentActivity` — latest 10 event log entries

**Data Source:** `GET /api/v1/status`, `GET /api/v1/logs?per_page=10`

---

### 2.3 File Browser Page (`FileBrowserPage.tsx`)

**Purpose:** Primary interface — file browser style view of all tracked files (Requirement 29).

**Layout:**

```text
┌──────────────────────────────────────────────┐
│  BreadcrumbNav                               │
├──────────┬───────────────────────────────────┤
│          │                                   │
│ FileTree │         FileGrid                  │
│ (virtual │  ┌──────┬──────┬──────┬────────┐  │
│  path    │  │ Name │ Size │Status│ Actions│  │
│  tree)   │  ├──────┼──────┼──────┼────────┤  │
│          │  │ ...  │ ...  │  ✓   │  ···   │  │
│          │  │ ...  │ ...  │  ⚠   │  ···   │  │
│          │  └──────┴──────┴──────┴────────┘  │
│          │                                   │
│          ├───────────────────────────────────┤
│          │  FileDetails (selected file)      │
│          │  - Checksum, paths, drive pair    │
│          │  - Last verified, mirror status   │
└──────────┴───────────────────────────────────┘

```

**Components:**

- `FileTree` — collapsible tree showing virtual path hierarchy
  - Clicking a node navigates into that virtual folder
  - Shows folder icons and file count badges
- `BreadcrumbNav` — current virtual path as clickable breadcrumbs
- `FileGrid` — sortable table/grid of files in the current virtual folder
  - Columns: Name, Size, Checksum (truncated), Mirror status (`is_mirrored`), Last verified, Actions
  - Toggle between grid and list view
  - Multi-select support for bulk operations
- `FileDetails` — detail panel for selected file showing:
  - `relative_path` — path relative to the drive pair root
  - Resolved active-side path (computed from drive pair's `active_role`)
  - Resolved standby-side path (computed from the other slot)
  - `checksum` — BLAKE3 hex hash
  - Drive pair name and ID
  - `last_verified` timestamp
  - `is_mirrored` boolean
  - `file_size` in human-readable format
- `FileActions` — per-file context menu:
  - Verify integrity (`POST /integrity/check/{id}`)
  - Mirror to standby (`POST /files/{id}/mirror`)
  - Edit virtual path (`PUT /virtual-paths/{file_id}`)
  - Remove tracking (`DELETE /files/{id}`)
  - View logs for this file (`GET /logs?file_id={id}`)

**Data Sources:** `GET /api/v1/files`, `GET /api/v1/virtual-paths`, `GET /api/v1/drives`

> **Note:** Files use `relative_path` (relative to the drive pair root), not absolute paths. The frontend must resolve the full path by joining the drive pair's active/standby paths with the `relative_path`.

---

### 2.4 Drives Page (`DrivesPage.tsx`)

**Purpose:** Manage drive pairs, view drive health, and perform replacement workflows.

**Components:**

- `DriveList` — cards or table showing all drive pairs
  - Each card shows: name, primary path, secondary path, `primary_state`, `secondary_state`, `active_role`
  - Color-coded state badges: active (green), quiescing (yellow), failed (red), rebuilding (blue)
- `DriveForm` — modal form for creating/editing drive pairs
  - Fields: name, primary path, secondary path
  - Validation: paths must be non-empty and different
- `DriveCard` — individual drive pair display with edit/delete actions
- `ReplacementWorkflow` — UI for the drive replacement lifecycle:
  - "Mark for Replacement" button → `POST /drives/{id}/replacement/mark` (with role selector)
  - "Cancel Replacement" button → `POST /drives/{id}/replacement/cancel`
  - "Confirm Failure" button → `POST /drives/{id}/replacement/confirm`
  - "Assign Replacement Drive" form → `POST /drives/{id}/replacement/assign` (role, new_path, skip_validation)
  - State machine visualization: `active → quiescing → failed → rebuilding → active`

**Data Source:** `GET /api/v1/drives`, `POST/PUT/DELETE /api/v1/drives`, `POST /api/v1/drives/{id}/replacement/*`

---

### 2.5 Folders Page (`FoldersPage.tsx`)

**Purpose:** Manage tracked folders.

**Components:**

- `FolderList` — table showing tracked folders
  - Columns: Path, Drive Pair, Auto Virtual Path, Default Virtual Base, Created At
  - "Scan" button per folder → `POST /folders/{id}/scan` (discovers new files, detects changes)
- `FolderForm` — modal form for adding tracked folders
  - Fields: drive pair (select), folder path, auto virtual path (toggle), default virtual base
  - Note: There is no PUT endpoint — folders cannot be edited after creation, only deleted and re-created

**Data Source:** `GET /api/v1/folders`, `POST/DELETE /api/v1/folders`, `POST /api/v1/folders/{id}/scan`

---

### 2.6 Integrity Page (`IntegrityPage.tsx`)

**Purpose:** View integrity check results and trigger checks.

**Components:**

- `IntegrityStatus` — summary of the last batch check results
  - Counts by status: ok, master_corrupted, mirror_corrupted, both_corrupted, master_missing, mirror_missing, primary_drive_unavailable, secondary_drive_unavailable
  - Number auto-recovered
- `IntegrityResults` — detailed results table
  - Columns: File ID, Relative Path, Status, Recovered
  - Color-coded status badges
  - Sortable and filterable
- `CorruptionAlert` — prominent alert banner when both-corrupted or missing files exist
- "Check Single File" — select a file and run `POST /integrity/check/{id}?recover=true`
  - Returns: `file_id`, `status`, `master_valid`, `mirror_valid`, `recovered`
- "Check All Files" button → `GET /integrity/check-all?recover=true`
  - Optional `drive_id` filter to limit to one drive pair
  - Returns: `{ "results": [{ "file_id", "status", "recovered" }] }`

**Data Source:** `POST /api/v1/integrity/check/{id}`, `GET /api/v1/integrity/check-all`

**Note:** Both endpoints are synchronous — the response contains the full results. For batch checks on large file sets, the frontend should display a loading indicator during the request. There is no job ID or polling mechanism.

**Valid `status` values:** `ok`, `master_corrupted`, `mirror_corrupted`, `both_corrupted`, `master_missing`, `mirror_missing`, `primary_drive_unavailable`, `secondary_drive_unavailable`

---

### 2.7 Sync Queue Page (`SyncQueuePage.tsx`)

**Purpose:** View and manage the sync queue.

**Components:**

- `SyncQueueTable` — filterable, paginated table of queue items
  - Columns: File ID, Action, Status, Error Message, Created, Completed
  - Filter by status: `pending`, `in_progress`, `completed`, `failed`
  - Action values: `mirror`, `restore_master`, `restore_mirror`, `verify`, `user_action_required`
- `SyncQueueItem` — row with action button for items requiring resolution
- `ResolveDialog` — modal for resolving `user_action_required` items
  - Resolution options: `keep_master`, `keep_mirror`, `provide_new`
  - File path input field (required when resolution is `provide_new`, via `new_file_path`)
- "Process Queue" button → `POST /api/v1/sync/process` (processes all pending items)
- "Run Task" buttons → `POST /api/v1/sync/run/{task}` where `{task}` is `sync` or `integrity-check`
- "Add Queue Item" form → `POST /api/v1/sync/queue` with `tracked_file_id` and `action`

**Data Source:** `GET /api/v1/sync/queue`, `POST /api/v1/sync/process`, `POST /api/v1/sync/run/{task}`, `POST /api/v1/sync/queue/{id}/resolve`

---

### 2.9 Scheduler Page (`SchedulerPage.tsx`)

**Purpose:** Configure sync and integrity check schedules.

**Components:**

- `ScheduleList` — table of configured schedules
  - Columns: Task Type, Cron Expression, Interval (seconds), Enabled, Last Run, Next Run
  - Enable/disable toggle per schedule (via `PUT /scheduler/schedules/{id}` with `{ enabled: bool }`)
- `ScheduleForm` — modal for creating/editing schedules
  - Fields: `task_type` (select: `sync` / `integrity_check`), `cron_expr` (optional), `interval_seconds` (optional), `enabled` toggle
  - At least one of `cron_expr` or `interval_seconds` must be provided
  - Cron expression helper/validation

**Data Source:** `GET /api/v1/scheduler/schedules` (returns `{ "schedules": [...] }`), `POST/PUT/DELETE /api/v1/scheduler/schedules/{id}`

> **Note:** Schedule changes automatically reload the background scheduler.

---

### 2.10 Logs Page (`LogsPage.tsx`)

**Purpose:** View and filter event logs.

**Components:**

- `LogFilter` — filter bar:
  - Event type dropdown (multi-select)
  - File ID / path search
  - Date range picker (from/to)
- `LogTable` — paginated table of log entries
  - Columns: Timestamp, Event Type, File, Message, Details
  - Expandable rows for detail content
  - Color-coded event types (green=pass, red=fail, yellow=warning)

**Data Source:** `GET /api/v1/logs`

---

### 2.11 Database Backups Page (`DatabaseBackupsPage.tsx`)

**Purpose:** Manage database backup destinations and trigger backups.

**UI Elements:**

- Table of backup configurations: `backup_path`, `drive_label`, `max_copies`, `enabled`, `last_backup`, `created_at`
- Add backup destinations (`POST /database/backups` — fields: `backup_path` required; `drive_label`, `max_copies` (default 5), `enabled` (default true) optional)
- Edit backup destinations (`PUT /database/backups/{id}` — only `max_copies` and `enabled` can be updated)
- Delete backup destinations (`DELETE /database/backups/{id}`)
- "Run Backup Now" button → `POST /api/v1/database/backups/run?db_path=<path>`
  - **Requires** `db_path` query parameter (absolute path to the live database file)
  - The frontend should store or configure the database path (from settings or env)
  - Returns per-destination results: `[{ backup_config_id, backup_path, status, error }]`
- Results display after backup execution

**Data Source:** `GET /api/v1/database/backups`, `POST/PUT/DELETE /api/v1/database/backups`, `POST /api/v1/database/backups/run?db_path=...`

---

## 3. Shared Infrastructure

### 3.1 API Client (`api/client.ts`)

- Axios instance configured with base URL from environment variable
- Request interceptor: attach JWT bearer token from auth store
- Response interceptor: on 401 → clear auth state → redirect to login
- Typed response wrappers for all endpoints

### 3.2 Authentication Flow

1. User submits credentials on Login page
2. POST `/api/v1/auth/login` → receive `{ token, username, expires_at }`
3. Store JWT and `expires_at` in Zustand auth store (persisted to sessionStorage)
4. All subsequent requests include `Authorization: Bearer <token>`
5. On page load, validate token via `GET /api/v1/auth/validate` → returns `{ username, valid }`
6. On token expiry (check `expires_at` client-side) or 401 response → clear auth state → redirect to login

### 3.3 Protected Routing

- `ProtectedRoute` component wraps all authenticated pages
- Checks auth store for valid token
- Redirects to `/login` if not authenticated
- Renders children if authenticated

### 3.4 Polling Hook (`usePolling`)

Used for monitoring changing state (sync queue progress, system status refresh):

- Accepts a fetch function and interval (default 5s)
- Calls fetch function on interval
- Returns current data, loading state, error state
- Can be paused/resumed manually

> **Note:** Integrity checks are synchronous — the API returns full results in the response. No polling is needed for integrity operations. The polling hook is useful for refreshing the sync queue status, system status dashboard, and similar views.

---

## 4. Implementation Milestones

### Milestone 1: Project Setup & Scaffolding

**Objective:** Initialize React project with all tooling configured.

**Steps:**

1. Create Vite + React + TypeScript project
2. Install and configure Tailwind CSS
3. Install and configure Shadcn/ui components
4. Configure ESLint + Prettier
5. Configure Vitest + React Testing Library
6. Configure Playwright
7. Set up project directory structure
8. Configure environment variables (`.env.example`)
9. Set up Axios API client with base configuration

**Tests:**

- Unit: Verify Axios client creates correct base URL
- Unit: Verify Axios interceptor attaches auth header
- Component: App renders without crashing

**Commit:** `feat: frontend project scaffolding with React, TypeScript, Tailwind, and test tooling`

---

### Milestone 2: Authentication & Layout

**Objective:** Implement login flow, auth state, and application shell.

**Steps:**

1. Implement TypeScript types for auth (`types/api.ts`, `types/auth.ts` — `LoginResponse` has `token`, `username`, `expires_at`)
2. Implement auth API client (`api/auth.ts` — `login()` and `validate()` calls)
3. Implement auth Zustand store (`stores/auth-store.ts` — persist token + expires_at to sessionStorage)
4. Implement `LoginPage.tsx` with form validation
5. Implement `AppLayout.tsx` with sidebar and header
6. Implement `Sidebar.tsx` with navigation links
7. Implement `Header.tsx` with user info and logout
8. Implement `ProtectedRoute.tsx` — validates token via `GET /auth/validate` on mount
9. Configure React Router with auth-guarded routes
10. Implement `useAuth` hook

**Tests:**

- Unit: Auth store — login sets token, logout clears state
- Unit: API client — auth interceptor attaches bearer token
- Unit: API client — 401 response clears auth and redirects
- Component: LoginPage — renders form, shows error on invalid login
- Component: LoginPage — successful login redirects to dashboard
- Component: ProtectedRoute — redirects unauthenticated users
- Component: Sidebar — renders all navigation links
- E2E: Full login → dashboard → logout flow

**Commit:** `feat: authentication flow with login page, auth guard, and app layout`

---

### Milestone 3: Dashboard

**Objective:** Implement the Dashboard page with system status overview.

**Steps:**

1. Implement status types (`types/status.ts` — fields: `files_tracked`, `files_mirrored`, `pending_sync`, `integrity_issues`, `drive_pairs`, `degraded_pairs`, `active_secondary_pairs`, `rebuilding_pairs`, `quiescing_pairs`)
2. Implement status API client (`api/status.ts`)
3. Implement status store (`stores/status-store.ts`)
4. Implement logs API client (partial — recent entries) (`api/logs.ts`)
5. Implement `DashboardPage.tsx`
6. Implement `StatusOverview.tsx` — status metric cards including drive health indicators
7. Implement `QuickActions.tsx` — action buttons (integrity check-all, sync process, database backup run)
8. Implement `RecentActivity.tsx` — recent log entries list

**Tests:**

- Unit: Status store fetches and stores data correctly
- Component: StatusOverview — renders all metric values including drive health fields
- Component: QuickActions — buttons trigger correct API calls
- Component: RecentActivity — renders log entries
- Component: DashboardPage — displays loading state, then data
- E2E: Dashboard loads and displays live system status

**Commit:** `feat: dashboard page with system status overview and quick actions`

---

### Milestone 4: Drive Pair Management

**Objective:** Implement drive pair configuration UI with drive health and replacement workflow.

**Steps:**

1. Implement drive types (`types/drive.ts` — include `primary_state`, `secondary_state`, `active_role` fields; replacement request types)
2. Implement drives API client (`api/drives.ts` — CRUD + mark/cancel/confirm/assign replacement)
3. Implement drives store (`stores/drives-store.ts`)
4. Implement `DrivesPage.tsx`
5. Implement `DriveList.tsx` — with state badges and health indicators
6. Implement `DriveCard.tsx` — show all state fields with color coding
7. Implement `DriveForm.tsx` with validation
8. Implement `ReplacementWorkflow.tsx` — step-by-step replacement UI
9. Implement `ConfirmDialog.tsx` for delete confirmation

**Tests:**

- Unit: Drives store — CRUD operations update state correctly
- Component: DriveList — renders all drive pairs with correct state badges
- Component: DriveForm — validates required fields, path uniqueness
- Component: DriveCard — displays info including states, edit/delete buttons work
- Component: ReplacementWorkflow — mark/cancel/confirm/assign actions work
- Component: ConfirmDialog — confirms or cancels action
- E2E: Create, edit, and delete a drive pair
- E2E: Walk through a planned drive replacement workflow

**Commit:** `feat: drive pair management page with CRUD and replacement workflow`

---

### Milestone 5: File Browser — Core

**Objective:** Implement the primary file browser interface.

**Steps:**

1. Implement file types (`types/file.ts` — `id`, `drive_pair_id`, `relative_path`, `checksum`, `file_size`, `virtual_path`, `is_mirrored`, `last_verified`, `created_at`, `updated_at`; `types/virtual-path.ts`)
2. Implement files API client (`api/files.ts` — list with pagination, get, track, mirror, delete)
3. Implement virtual paths API client (`api/virtual-paths.ts` — set, remove, tree)
4. Implement files store (`stores/files-store.ts`)
6. Implement `FileBrowserPage.tsx`
7. Implement `FileTree.tsx` — virtual path tree sidebar (built from `virtual_path` fields on files)
8. Implement `BreadcrumbNav.tsx`
9. Implement `FileGrid.tsx` — sortable file table with pagination (API returns `{ files, total, page, per_page }`)
10. Implement `FileRow.tsx` — individual file row with status indicators
11. Implement `Pagination.tsx` shared component
12. Implement `DataTable.tsx` shared component

**Tests:**

- Unit: Files store — fetches, paginates, filters files
- Unit: Virtual paths store — builds tree structure from flat list
- Component: FileTree — renders tree, clicking navigates
- Component: BreadcrumbNav — renders path segments, clicking navigates
- Component: FileGrid — renders file table, sorting works
- Component: FileRow — displays name, size, checksum, status icon
- Component: Pagination — page navigation works
- E2E: Navigate virtual path tree, view file listing

**Commit:** `feat: file browser page with virtual path tree and file grid`

---

### Milestone 6: File Browser — Details & Actions

**Objective:** Implement file detail panel and file actions.

**Steps:**

1. Implement `FileDetails.tsx` — detail panel showing relative_path, resolved paths, checksum, drive pair, mirror status
2. Implement `FileActions.tsx` — context menu with actions
3. Wire "Verify Integrity" action → `POST /integrity/check/{id}?recover=true`
4. Wire "Mirror File" action → `POST /files/{id}/mirror`
5. Wire "Edit Virtual Path" → `PUT /virtual-paths/{file_id}`
6. Wire "Remove Tracking" → `DELETE /files/{id}` with confirmation dialog
7. Wire "View Logs" → navigate to logs page with `?file_id={id}` filter
8. Implement multi-select for bulk operations in FileGrid

**Tests:**

- Component: FileDetails — displays all file metadata
- Component: FileActions — menu renders, actions fire correct API calls
- Component: FileGrid — multi-select works, bulk action bar appears
- E2E: Select file → view details → verify integrity → see result
- E2E: Edit virtual path for a file

**Commit:** `feat: file browser details panel and file actions`

---

### Milestone 8: Tracked Folders

**Objective:** Implement tracked folder management UI.

**Steps:**

1. Implement folder types (`types/folder.ts` — `id`, `drive_pair_id`, `folder_path`, `auto_virtual_path`, `default_virtual_base`, `created_at`)
2. Implement folders API client (`api/folders.ts` — list, get, create, delete, scan)
3. Implement `FoldersPage.tsx`
4. Implement `FolderList.tsx` — with "Scan" button per folder
5. Implement `FolderForm.tsx` with drive pair selector and auto-virtual-path toggle (create only — no edit)
6. Implement scan results display (`POST /folders/{id}/scan` → `{ new_files, changed_files }`)

**Tests:**

- Unit: Folders API client — correct requests for create, delete, scan
- Component: FolderList — renders all tracked folders with scan buttons
- Component: FolderForm — validates and submits, drive pair selector works
- Component: Scan results — shows new/changed file counts
- E2E: Add tracked folder with auto-virtual-path, scan it, verify results

**Commit:** `feat: tracked folder management page with scan support`

---

### Milestone 9: Integrity Check & Sync Queue

**Objective:** Implement integrity checking UI and sync queue management.

**Steps:**

1. Implement integrity types (`types/integrity.ts` — all 8 status values, single/batch result types)
2. Implement integrity API client (`api/integrity.ts` — `checkFile(id, recover)`, `checkAll(driveId, recover)`)
3. Implement sync types (`types/sync.ts` — queue item, resolve request with `keep_master`/`keep_mirror`/`provide_new`)
4. Implement sync API client (`api/sync.ts` — list queue, add item, get item, resolve, process, run task)
5. Implement sync store (`stores/sync-store.ts`)
6. Implement `usePolling` hook (for sync queue refresh, not integrity polling)
7. Implement `IntegrityPage.tsx`
8. Implement `IntegrityStatus.tsx` — summary of last batch results (no progress bar — API is synchronous)
9. Implement `IntegrityResults.tsx` — results table with all status types
10. Implement `CorruptionAlert.tsx` — alert banner
11. Implement `SyncQueuePage.tsx`
12. Implement `SyncQueueTable.tsx` — filterable queue table
13. Implement `SyncQueueItem.tsx`
14. Implement `ResolveDialog.tsx` — resolution modal for `user_action_required` items

**Tests:**

- Unit: usePolling — polls at interval, can be paused/resumed
- Unit: Sync store — filters by status correctly
- Component: IntegrityStatus — shows loading during API call, summary after completion
- Component: IntegrityResults — renders result rows with correct status colors for all 8 statuses
- Component: CorruptionAlert — displays when both-corrupted or missing files exist
- Component: SyncQueueTable — filters by status, renders items correctly
- Component: ResolveDialog — options render (`keep_master`, `keep_mirror`, `provide_new`), submission sends correct payload
- E2E: Trigger integrity check-all → view results
- E2E: View sync queue → resolve a user_action_required item

**Commit:** `feat: integrity check and sync queue management pages`

---

### Milestone 10: Scheduler Configuration

**Objective:** Implement scheduler configuration UI.

**Steps:**

1. Implement scheduler types (`types/scheduler.ts` — `id`, `task_type`, `cron_expr`, `interval_seconds`, `enabled`, `last_run`, `next_run`, `created_at`, `updated_at`)
2. Implement scheduler API client (`api/scheduler.ts` — CRUD on `/scheduler/schedules`; response wraps list in `{ "schedules": [...] }`)
3. Implement `SchedulerPage.tsx`
4. Implement `ScheduleList.tsx` with enable/disable toggle
5. Implement `ScheduleForm.tsx` with cron expression input and interval option (at least one required)

**Tests:**

- Component: ScheduleList — renders schedules, toggle changes enabled state via PUT
- Component: ScheduleForm — validates that at least one of cron_expr or interval_seconds is provided
- Component: ScheduleForm — task_type restricted to `sync` or `integrity_check`
- E2E: Create schedule, toggle enable/disable, delete schedule

**Commit:** `feat: scheduler configuration page`

---

### Milestone 11: Event Logs

**Objective:** Implement event log viewing with filters.

**Steps:**

1. Implement log types (`types/log.ts` — event types: `file_created`, `file_edited`, `file_mirrored`, `integrity_pass`, `integrity_fail`, `recovery_success`, `recovery_fail`, `both_corrupted`, `change_detected`, `sync_completed`, `sync_failed`)
2. Implement logs API client (`api/logs.ts` — list with filters, get by id; response is a flat JSON array)
3. Implement logs store (`stores/logs-store.ts`)
4. Implement `LogsPage.tsx`
5. Implement `LogFilter.tsx` — event type multiselect, date range (from/to ISO 8601), file ID search
6. Implement `LogTable.tsx` — paginated (client-side or API `page`/`per_page`), expandable rows, color-coded types

**Tests:**

- Unit: Logs store — filtering builds correct query params
- Component: LogFilter — selecting filters updates query
- Component: LogTable — renders entries, expandable rows show details
- Component: LogTable — event types color-coded correctly
- E2E: Filter logs by type and date range, verify results

**Commit:** `feat: event log viewer with filtering and color-coded entries`

---

### Milestone 12: Database Backup Management

**Objective:** Implement database backup configuration and manual trigger.

**Steps:**

1. Implement database backup API client (`api/database.ts` — CRUD on `/database/backups`; `run` requires `db_path` query param)
2. Implement `DatabaseBackupsPage.tsx`
3. Implement backup config table with enable/disable and CRUD (only `max_copies` and `enabled` are editable via PUT)
4. Implement "Run Backup Now" with `db_path` parameter and per-destination results display

**Tests:**

- Component: Backup config table — renders configs, edit/delete work
- Component: Run backup — sends db_path, shows per-destination results after execution
- E2E: Add backup destination → run backup → view results

**Commit:** `feat: database backup management page`

---

### Milestone 13: Error Handling, Loading States & Polish

**Objective:** Ensure consistent UX across all pages.

**Steps:**

1. Implement `ErrorBoundary.tsx` — global error boundary
2. Implement `EmptyState.tsx` — empty state placeholders for all lists
3. Implement `LoadingSpinner.tsx` — consistent loading indicators
4. Add toast notifications (Sonner) for:
   - Successful operations (create, update, delete)
   - Error messages from API
   - Integrity check completion notifications
5. Responsive layout adjustments (sidebar collapse on mobile)
6. Keyboard navigation support in file browser
7. Accessibility audit (aria labels, focus management)

**Tests:**

- Component: ErrorBoundary — catches and displays errors
- Component: EmptyState — renders correct message per context
- Component: Toast notifications — appear for success/error
- E2E: Full workflow test — login → browse files → check integrity → view logs → logout

**Commit:** `feat: error handling, loading states, notifications, and UX polish`

---

### Milestone 14: Build & Packaging

**Objective:** Production build and integration with backend packaging.

**Steps:**

1. Configure Vite production build with `base: "/"` and output to `dist/`
2. Add `actix-files` crate to the backend `Cargo.toml`
3. In `src/api/server.rs`, mount static file serving after all API routes:
   - `actix_files::Files::new("/", "/var/lib/bitprotector/frontend").index_file("index.html")`
   - Add a catch-all `GET /{tail:.*}` handler returning `index.html` to support React Router v6 client-side routing
4. Verify all API routes at `/api/v1/` still resolve correctly (files mount must come last)
5. Verify production build works end-to-end with backend TLS on port `8443`
6. Include built `dist/` output in the `.deb` package, installed to `/var/lib/bitprotector/frontend/`

**Tests:**

- Unit: Production build completes without errors
- Integration: `GET /` returns `index.html` with correct content-type
- Integration: `GET /drives` (unmatched path) returns `index.html` (React Router fallback)
- Integration: `GET /api/v1/status` still resolves to the API handler, not static files
- E2E: Full E2E suite runs against production build served by actix-web
- Integration: Frontend served by backend, all pages functional over TLS

**Commit:** `feat: production build and packaging integration`

---

## 5. Testing Strategy

### 5.1 Test Levels

| Level      | Scope                              | Tools                        | Location             |
| --- | --- | --- | --- |
| Unit       | API clients, stores, hooks, utils   | Vitest                       | `tests/unit/`        |
| Component  | Individual React components         | Vitest + RTL                 | `tests/component/`   |
| E2E        | Full user workflows                 | Playwright                   | `tests/e2e/`         |

### 5.2 Test Conventions

- **TDD workflow:** Write failing test → implement component → pass → refactor
- API calls mocked in unit/component tests (MSW or Vitest mocks)
- E2E tests run against a real or mock backend
- Use `data-testid` attributes for stable E2E selectors
- Each component test verifies: renders correctly, handles user interaction, handles loading/error/empty states
- Snapshot tests only for stable layout components, not for dynamic content

### 5.3 Mocking Strategy

| Dependency        | Mock Approach                              |
| --- | --- |
| API responses     | MSW (Mock Service Worker) in unit/component tests |
| Auth state        | Direct Zustand store manipulation          |
| Router            | MemoryRouter wrapper in component tests    |
| Timers (polling)  | Vitest fake timers                         |

### 5.4 E2E Test Coverage

Each E2E spec covers a full user workflow:

| Spec File                  | Workflow                                              |
| --- | --- |
| `auth.spec.ts`             | Login, session persistence, token validation, logout  |
| `file-browser.spec.ts`     | Navigate tree, sort grid, select file, view details   |
| `drives.spec.ts`           | Create, edit, delete drive pair; replacement workflow  |
| `integrity.spec.ts`        | Trigger single + batch check, view results            |
| `sync-queue.spec.ts`       | View queue, filter, resolve user_action_required item |
| `scheduler.spec.ts`        | Create, toggle, delete schedule                       |
| `logs.spec.ts`             | Filter by type, date range, expand details            |
| `database-backups.spec.ts` | Add destination, trigger backup with db_path, results |

### 5.5 Test Coverage Targets

| Area                  | Minimum Coverage |
| --- | --- |
| API clients           | 95%             |
| Zustand stores        | 95%             |
| Custom hooks          | 90%             |
| Page components       | 85%             |
| Shared components     | 90%             |
| Feature components    | 85%             |
| Utility functions     | 100%            |

---

## 6. Environment Configuration

`.env.example`:

```env
# Backend API URL

VITE_API_BASE_URL=https://localhost:8443/api/v1

# Database path (used by the "Run Backup Now" action)

VITE_DB_PATH=/var/lib/bitprotector/bitprotector.db

# Development only — disable TLS verification

VITE_DEV_INSECURE=false

```

---

## 7. UI Design Guidelines

### 7.1 Design Principles

- **File browser as primary interface** — the main view users see and interact with most
- Clean, minimal design using Shadcn/ui components
- Consistent color coding for statuses:
  - Green: healthy / passed / mirrored / active
  - Yellow: pending / warning / changed / quiescing
  - Red: corrupted / failed / error / both_corrupted
  - Blue: rebuilding
  - Gray: unverified / inactive / missing

### 7.2 Layout

- Fixed sidebar navigation (collapsible)
- Top header with breadcrumbs and user menu
- Main content area with consistent padding
- Modal dialogs for forms and confirmations
- Toast notifications for operation feedback

### 7.3 Responsiveness

- Desktop-first design (primary use case)
- Sidebar collapses to icons on narrow viewports
- Tables become scrollable on small screens
- File browser tree hides behind a toggle on mobile
