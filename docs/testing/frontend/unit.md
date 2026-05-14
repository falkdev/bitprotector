# Frontend Unit and Component Tests

These tests use Vitest and React Testing Library to render components in a JSDOM environment with MSW-mocked API responses. For an overview of the toolchain and patterns, see [README.md](README.md).

---

## Table of Contents

### Page tests

- [DashboardPage.test.tsx](#dashboardpagetesttsx)
- [DatabaseBackupsPage.test.tsx](#databasebackupspagetesttsx)
- [DrivesPage.test.tsx](#drivespagetesttsx)
- [IntegrityPage.test.tsx](#integritypagetesttsx)
- [LoginPage.test.tsx](#loginpagetesttsx)
- [LogsPage.test.tsx](#logspagetesttsx)
- [SchedulerPage.test.tsx](#schedulerpagetesttsx)
- [SyncQueuePage.test.tsx](#syncqueuepagetesttsx)
- [TrackingWorkspacePage.test.tsx](#trackingworkspacepagetesttsx)

### Component tests

- [AppLayout.test.tsx](#applayouttesttsx)
- [ProtectedRoute.test.tsx](#protectedroutetesttsx)
- [DriveForm.test.tsx](#driveformtesttsx)
- [TrackFileModal.test.tsx](#trackfilemodaltesttsx)
- [FolderFormModal.test.tsx](#folderformmodaltesttsx)
- [ConfirmDialog.test.tsx](#confirmdialogtesttsx)
- [ModalLayer.test.tsx](#modallayertesttsx)
- [PathPickerDialog.test.tsx](#pathpickerdialogtesttsx)

---

## Page Tests

### DashboardPage.test.tsx

**File:** `frontend/src/pages/DashboardPage.test.tsx`

The dashboard displays system status metrics and quick-action buttons for sync, integrity, and database backup.

**Quick action disabled without drives:** When the status endpoint reports zero drive pairs, the integrity quick-action button is disabled and a helper text element explains that a drive pair must be added first. The sync and backup buttons remain enabled because they can run without a drive pair. This test guards against a regression where the drive-count guard was removed or applied to the wrong button.

---

### DatabaseBackupsPage.test.tsx

**File:** `frontend/src/pages/DatabaseBackupsPage.test.tsx`

**Create destination and run backup:** The test opens the "Add Destination" form, clicks Browse (which invokes the `PathPickerDialog` mock), fills in the drive label field, and submits. It verifies that the creation request body contains `backup_path`, `drive_label`, and `enabled: true`. After creation it clicks "Run Backup Now" and verifies the success toast and the backup file path in the result.

**Save backup settings:** The settings form (automatic backup interval, automatic integrity check interval) is filled and submitted. The test verifies the settings body contains `backup_enabled`, `backup_interval_seconds`, `integrity_enabled`, and `integrity_interval_seconds`, sent via `PUT /database/backups/settings`.

**Disables manual actions when no enabled destinations:** When the only configured destination has `enabled: false`, the "Run Backup Now" and "Check Integrity Now" buttons are disabled and a hint is shown.

**Integrity check and staged restore:** Clicking "Check Integrity Now" calls the integrity-check endpoint and renders the result status. Clicking "Restore Older Backup" opens a form with a Browse button; selecting a file path and clicking "Stage Restore" calls the restore endpoint and shows a "Restore staged; restart BitProtector to apply it" toast, and the "Restore Staged" label appears.

**Backup form validation:** Submitting the "Add Destination" form without a backup path shows "Backup path is required." rather than sending the request.

The `PathPickerDialog` is mocked in this test file to return a pre-set path, keeping the test focused on the backup page logic rather than the picker's own behavior (which is tested in `PathPickerDialog.test.tsx`).

---

### DrivesPage.test.tsx

**File:** `frontend/src/pages/DrivesPage.test.tsx`

**Empty state shows a single add button:** When no drive pairs are configured, the page shows exactly one "Add Drive Pair" button (not multiple across the header and empty state). Clicking it opens the create modal overlay and the drive name input becomes visible.

This test is minimal by design: the drive form behavior is covered in `DriveForm.test.tsx` and the full create/edit/delete workflow is covered in the E2E tests. The unit test focuses on the empty-state rendering and modal trigger behavior, which are the most likely things to break during a UI refactor.

---

### IntegrityPage.test.tsx

**File:** `frontend/src/pages/IntegrityPage.test.tsx`

The integrity page has several states: idle (showing latest results), running (showing progress), and no-drives.

**Latest results on load:** When the latest run endpoint returns a completed run with results, the "Last integrity check" timestamp is rendered with the run's end time, and the result rows appear in the table. This confirms the page correctly initializes from cached results rather than starting blank.

**No drives — button disabled with hint:** When no drive pairs exist, the "Run Check" button is disabled and a helper text element is visible. Clicking the disabled button does not open the start-run modal.

**Start and stop flow through dialog:** The full modal flow is tested: click "Run Check" → modal opens → click "Start" → start endpoint is called → "Integrity run started" toast and running banner ("Integrity check running...") appear → click "Stop" → stop endpoint is called → "Stop requested for run #301" toast appears.

**Active workers card:** When an active run is in `running` status with `active_workers: 4`, a card reading "Files checking in parallel" with the worker count `4` is displayed.

---

### LoginPage.test.tsx

**File:** `frontend/src/pages/LoginPage.test.tsx`

**Form elements are rendered:** The login page renders the heading, form container, username input, password input, and login button. The page title element (which appears on authenticated pages as a heading row) is not rendered on the login page — this confirms the login page uses the unauthenticated layout.

This test is intentionally minimal. The login interaction logic lives in the `useAuth` hook, which is mocked out. The page test only verifies that the correct elements are present and the layout is correct.

---

### LogsPage.test.tsx

**File:** `frontend/src/pages/LogsPage.test.tsx`

**Filter by file ID and expand details:** The user types a file ID into the filter field, clicks Apply, and the request to the logs endpoint includes the correct `file_id` query parameter. The test also clicks the "View" button on a log entry to expand its detail panel, and verifies that the structured JSON detail content is rendered.

**Empty state:** When the logs endpoint returns no entries, the "No matching log entries" message is visible. This confirms the page handles the zero-results case gracefully rather than rendering an empty table body without explanation.

---

### SchedulerPage.test.tsx

**File:** `frontend/src/pages/SchedulerPage.test.tsx`

**Create interval-based integrity schedule:** The test opens the create form, clicks the "Integrity Check" task type card, sets the interval to 1 hour, and submits. The request body is captured and verified to contain `task_type: "integrity_check"` and `interval_seconds: 3600`.

**Validation feedback for empty interval:** If the interval value field is cleared before submission, the form displays a validation error ("Interval must be a positive number.") rather than sending an invalid request.

**Create cron-based schedule:** The test switches to the "Cron Schedule" timing method, clicks a daily preset ("Daily at 02:00"), and submits. The request body contains `cron_expr: '0 2 * * *'`.

**Create custom cron schedule:** The test switches to "Cron Schedule", clicks "Custom…", types `30 4 * * 1-5`, and submits. The request body contains the typed `cron_expr`.

**Cron validation — no expression:** Switching to "Cron Schedule", clicking "Custom…", and submitting without entering anything shows "Select a preset or enter a custom cron expression."

**Interval unit conversion:** Entering 2 in the interval field and selecting "hours" submits `interval_seconds: 7200`.

**Human-friendly descriptions in the table:** Three schedules are loaded — a cron sync at 02:00, an hourly integrity-check, and a 2-minute sync interval. The rows show "File Sync" / "Daily at 02:00" (time formatted to the user's locale via `toLocaleTimeString`), "Integrity Check" / "Every hour", and "Every 2 minutes" respectively.

---

### SyncQueuePage.test.tsx

**File:** `frontend/src/pages/SyncQueuePage.test.tsx`

**Resolve manual action item:** A queue item in `user_action_required` status has a "Resolve" button. Clicking it opens a resolution dialog. Confirming the resolution calls the resolve endpoint with `{ "resolution": "keep_master" }` and a success toast appears. This tests the dialog-to-API wiring for the most sensitive queue operation.

**Polling every five seconds:** The queue list refreshes automatically. The test uses fake timers to advance time by 5,000 ms and verifies that the list endpoint has been called more than once, confirming the polling is active.

**Empty state:** When the queue is empty, the "No queue items" message is rendered.

**Buttons present with queue items:** "Process Queue", "Clear Completed", and "Pause Queue" are all present. "Run Sync Task" and "Run Integrity Task" are absent.

**Disables process queue when no drive pairs:** "Process Queue" is disabled and a hint is shown; "Clear Completed" remains enabled.

**Clear Completed button state:** Disabled when no completed items exist; enabled when at least one completed item exists.

**Clear completed removes items:** Clicking "Clear Completed" calls `DELETE /sync/queue/completed`, shows a "Cleared 1 completed queue item(s)" toast, and the completed row disappears while pending rows remain.

**Clear completed error:** When the delete endpoint returns 500, a "Failed to clear completed queue items" toast appears and the row is still visible.

**Paused banner:** When `queue_paused: true` is returned, a pause banner is shown and the button label changes to "Resume Queue". Clicking "Resume Queue" calls the resume endpoint and the banner is removed.

**Hidden paused banner:** When `queue_paused: false`, the pause banner is absent and the "Pause Queue" button is visible.

**Pause queue:** Clicking "Pause Queue" calls the pause endpoint and the paused banner appears.

**Resume queue:** Clicking "Resume Queue" calls the resume endpoint and the "Sync queue processing resumed" toast appears, with the banner removed.

---

### TrackingWorkspacePage.test.tsx

**File:** `frontend/src/pages/TrackingWorkspacePage.test.tsx`

This page is the most complex in the application. It renders a unified list of both tracked files and tracked folders, with filters, a virtual-path tree, and per-item action buttons.

**Unified mixed list with source badges:** Files show a "Direct" badge; folder rows show "Folder" and their aggregate status badge ("Partial (2/4)" in the mixed-list test).

**Folder status badge variants:** Five status values are tested:

- `not_scanned` → "Not scanned" (no file count displayed)
- `empty` → "Empty" (no file count displayed)
- `tracked` → "Tracked (10/10)"
- `partial` → "Partial (4/10)"
- `mirrored` → "Mirrored (10/10)"

**Source dropdown does not include "Both":** The legacy "Both" option is absent. The API rejects `source=both` with 400, so removing it prevents a class of client-caused errors.

**No drive pairs — actions disabled:** When no drive pairs exist, the "Track File" and "Add Folder" buttons are disabled and a hint is shown. Clicking them does not open any modal.

**Filter dropdowns send correct params:** Selecting drive, kind, source, and has_virtual_path from the four filter dropdowns causes the tracking list request to include the matching query parameters.

**Redundant virtual prefix text field removed:** There is no free-text "Virtual path prefix (/docs)" filter input — filtering by virtual path is done exclusively via the tree pane.

**Bulk action bar — multi-select and deselect:** Clicking row checkboxes shows the bulk action bar with a selected-count label ("1 selected (1 file, 0 folders)", "2 selected (2 files, 0 folders)"). Clicking the deselect control hides the bar.

**Bulk action bar — mirror:** Selecting one file row and one folder row and clicking "Mirror" calls `POST /files/{id}/mirror` and `POST /folders/{id}/mirror` for each.

**Bulk action bar — delete:** Selecting mixed items and confirming deletion via the alert dialog calls `DELETE /files/{id}` and `DELETE /folders/{id}` and removes the rows.

**Detail panel navigates after delete:** Deleting the open file while its detail panel is visible moves the panel to the next file. Deleting the last remaining file closes the panel entirely.

**Virtual-path tree drives server-side filtering:** Clicking a tree node sends a new `tracking/items` request with the `virtual_prefix` matching the selected path. Files not matching the prefix disappear from the table.

**Folder set-path flow shows browse control:** Clicking "Set Path" on a folder row opens a "Set Folder Virtual Path" dialog containing a "Browse" button.

**Folder Scan → Mirror button transition:** After clicking "Scan" on a `not_scanned` folder the button changes to "Mirror" (status becomes `tracked`). After clicking "Mirror" the button returns to "Scan" (status becomes `mirrored`).

**Virtual tree refreshes after folder scan:** After a successful scan, the virtual-paths tree is re-fetched (tree call count increases).

**Virtual paths pane collapse and expand:** The pane starts collapsed (tree nodes not present). Clicking the toggle reveals the tree; clicking again hides it; clicking a third time reveals it again.

**File detail panel — BLAKE3 checksum:** Clicking a file row opens the detail panel showing the full BLAKE3 checksum string, the "Primary Mirror" label, and the "Last integrity check" field.

**File detail panel — effective virtual path from list data:** When the files endpoint returns `virtual_path: null` but the tracking list item had a non-null `virtual_path`, the detail panel shows the effective virtual path from the list data.

---

## Component Tests

### AppLayout.test.tsx

**File:** `frontend/src/components/layout/AppLayout.test.tsx`

**No top header chrome on authenticated pages:** The application uses a sidebar layout without a traditional top navigation bar on authenticated pages. The test verifies that no element with the `banner` ARIA role is rendered.

**Sidebar user menu and logout:** The sidebar footer contains the logged-in username and a user menu trigger. The test opens the user menu, verifies the username is displayed in the footer, and clicks "Logout". After logout, the login page is rendered and the auth store reflects the unauthenticated state.

**Sidebar collapse persists across remounts:** Clicking the sidebar toggle collapses the sidebar from `w-56` to `w-16`, stores `'1'` in `localStorage` under the collapse key, and nav items switch to icon-only mode with `title` attributes. After unmounting and re-rendering, the sidebar is still collapsed and nav items still show their `title` attributes.

**Dark mode toggle in user menu:** Opening the user menu and clicking the theme toggle adds the `dark` class to the document root. Clicking it again removes the class.

---

### ProtectedRoute.test.tsx

**File:** `frontend/src/components/layout/ProtectedRoute.test.tsx`

**Redirect unauthenticated users:** When the auth hook reports `isAuthenticated: false`, rendering a protected route redirects to `/login` rather than rendering the protected content.

**Render children when valid:** When the auth hook reports `isAuthenticated: true` and token validation succeeds, the protected content is rendered.

**Redirect when validation fails:** When `isAuthenticated: true` but `validate()` resolves to `false`, the component redirects to `/login`.

---

### DriveForm.test.tsx

**File:** `frontend/src/components/drives/DriveForm.test.tsx`

**Path picker fills the input:** Clicking the Browse button for the primary path opens the `PathPickerDialog`. When a path is selected from the mock dialog, the primary path input field is populated with the selected path.

**Skip validation checkbox:** When "Skip path validation" is checked and the form is submitted, the request body includes `skip_validation: true`. This flag bypasses the on-disk path existence check, which is required when setting up a drive pair before the drives are mounted.

**Backend error display:** When the create request fails with a `400` response containing an error message, that message is displayed in the form. The test simulates the error through the mock `onSave` function and verifies the message appears.

---

### TrackFileModal.test.tsx

**File:** `frontend/src/components/tracking/TrackFileModal.test.tsx`

**Absolute path converted to relative:** When the user types an absolute path (e.g., `/mnt/primary/docs/report.pdf`) into the file path field, the modal strips the drive root prefix before submitting. The `onTrack` callback receives `relative_path: "docs/report.pdf"` rather than the absolute path.

**Optional virtual path submitted when provided:** When the user also fills in the virtual path field, the `onTrack` callback receives both `relative_path` and `virtual_path`. When the virtual path field is left empty, `virtual_path` is absent from the call.

---

### FolderFormModal.test.tsx

**File:** `frontend/src/components/tracking/FolderFormModal.test.tsx`

**Absolute path converted to relative:** The same path normalization behavior as `TrackFileModal` is verified for folders. An absolute folder path is converted to the relative path before the `onSave` callback is called.

---

### ConfirmDialog.test.tsx

**File:** `frontend/src/components/shared/ConfirmDialog.test.tsx`

**Overlay appears on open:** Before the dialog is opened, no overlay is in the DOM. After the trigger is clicked, the modal overlay and the alert dialog element appear.

**Confirm fires the callback:** Clicking the "Confirm" button inside the dialog calls the `onConfirm` callback exactly once.

---

### ModalLayer.test.tsx

**File:** `frontend/src/components/shared/ModalLayer.test.tsx`

**Portal renders overlay:** When `ModalLayer` is mounted with children, a `modal-overlay` test ID element appears in the DOM via a portal. The children are rendered inside it.

**Conditional mounting:** When the parent component does not render `ModalLayer` (because a condition is false), no overlay appears. When the condition becomes true, the overlay appears. This verifies that the portal correctly tracks the component lifecycle.

**Toast actions work through the overlay:** When a `ModalLayer` is visible and a `sonner` toast with an action button is triggered, clicking the toast action button calls its callback. This confirms the overlay does not block pointer events on toasts rendered outside it.

---

### PathPickerDialog.test.tsx

**File:** `frontend/src/components/shared/PathPickerDialog.test.tsx`

The path picker dialog is a lazy-loading tree browser that communicates with the filesystem browse API. It is used in several places: drive configuration, file tracking, folder tracking, and backup destination creation.

**Root directory loaded on open:** When the dialog opens, `filesystemApi.children` is called with `{ path: '/', include_hidden: false, directories_only: true }` and the root entries are rendered.

**Constrained root via `rootPath` prop:** When `rootPath="/mnt/primary"` is passed, the initial request uses `path: '/mnt/primary'` instead of `/`.

**Lazy loading on expand:** Expanding a directory node triggers a second `children` call for that directory's path. The child entries appear in the tree after loading.

**File-mode directory selection disables confirmation:** When opened in `mode: "file"`, clicking a directory entry shows "Select a file, not a folder" and the "Use Path" button is disabled.

**Unavailable endpoint shows recovery hint:** When the API call rejects with the "filesystem browser endpoint not yet available" error message, that message is displayed alongside "You can still type a path above and confirm it manually."
