# Frontend Implementation Plan
## Distributed File Mirror and Integrity Protection System

---

## 1. Architecture Overview

### 1.1 Technology Stack

| Component            | Technology                        |
|----------------------|-----------------------------------|
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

```
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
└─────────────────────────────────────────────┘
```

### 1.3 Project Structure

```
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
│   │   ├── auth.ts                  # Auth API calls
│   │   ├── drives.ts               # Drive pair API calls
│   │   ├── files.ts                # File tracking API calls
│   │   ├── virtual-paths.ts        # Virtual path API calls
│   │   ├── folders.ts              # Tracked folder API calls
│   │   ├── integrity.ts            # Integrity check API calls
│   │   ├── sync.ts                 # Sync queue API calls
│   │   ├── scheduler.ts            # Scheduler API calls
│   │   ├── logs.ts                 # Event log API calls
│   │   ├── database.ts             # Database backup API calls
│   │   └── status.ts               # System status API calls
│   ├── stores/
│   │   ├── auth-store.ts           # Auth state (token, user)
│   │   ├── drives-store.ts         # Drive pairs state
│   │   ├── files-store.ts          # Tracked files state
│   │   ├── virtual-paths-store.ts  # Virtual path state
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
│   │   ├── DatabaseBackupsPage.tsx
│   │   └── VirtualPathManagerPage.tsx
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
│   │   │   └── DriveCard.tsx
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
│   │   ├── virtual-paths/
│   │   │   ├── VirtualPathTree.tsx
│   │   │   ├── BulkAssignDialog.tsx
│   │   │   └── PathMappingForm.tsx
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
│   │   ├── api.ts                   # API response types
│   │   ├── drive.ts
│   │   ├── file.ts
│   │   ├── virtual-path.ts
│   │   ├── integrity.ts
│   │   ├── sync.ts
│   │   ├── scheduler.ts
│   │   ├── log.ts
│   │   └── status.ts
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
│       ├── virtual-paths.spec.ts
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
- On success: store JWT in auth store, redirect to Dashboard
- On failure: display error message
- Redirect to Dashboard if already authenticated

---

### 2.2 Dashboard Page (`DashboardPage.tsx`)

**Purpose:** Provide an at-a-glance system overview.

**Components:**
- `StatusOverview` — displays key metrics:
  - Total tracked files
  - Mirrored files count
  - Pending sync items
  - Integrity issues count
  - Changed files requiring action
  - Last integrity check time
  - Last sync time
  - Number of drive pairs
- `QuickActions` — buttons for common operations:
  - Run integrity check
  - Process sync queue
  - Trigger database backup
- `RecentActivity` — latest 10 event log entries

**Data Source:** `GET /api/v1/status`, `GET /api/v1/logs?per_page=10`

---

### 2.3 File Browser Page (`FileBrowserPage.tsx`)

**Purpose:** Primary interface — file browser style view of all tracked files (Requirement 29).

**Layout:**
```
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
  - Columns: Name, Size, Checksum (truncated), Mirror status, Last verified, Actions
  - Toggle between grid and list view
  - Multi-select support for bulk operations
- `FileDetails` — detail panel for selected file showing:
  - Full real path (primary drive)
  - Full mirror path (secondary drive)
  - BLAKE3 checksum
  - Drive pair name
  - Last verification timestamp
  - Mirror status
- `FileActions` — per-file context menu:
  - Verify integrity
  - Edit virtual path
  - Remove tracking
  - View logs for this file

**Data Sources:** `GET /api/v1/files`, `GET /api/v1/virtual-paths`

---

### 2.4 Drives Page (`DrivesPage.tsx`)

**Purpose:** Manage drive pairs.

**Components:**
- `DriveList` — cards or table showing all drive pairs
  - Each card shows: name, primary path, secondary path, file count
- `DriveForm` — modal form for creating/editing drive pairs
  - Fields: name, primary path, secondary path
  - Validation: paths must be non-empty and different
