import type { DbBackupConfig, RunBackupResult } from '@/types/database'
import type { DrivePair } from '@/types/drive'
import type { TrackedFile, TrackedFileListResponse } from '@/types/file'
import type { TrackedFolder, ScanFolderResult } from '@/types/folder'
import type { BatchIntegrityResult, SingleIntegrityResult } from '@/types/integrity'
import type { EventLogEntry } from '@/types/log'
import type { ScheduleConfig } from '@/types/scheduler'
import type { SystemStatus } from '@/types/status'
import type { SyncQueueItem } from '@/types/sync'
import type { TrackingItem, TrackingListResponse } from '@/types/tracking'
import type { BulkAssignResult, RefreshSymlinksResult } from '@/types/virtual-path'

const DEFAULT_DATE = '2026-01-01T00:00:00Z'

export function makeDrivePair(overrides: Partial<DrivePair> = {}): DrivePair {
  return {
    id: 1,
    name: 'Primary Mirror',
    primary_path: '/mnt/primary',
    secondary_path: '/mnt/mirror',
    primary_state: 'active',
    secondary_state: 'active',
    active_role: 'primary',
    created_at: DEFAULT_DATE,
    updated_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeTrackedFolder(overrides: Partial<TrackedFolder> = {}): TrackedFolder {
  return {
    id: 1,
    drive_pair_id: 1,
    folder_path: 'documents',
    virtual_path: null,
    created_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeScanFolderResult(overrides: Partial<ScanFolderResult> = {}): ScanFolderResult {
  return {
    new_files: 2,
    changed_files: 1,
    ...overrides,
  }
}

export function makeTrackedFile(overrides: Partial<TrackedFile> = {}): TrackedFile {
  return {
    id: 1,
    drive_pair_id: 1,
    relative_path: 'documents/report.pdf',
    checksum: 'abc123',
    file_size: 1024,
    virtual_path: null,
    is_mirrored: false,
    tracked_direct: true,
    tracked_via_folder: false,
    last_verified: DEFAULT_DATE,
    created_at: DEFAULT_DATE,
    updated_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeTrackedFileListResponse(
  files: TrackedFile[],
  overrides: Partial<TrackedFileListResponse> = {}
): TrackedFileListResponse {
  return {
    files,
    total: files.length,
    page: 1,
    per_page: 50,
    ...overrides,
  }
}

export function makeTrackingItem(overrides: Partial<TrackingItem> = {}): TrackingItem {
  return {
    kind: 'file',
    id: 1,
    drive_pair_id: 1,
    path: 'documents/report.pdf',
    virtual_path: null,
    is_mirrored: false,
    tracked_direct: true,
    tracked_via_folder: false,
    source: 'direct',
    created_at: DEFAULT_DATE,
    updated_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeTrackingListResponse(
  items: TrackingItem[],
  overrides: Partial<TrackingListResponse> = {}
): TrackingListResponse {
  return {
    items,
    total: items.length,
    page: 1,
    per_page: 50,
    ...overrides,
  }
}

export function makeSystemStatus(overrides: Partial<SystemStatus> = {}): SystemStatus {
  return {
    files_tracked: 3,
    files_mirrored: 2,
    pending_sync: 1,
    integrity_issues: 0,
    drive_pairs: 1,
    degraded_pairs: 0,
    active_secondary_pairs: 0,
    rebuilding_pairs: 0,
    quiescing_pairs: 0,
    ...overrides,
  }
}

export function makeIntegrityResult(overrides: Partial<BatchIntegrityResult> = {}): BatchIntegrityResult {
  return {
    file_id: 1,
    status: 'ok',
    recovered: false,
    ...overrides,
  }
}

export function makeSingleIntegrityResult(
  overrides: Partial<SingleIntegrityResult> = {}
): SingleIntegrityResult {
  return {
    file_id: 1,
    status: 'ok',
    master_valid: true,
    mirror_valid: true,
    recovered: false,
    ...overrides,
  }
}

export function makeLogEntry(overrides: Partial<EventLogEntry> = {}): EventLogEntry {
  return {
    id: 1,
    event_type: 'file_created',
    tracked_file_id: 1,
    message: 'Tracked file created',
    details: '{"source":"test"}',
    created_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeSyncQueueItem(overrides: Partial<SyncQueueItem> = {}): SyncQueueItem {
  return {
    id: 1,
    tracked_file_id: 1,
    action: 'mirror',
    status: 'pending',
    error_message: null,
    created_at: DEFAULT_DATE,
    completed_at: null,
    ...overrides,
  }
}

export function makeBackupConfig(overrides: Partial<DbBackupConfig> = {}): DbBackupConfig {
  return {
    id: 1,
    backup_path: '/mnt/backups/bitprotector',
    drive_label: 'usb-backup-1',
    max_copies: 5,
    enabled: true,
    last_backup: DEFAULT_DATE,
    created_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeRunBackupResult(overrides: Partial<RunBackupResult> = {}): RunBackupResult {
  return {
    backup_config_id: 1,
    backup_path: '/mnt/backups/bitprotector',
    status: 'success',
    error: null,
    ...overrides,
  }
}

export function makeSchedule(overrides: Partial<ScheduleConfig> = {}): ScheduleConfig {
  return {
    id: 1,
    task_type: 'sync',
    cron_expr: '0 2 * * *',
    interval_seconds: null,
    enabled: true,
    last_run: DEFAULT_DATE,
    next_run: '2026-01-02T00:00:00Z',
    created_at: DEFAULT_DATE,
    updated_at: DEFAULT_DATE,
    ...overrides,
  }
}

export function makeBulkAssignResult(overrides: Partial<BulkAssignResult> = {}): BulkAssignResult {
  return {
    succeeded: [1],
    failed: [],
    ...overrides,
  }
}

export function makeRefreshSymlinksResult(
  overrides: Partial<RefreshSymlinksResult> = {}
): RefreshSymlinksResult {
  return {
    created: 2,
    removed: 1,
    errors: [],
    ...overrides,
  }
}
