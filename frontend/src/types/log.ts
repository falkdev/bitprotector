export type EventType =
  | 'file_created'
  | 'file_edited'
  | 'file_mirrored'
  | 'integrity_pass'
  | 'integrity_fail'
  | 'recovery_success'
  | 'recovery_fail'
  | 'both_corrupted'
  | 'change_detected'
  | 'sync_completed'
  | 'sync_failed'

export interface EventLogEntry {
  id: number
  event_type: EventType
  tracked_file_id: number | null
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