- `DriveCard` — individual drive pair display with edit/delete actions

**Data Source:** `GET /api/v1/drives`, `POST/PUT/DELETE /api/v1/drives`

---

### 2.5 Folders Page (`FoldersPage.tsx`)

**Purpose:** Manage tracked folders.

**Components:**
- `FolderList` — table showing tracked folders
  - Columns: Path, Drive Pair, Auto Virtual Path, Default Virtual Base
- `FolderForm` — modal form for adding/editing tracked folders
  - Fields: drive pair (select), folder path, auto virtual path (toggle), default virtual base

**Data Source:** `GET /api/v1/folders`, `POST/PUT/DELETE /api/v1/folders`

---

### 2.6 Integrity Page (`IntegrityPage.tsx`)

**Purpose:** View integrity check status and results, trigger checks.

**Components:**
- `IntegrityStatus` — current/last check status
  - Progress bar during active check
  - Summary: passed, master corrupted, mirror corrupted, both corrupted, auto-recovered
- `IntegrityResults` — detailed results table
  - File path, status, master checksum, mirror checksum, action taken
- `CorruptionAlert` — prominent alert banner for files requiring user action
- "Run Integrity Check" button

**Data Source:** `POST /api/v1/integrity/check`, `GET /api/v1/integrity/check/{job_id}`

**Polling:** When a check is running, poll job status every 2 seconds via `usePolling` hook.

---

### 2.7 Sync Queue Page (`SyncQueuePage.tsx`)

**Purpose:** View and manage the sync queue.

**Components:**
- `SyncQueueTable` — filterable table of queue items
  - Columns: File, Action, Status, Created, Completed, Error
  - Filter by status: pending, in_progress, completed, failed
- `SyncQueueItem` — row with action button for items requiring resolution
- `ResolveDialog` — modal for resolving "both corrupted" items
  - Options: Keep master, Keep mirror, Provide new file
  - File path input for "provide new" option
- "Process Queue" button to trigger sync

**Data Source:** `GET /api/v1/sync/queue`, `POST /api/v1/sync/run`, `POST /api/v1/sync/queue/{id}/resolve`

---

### 2.8 Virtual Path Manager Page (`VirtualPathManagerPage.tsx`)

**Purpose:** Dedicated page for managing virtual path assignments, including bulk operations.

**Components:**
- `VirtualPathTree` — tree view of current virtual path structure
- `PathMappingForm` — form to assign/edit a single file's virtual path
- `BulkAssignDialog` — dialog for bulk operations:
  - Select drive pair
  - Select source folder
  - Define virtual base
  - Define prefix to strip
  - Preview of resulting mappings before applying
- "Refresh Symlinks" button

**Data Source:** `GET /api/v1/virtual-paths`, `PUT /api/v1/files/{id}/virtual-path`, `POST /api/v1/virtual-paths/bulk`, `POST /api/v1/virtual-paths/bulk-from-real`, `POST /api/v1/virtual-paths/refresh-symlinks`

---

### 2.9 Scheduler Page (`SchedulerPage.tsx`)

**Purpose:** Configure sync and integrity check schedules.

**Components:**
- `ScheduleList` — table of configured schedules
  - Columns: Task Type, Cron/Interval, Enabled, Last Run, Next Run
  - Enable/disable toggle per schedule
- `ScheduleForm` — modal for creating/editing schedules
  - Fields: task type (select: sync / integrity_check), cron expression or interval, enabled toggle
  - Cron expression helper/validation

**Data Source:** `GET /api/v1/scheduler`, `POST/PUT/DELETE /api/v1/scheduler`

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
- Table of backup configurations: path, drive label, max copies, enabled, last backup
- Add/edit/delete backup destinations
- "Run Backup Now" button
- Results display after backup execution

**Data Source:** `GET /api/v1/database/backups`, `POST/PUT/DELETE /api/v1/database/backups`, `POST /api/v1/database/backups/run`

---

