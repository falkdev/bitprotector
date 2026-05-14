# Frontend End-to-End Tests

These tests use Playwright to drive a real browser against a running QEMU guest. Each test follows a complete user workflow and verifies side-effects on the guest filesystem or API state. For an overview of the QEMU fixture and Playwright setup, see [README.md](README.md).

Run them with:

```bash
cd frontend
npm run test:e2e:qemu
```

---

## Table of Contents

- [auth-and-nav.spec.ts — Login, Navigation, and Logout](#auth-and-navspects--login-navigation-and-logout)
- [dashboard.spec.ts — Dashboard Quick Actions](#dashboardspects--dashboard-quick-actions)
- [drives.spec.ts — Drive Pair Lifecycle and Replacement Workflow](#drivesspects--drive-pair-lifecycle-and-replacement-workflow)
- [file-browser.spec.ts — File Tracking Lifecycle](#file-browserspects--file-tracking-lifecycle)
- [folders.spec.ts — Folder Tracking and Scan Workflow](#foldersspects--folder-tracking-and-scan-workflow)
- [integrity.spec.ts — Integrity Run Lifecycle](#integrityspects--integrity-run-lifecycle)
- [scheduler.spec.ts — Schedule Create, Edit, and Delete](#schedulerspects--schedule-create-edit-and-delete)
- [database-backups.spec.ts — Backup Destination Lifecycle and Restore Staging](#database-backupsspects--backup-destination-lifecycle-and-restore-staging)

---

## auth-and-nav.spec.ts — Login, Navigation, and Logout

**File:** `frontend/tests/e2e/auth-and-nav.spec.ts`

This spec runs without a pre-authenticated storage state (explicitly cleared at the top of the file). It validates the full authentication flow and navigation between all major pages.

**Login through the live backend:** The test visits the login page, enters credentials, and submits the form. After a successful login, the browser redirects to the dashboard and the page title is "Dashboard".

**Navigation to all protected pages:** After logging in, the test visits each sidebar destination — Drives, Sync Queue, Tracking Workspace, Integrity, Scheduler, Logs, and Database Backups — and verifies that each page loads, has the correct title, and shows its primary UI element (e.g., the "Add Drive Pair" button on the Drives page).

**Logout and redirect:** Clicking the user menu trigger in the sidebar and then clicking "Logout" redirects the browser to `/login`. Navigating to `/dashboard` after logout redirects back to `/login` rather than showing protected content. This confirms the session is fully invalidated.

**Dark-mode toggle persists across reload:** After logging in, the user opens the user menu and toggles dark mode. The `html` element gains the `dark` class and the computed `color-scheme` style switches to `dark`. After toggling back to light, the class is removed. This verifies that the theme preference is persisted in local storage and applied on render.

---

## dashboard.spec.ts — Dashboard Quick Actions

**File:** `frontend/tests/e2e/dashboard.spec.ts`

This spec tests the dashboard's three quick-action buttons against a real guest with real data.

**Setup:** A drive fixture is seeded on the guest (primary and secondary directories, test files) and a database backup destination is registered via the CLI before the test interacts with the UI.

**Status metrics visible:** The "Files Tracked" metric card is visible, confirming the dashboard loaded the status API successfully.

**Sync quick action:** Clicking the "Process Queue" quick action button shows a success toast confirming the queue was processed.

**Integrity quick action:** Clicking the "Run Check" quick action button shows a toast confirming an integrity run was started.

**Backup quick action:** Clicking the "Run Backup" quick action button shows a toast confirming the backup completed. The backup destination was registered in the setup step, so the backup has a real destination to write to.

---

## drives.spec.ts — Drive Pair Lifecycle and Replacement Workflow

**File:** `frontend/tests/e2e/drives.spec.ts`

**Create, edit, and delete a drive pair:** A drive pair is created using real paths from the seeded fixture. The pair card appears on the Drives page. The name is updated through the Edit button and the card reflects the new name. The pair is deleted using the Delete button and confirming the alert dialog. The pair card disappears and a success toast confirms deletion.

**Replacement workflow — mark, confirm, assign:** This test exercises the full planned replacement state machine through the UI:

1. Click "Replace" on the drive pair card to open the replacement panel.
2. Click "Mark for Replacement" — the card shows `P: quiescing` and a toast confirms.
3. Close the panel and re-open it.
4. Click "Confirm Failure" — the card shows `P: failed` and a toast confirms.
5. Close the panel and re-open it.
6. Enter the replacement primary path in the assign input and click "Assign Replacement Drive" — the card shows the replacement path and a toast confirms.

This is the most critical user-facing workflow in the application. The E2E test verifies that every step correctly updates both the UI and the backend state, and that the UI reflects the new state without requiring a page reload.

---

## file-browser.spec.ts — File Tracking Lifecycle

**File:** `frontend/tests/e2e/file-browser.spec.ts`

**Track a file:** After creating a drive pair with the fixture paths, the user opens the Tracking Workspace and clicks "Track File". The drive pair is selected and the absolute file path is entered. After clicking "Track file", a success toast appears and the file row is visible in the tracking list.

**Inspect file details:** Clicking the file row opens the detail panel on the right, which shows the file's relative path.

**Set virtual path:** Clicking the "Set Virtual Path" action on the file row opens the virtual path form. After entering the desired virtual path and saving, the tracking list row updates to show the virtual path.

**Mirror the file:** Clicking the "Mirror" action on the file row triggers an immediate mirror operation. After the toast confirms, the row shows "Mirrored". The test then verifies on the guest that the file exists at the expected secondary path and that its content matches the source file.

**Delete the tracked file:** Clicking the "Delete" action and confirming the dialog removes the file from tracking. The row disappears and the virtual path on the guest filesystem is also removed, confirming cleanup.

---

## folders.spec.ts — Folder Tracking and Scan Workflow

**File:** `frontend/tests/e2e/folders.spec.ts`

**Add a tracked folder:** A folder is added through the "Add Folder" button in the Tracking Workspace. The drive pair and folder relative path are selected, and a virtual path is set on the folder. After clicking "Add Folder", a success toast appears and the folder row is visible.

**Scan the folder:** Clicking the "Scan" button on the folder row scans the folder's directory on the primary drive and discovers new files. A toast reports the scan result with the number of new files found. The discovered file appears as a row in the tracking list under the folder.

**Mirror all folder files:** Clicking the "Mirror" button on the folder row mirrors all discovered files to the secondary drive. A toast reports the mirror result. The test then verifies on the guest that the mirrored file exists at the expected secondary path and that its content matches the source.

**Scan button re-appears after mirror:** After clicking Mirror, the folder row should show the "Scan" button again (indicating the folder is in the mirrored state and can be re-scanned). This confirms the button state is correctly derived from the folder status rather than being toggled client-side.

---

## integrity.spec.ts — Integrity Run Lifecycle

**File:** `frontend/tests/e2e/integrity.spec.ts`

This spec tests an integrity run against a real tracked file. Because the file is tracked but not yet mirrored (it is tracked at the start of the test but the secondary does not contain a copy), the integrity check should report an issue.

**Setup:** A drive pair is created with the fixture paths, and a file is tracked through the UI.

**Start an integrity run:** Navigating to the Integrity page and clicking "Run Check" opens the start-run modal. The drive pair is selected from the dropdown and automatic recovery is unchecked (to prevent the test from being confounded by auto-repair). Clicking "Start" triggers the run. A success toast confirms the run started.

**Progress banner visible:** After starting, the "Integrity check running" banner appears on the page, confirming the UI is showing the active run state.

**Issue row appears:** The test waits (up to 30 seconds) for a result row containing the tracked file's relative path to appear in the results table. The appearance of this row confirms the run completed and correctly identified the file as having no secondary copy.

---

## scheduler.spec.ts — Schedule Create, Edit, and Delete

**File:** `frontend/tests/e2e/scheduler.spec.ts`

**Create an interval-based File Sync schedule:** The test opens the Add Schedule form. "File Sync" is the default task type (verified to be selected). The interval is set to 2 hours. After creating the schedule, the row appears in the table with "Every 2 hours".

**Edit the schedule:** Clicking "Edit" on the row opens the edit form. The task type cards are disabled during editing (you cannot change the task type of an existing schedule). The interval is changed to 30 minutes and saved. The row updates to "Every 30 minutes".

**Delete the schedule:** Clicking "Delete" and confirming the alert dialog removes the schedule. The row disappears from the table.

**Create a cron-based Integrity Check schedule:** The test creates a second schedule: "Integrity Check" task type, "Cron Schedule" timing method, and a "Daily at" preset (the last button matching that label). After creation, the row shows "Integrity Check" and a human-readable description matching `/Daily at/`. The schedule is deleted at the end of the test as cleanup.

**Create a schedule with a custom cron expression:** Switching to "Cron Schedule" and clicking "Custom…" reveals a text input. The expression `15 3 * * 1-5` is typed, and after creation the row displays `Cron: 15 3 * * 1-5`. The schedule is deleted as cleanup.

**Toggle a schedule between enabled and disabled:** A 6-hour interval File Sync schedule is created. Clicking the "Enabled" toggle button shows a "Schedule disabled" toast and the button label changes to "Disabled". Clicking "Disabled" shows a "Schedule enabled" toast and the label reverts to "Enabled". The schedule is deleted as cleanup.

---

## database-backups.spec.ts — Backup Destination Lifecycle and Restore Staging

**File:** `frontend/tests/e2e/database-backups.spec.ts`

This spec validates the complete backup management workflow against a live guest.

**Add a backup destination:** The "Add Destination" form is opened and the Browse button opens the path picker dialog. The backup path from the fixture is entered into the selected path field and confirmed. After adding the drive label and clicking "Create Destination", a success toast appears and the destination row shows the backup path.

**Configure automatic backup settings:** The Settings panel is opened. Automatic backups are enabled with a 1-hour interval. Automatic integrity checks are enabled with a 2-hour interval. After saving, a success toast confirms the settings are persisted.

**Run a manual backup:** Clicking "Run Backup Now" triggers an immediate backup to all enabled destinations. A toast confirms the result. The test then verifies on the guest that the backup file exists at the expected backup path.

**Check backup integrity:** Clicking "Check Integrity Now" verifies the most recent backup file. A toast confirms the check completed. The "Latest Integrity Check" section becomes visible on the page.

**Stage a restore:** Clicking "Restore Older Backup" opens the restore form. The Browse button opens the file picker and the backup file path is entered into the "Selected Path" field, then confirmed with "Use Backup File". Clicking "Stage Restore" shows a "Restore staged; restart BitProtector to apply it" toast. The "Restore Staged" indicator becomes visible, confirming the UI reflects the staged state.
