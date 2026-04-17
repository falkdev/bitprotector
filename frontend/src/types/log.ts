export type EventType =
  | 'file_created'
  | 'file_edited'
  | 'file_mirrored'
  | 'file_untracked'
  | 'integrity_pass'
  | 'integrity_fail'
  | 'recovery_success'
  | 'recovery_fail'
  | 'both_corrupted'
  | 'change_detected'
  | 'sync_completed'
  | 'sync_failed'
  | 'folder_tracked'
  | 'folder_untracked'
  | 'integrity_run_started'
  | 'integrity_run_completed'
  | 'drive_created'
  | 'drive_updated'
  | 'drive_deleted'
  | 'drive_failover'
  | 'drive_quiescing'
  | 'drive_quiesce_cancelled'
  | 'drive_failure_confirmed'
  | 'drive_replacement_assigned'
  | 'drive_rebuild_completed'

export interface EventLogEntry {
  id: number
  event_type: EventType
  tracked_file_id: number | null
  file_path: string | null
  message: string
  details: string | null
  created_at: string
}

export interface LogsQueryParams {
  event_type?: EventType
  file_id?: number
  from?: string
  to?: string
  page?: number
  per_page?: number
}