## 3. Shared Infrastructure

### 3.1 API Client (`api/client.ts`)

- Axios instance configured with base URL from environment variable
- Request interceptor: attach JWT bearer token from auth store
- Response interceptor: on 401 → clear auth state → redirect to login
- Typed response wrappers for all endpoints

### 3.2 Authentication Flow

1. User submits credentials on Login page
2. POST `/api/v1/auth/login` → receive JWT
3. Store JWT in Zustand auth store (persisted to sessionStorage)
4. All subsequent requests include `Authorization: Bearer <token>`
5. On token expiry or 401 response → redirect to login

### 3.3 Protected Routing

- `ProtectedRoute` component wraps all authenticated pages
- Checks auth store for valid token
- Redirects to `/login` if not authenticated
- Renders children if authenticated

### 3.4 Polling Hook (`usePolling`)

Used for monitoring long-running operations (integrity checks, sync jobs):
- Accepts a fetch function and interval (default 2s)
- Calls fetch function on interval
- Returns current data, loading state, error state
- Auto-stops when job reaches terminal state (completed/failed)

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
1. Implement TypeScript types for auth (`types/api.ts`, `types/auth.ts`)
2. Implement auth API client (`api/auth.ts`)
3. Implement auth Zustand store (`stores/auth-store.ts`)
4. Implement `LoginPage.tsx` with form validation
5. Implement `AppLayout.tsx` with sidebar and header
6. Implement `Sidebar.tsx` with navigation links
7. Implement `Header.tsx` with user info and logout
8. Implement `ProtectedRoute.tsx`
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
1. Implement status types (`types/status.ts`)
2. Implement status API client (`api/status.ts`)
3. Implement status store (`stores/status-store.ts`)
4. Implement logs API client (partial — recent entries) (`api/logs.ts`)
5. Implement `DashboardPage.tsx`
6. Implement `StatusOverview.tsx` — status metric cards
7. Implement `QuickActions.tsx` — action buttons
8. Implement `RecentActivity.tsx` — recent log entries list

**Tests:**
- Unit: Status store fetches and stores data correctly
- Component: StatusOverview — renders all metric values
- Component: QuickActions — buttons trigger correct API calls
- Component: RecentActivity — renders log entries
- Component: DashboardPage — displays loading state, then data
- E2E: Dashboard loads and displays live system status

**Commit:** `feat: dashboard page with system status overview and quick actions`

---

### Milestone 4: Drive Pair Management
**Objective:** Implement drive pair configuration UI.

**Steps:**
1. Implement drive types (`types/drive.ts`)
2. Implement drives API client (`api/drives.ts`)
3. Implement drives store (`stores/drives-store.ts`)
4. Implement `DrivesPage.tsx`
5. Implement `DriveList.tsx`
6. Implement `DriveCard.tsx`
7. Implement `DriveForm.tsx` with validation
8. Implement `ConfirmDialog.tsx` for delete confirmation

**Tests:**
- Unit: Drives store — CRUD operations update state correctly
- Component: DriveList — renders all drive pairs
- Component: DriveForm — validates required fields, path uniqueness
- Component: DriveCard — displays info, edit/delete buttons work
- Component: ConfirmDialog — confirms or cancels action
- E2E: Create, edit, and delete a drive pair

**Commit:** `feat: drive pair management page with CRUD operations`

---

### Milestone 5: File Browser — Core
**Objective:** Implement the primary file browser interface.

**Steps:**
1. Implement file types (`types/file.ts`, `types/virtual-path.ts`)
2. Implement files API client (`api/files.ts`)
3. Implement virtual paths API client (`api/virtual-paths.ts`)
4. Implement files store (`stores/files-store.ts`)
5. Implement virtual paths store (`stores/virtual-paths-store.ts`)
6. Implement `FileBrowserPage.tsx`
7. Implement `FileTree.tsx` — virtual path tree sidebar
8. Implement `BreadcrumbNav.tsx`
9. Implement `FileGrid.tsx` — sortable file table with pagination
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
1. Implement `FileDetails.tsx` — detail panel for selected file
2. Implement `FileActions.tsx` — context menu with actions
3. Wire "Verify Integrity" action to API
4. Wire "Edit Virtual Path" to inline edit or modal
5. Wire "Remove Tracking" with confirmation dialog
6. Wire "View Logs" to filtered logs view
7. Implement multi-select for bulk operations in FileGrid

