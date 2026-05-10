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

**Create destination using path picker:** The test opens the "Add Destination" form, clicks the Browse button (which opens the `PathPickerDialog`), selects a path through a mock picker, fills in the drive label field, and submits. It verifies that the creation request body contains the correct `backup_path` and `drive_label` values. This test is important because the path comes from a dialog rather than a text field typed by the user, so the wiring between the picker and the form field must be correct.

**Run backup immediately:** After a destination exists, clicking "Run Backup Now" calls the run-backup endpoint and a success toast appears. The path written to is displayed in the result.

**Save backup settings:** The settings form (automatic backup interval, automatic integrity check interval) is filled and submitted. The test verifies the settings are sent to the correct endpoint with the correct body shape.

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

**Start and stop flow through dialog:** The full modal flow is tested: click "Run Check" → modal opens → select a drive pair and toggle auto-recovery → click "Start" → start endpoint is called → running banner appears → click "Stop" → stop endpoint is called → banner disappears. This covers the complete UI state machine for an integrity run without requiring a real backend.

**Loading state during initial fetch:** While the latest run data is being fetched (the mock response is not yet resolved), the page shows a loading indicator. This confirms the bootstrap loading state is rendered correctly.

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

**Create cron-based schedule:** The test switches to the "Cron Schedule" timing method, clicks a daily preset, and submits. The created schedule row in the table shows the human-readable description corresponding to the preset.

**Local time display:** Schedule times are rendered in the user's local time zone, not UTC. The test uses a locale-aware time formatter to generate the expected string and compares it against what the page renders, ensuring localization is applied consistently.

---

### SyncQueuePage.test.tsx

**File:** `frontend/src/pages/SyncQueuePage.test.tsx`

**Resolve manual action item:** A queue item in `user_action_required` status has a "Resolve" button. Clicking it opens a resolution dialog. Confirming the resolution calls the resolve endpoint with `{ "resolution": "keep_master" }` and a success toast appears. This tests the dialog-to-API wiring for the most sensitive queue operation.

**Polling every five seconds:** The queue list refreshes automatically. The test uses fake timers to advance time by 5,000 ms and verifies that the list endpoint has been called more than once, confirming the polling is active.

**Empty state:** When the queue is empty, the "No queue items" message is rendered.

**Process and pause buttons visible:** When queue items exist, both the "Process Queue" and "Pause Queue" buttons are present.

---

### TrackingWorkspacePage.test.tsx

**File:** `frontend/src/pages/TrackingWorkspacePage.test.tsx`

This page is the most complex in the application. It renders a unified list of both tracked files and tracked folders, with filters, a virtual-path tree, and per-item action buttons.

**Unified mixed list with source badges:** When the tracking list endpoint returns a mix of files and folders, each item's row is rendered with the correct source badge ("Direct" for directly tracked files, "Folder" for folder-origin items). Folder rows also display their aggregate status badge.

**Folder status badge variants:** The test verifies that each of the four folder status values renders the correct badge:

- `not_scanned` — the folder has been added but never scanned
- `empty` — the folder was scanned but no files were found
- `tracked` — files have been found but not mirrored
- `mirrored` — all files are mirrored
- `partial` — some but not all files are mirrored, with the count displayed as "Partial (N/M)"

**Source dropdown does not include "Both":** The source filter dropdown offers "Direct", "Folder", and "All" as options. The "Both" option that the API rejects with `400` is not present in the dropdown. This prevents a class of client-caused API errors.

**Virtual-path tree selection:** Clicking a node in the left-pane virtual-path tree triggers a new tracking list request with the corresponding `virtual_prefix` query parameter. This confirms the tree drives server-side filtering rather than client-side filtering of already-loaded data.

**Left-pane collapse and expand:** Clicking the collapse control hides the virtual-path tree. Clicking again reveals it. The tracking table expands to fill the available space when the tree is collapsed.

---

## Component Tests

### AppLayout.test.tsx

**File:** `frontend/src/components/layout/AppLayout.test.tsx`

**No top header chrome on authenticated pages:** The application uses a sidebar layout without a traditional top navigation bar on authenticated pages. The test verifies that no `<header>` landmark element is rendered, which would indicate the wrong layout component is being used.

**Sidebar user menu and logout:** The sidebar footer contains the logged-in username and a user menu trigger. The test opens the user menu, verifies the username is displayed in the footer, and clicks "Logout". After logout, the login page is rendered and the auth store reflects the unauthenticated state.

---

### ProtectedRoute.test.tsx

**File:** `frontend/src/components/layout/ProtectedRoute.test.tsx`

**Redirect unauthenticated users:** When the auth hook reports `isAuthenticated: false`, rendering a protected route redirects to `/login` rather than rendering the protected content.

**Render children when valid:** When the auth hook reports `isAuthenticated: true` and token validation succeeds, the protected content is rendered.

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

---

### PathPickerDialog.test.tsx

**File:** `frontend/src/components/shared/PathPickerDialog.test.tsx`

The path picker dialog is a lazy-loading tree browser that communicates with the filesystem browse API. It is used in several places: drive configuration, file tracking, folder tracking, and backup destination creation.

**Lazy loading of children:** When the dialog opens, the root directory entries are loaded. Expanding a directory node triggers a new API request for that directory's children rather than loading everything up front.

**Tree selection calls onPick:** Clicking a directory in the tree selects it and calls the `onPick` callback with the selected path when the user confirms.

**Directory-only filtering:** When opened in `mode: "directory"` mode, file entries are visually absent from the tree. Only directories are shown and selectable.

**Hidden file toggle:** The "Show hidden files" toggle triggers a new request with `show_hidden=true` and hidden entries appear in the tree.