**Tests:**
- Component: FileDetails — displays all file metadata
- Component: FileActions — menu renders, actions fire correct API calls
- Component: FileGrid — multi-select works, bulk action bar appears
- E2E: Select file → view details → verify integrity → see result
- E2E: Edit virtual path for a file

**Commit:** `feat: file browser details panel and file actions`

---

### Milestone 7: Virtual Path Management
**Objective:** Implement dedicated virtual path management with bulk operations.

**Steps:**
1. Implement `VirtualPathManagerPage.tsx`
2. Implement `VirtualPathTree.tsx` — full tree view
3. Implement `PathMappingForm.tsx`
4. Implement `BulkAssignDialog.tsx`:
   - Drive pair selector
   - Source folder browser
   - Virtual base input
   - Prefix strip input
   - Preview table of resulting mappings
   - Apply button
5. Implement "Refresh Symlinks" action

**Tests:**
- Component: VirtualPathTree — renders full path hierarchy
- Component: PathMappingForm — validates and submits path mapping
- Component: BulkAssignDialog — preview shows correct computed paths
- Component: BulkAssignDialog — apply triggers correct API call
- E2E: Bulk assign virtual paths from folder, verify preview, apply

**Commit:** `feat: virtual path manager with bulk assignment and symlink refresh`

---

### Milestone 8: Tracked Folders
**Objective:** Implement tracked folder management UI.

**Steps:**
1. Implement folder types (`types/folder.ts` if not done)
2. Implement folders API client (`api/folders.ts`)
3. Implement `FoldersPage.tsx`
4. Implement `FolderList.tsx`
5. Implement `FolderForm.tsx` with drive pair selector and auto-virtual-path toggle

**Tests:**
- Unit: Folders API client — correct requests for CRUD
- Component: FolderList — renders all tracked folders
- Component: FolderForm — validates and submits, drive pair selector works
- E2E: Add tracked folder with auto-virtual-path, verify in list

**Commit:** `feat: tracked folder management page`

---

### Milestone 9: Integrity Check & Sync Queue
**Objective:** Implement integrity checking UI and sync queue management.

**Steps:**
1. Implement integrity types (`types/integrity.ts`)
2. Implement integrity API client (`api/integrity.ts`)
3. Implement sync types (`types/sync.ts`)
4. Implement sync API client (`api/sync.ts`)
5. Implement sync store (`stores/sync-store.ts`)
6. Implement `usePolling` hook
7. Implement `IntegrityPage.tsx`
8. Implement `IntegrityStatus.tsx` — progress bar and status
9. Implement `IntegrityResults.tsx` — results table
10. Implement `CorruptionAlert.tsx` — alert banner
11. Implement `SyncQueuePage.tsx`
12. Implement `SyncQueueTable.tsx` — filterable queue table
13. Implement `SyncQueueItem.tsx`
14. Implement `ResolveDialog.tsx` — resolution modal for both-corrupted items

**Tests:**
- Unit: usePolling — polls at interval, stops on terminal state
- Unit: Sync store — filters by status correctly
- Component: IntegrityStatus — shows progress during check, summary after
- Component: IntegrityResults — renders result rows with correct status colors
- Component: CorruptionAlert — displays when both-corrupted files exist
- Component: SyncQueueTable — filters by status, renders items correctly
- Component: ResolveDialog — options render, submission sends correct payload
- E2E: Trigger integrity check → monitor progress → view results
- E2E: View sync queue → resolve a both-corrupted item

**Commit:** `feat: integrity check and sync queue management pages`

---

### Milestone 10: Scheduler Configuration
**Objective:** Implement scheduler configuration UI.

**Steps:**
1. Implement scheduler types (`types/scheduler.ts`)
2. Implement scheduler API client (`api/scheduler.ts`)
3. Implement `SchedulerPage.tsx`
4. Implement `ScheduleList.tsx` with enable/disable toggle
5. Implement `ScheduleForm.tsx` with cron expression input and interval option

**Tests:**
- Component: ScheduleList — renders schedules, toggle changes enabled state
- Component: ScheduleForm — validates cron expression format
- Component: ScheduleForm — interval and cron are mutually exclusive options
- E2E: Create schedule, toggle enable/disable, delete schedule

**Commit:** `feat: scheduler configuration page`

---

### Milestone 11: Event Logs
**Objective:** Implement event log viewing with filters.

**Steps:**
1. Implement log types (`types/log.ts`)
2. Implement logs API client (`api/logs.ts` — complete)
3. Implement logs store (`stores/logs-store.ts`)
4. Implement `LogsPage.tsx`
5. Implement `LogFilter.tsx` — event type multiselect, date range, file search
6. Implement `LogTable.tsx` — paginated, expandable rows, color-coded types

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
1. Implement database backup API client (`api/database.ts`)
2. Implement `DatabaseBackupsPage.tsx`
3. Implement backup config table with enable/disable and CRUD
4. Implement "Run Backup Now" with results display

**Tests:**
- Component: Backup config table — renders configs, edit/delete work
- Component: Run backup — shows results after execution
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
1. Configure Vite production build
2. Output static assets to `dist/` directory
3. Configure backend to serve frontend static files (or document reverse proxy setup)
4. Verify production build works with backend TLS
5. Include built frontend in .deb package

**Tests:**
- Unit: Production build completes without errors
- E2E: Full E2E suite runs against production build
- Integration: Frontend served by backend, all pages functional

**Commit:** `feat: production build and packaging integration`

---

## 5. Testing Strategy

### 5.1 Test Levels

| Level      | Scope                              | Tools                        | Location             |
|------------|-------------------------------------|------------------------------|----------------------|
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
|-------------------|--------------------------------------------|
| API responses     | MSW (Mock Service Worker) in unit/component tests |
| Auth state        | Direct Zustand store manipulation          |
| Router            | MemoryRouter wrapper in component tests    |
| Timers (polling)  | Vitest fake timers                         |

### 5.4 E2E Test Coverage

Each E2E spec covers a full user workflow:

| Spec File                  | Workflow                                              |
|----------------------------|-------------------------------------------------------|
| `auth.spec.ts`             | Login, session persistence, logout, expired token     |
| `file-browser.spec.ts`     | Navigate tree, sort grid, select file, view details   |
| `drives.spec.ts`           | Create, edit, delete drive pair                       |
| `integrity.spec.ts`        | Trigger check, monitor progress, view results         |
| `sync-queue.spec.ts`       | View queue, filter, resolve corrupted item            |
| `virtual-paths.spec.ts`    | Assign path, bulk assign, refresh symlinks            |
| `scheduler.spec.ts`        | Create, toggle, delete schedule                       |
| `logs.spec.ts`             | Filter by type, date range, expand details            |
| `database-backups.spec.ts` | Add destination, trigger backup, view results         |

### 5.5 Test Coverage Targets

| Area                  | Minimum Coverage |
|-----------------------|-----------------|
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

# Development only — disable TLS verification
VITE_DEV_INSECURE=false
```

---

## 7. UI Design Guidelines

### 7.1 Design Principles
- **File browser as primary interface** — the main view users see and interact with most
- Clean, minimal design using Shadcn/ui components
- Consistent color coding for statuses:
  - Green: healthy / passed / mirrored
  - Yellow: pending / warning / changed
  - Red: corrupted / failed / error
  - Gray: unverified / inactive

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
