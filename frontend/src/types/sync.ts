export type SyncStatus = 'pending' | 'in_progress' | 'completed' | 'failed'
export type SyncAction =
  | 'mirror'
  | 'restore_master'
  | 'restore_mirror'
  | 'verify'
  | 'adopt_mirror'
  | 'user_action_required'

export type SyncResolution = 'keep_master' | 'keep_mirror' | 'provide_new'

export interface SyncQueueItem {
  id: number
  tracked_file_id: number
  relative_path: string
  action: SyncAction
  status: SyncStatus
  error_message: string | null
  created_at: string
  completed_at: string | null
}

export interface AddQueueItemRequest {
  tracked_file_id: number
  action: SyncAction
}

export interface ResolveQueueItemRequest {
  resolution: SyncResolution
  new_file_path?: string
}

/** Response from POST /sync/process (202 Accepted). */
export interface ProcessQueueStarted {
  status: 'started'
}

export interface ClearCompletedQueueResult {
  deleted: number
}

export interface QueuePausedResult {
  queue_paused: boolean
}

export interface SyncQueueListResponse {
  queue: SyncQueueItem[]
  total: number
  page: number
  per_page: number
  queue_paused: boolean
  active_items: number
  in_progress_items: number
  pending_items: number
  completed_items: number
  failed_items: number
}

/**
 * Live summary pushed over the SSE stream at `/sync/queue/stream`.
 * Carries queue counts, processing state, and folder-scan progress so
 * the frontend no longer needs a separate folders poll.
 */
export interface SyncSummary {
  total: number
  active_items: number
  pending_items: number
  in_progress_items: number
  completed_items: number
  failed_items: number
  queue_paused: boolean
  /** True while a background `POST /sync/process` worker is running. */
  processing_active: boolean
  /** True when at least one tracked folder is currently being scanned. */
  scanning: boolean
  scan_total_files: number
  scan_scanned_files: number
  scan_active_folders: number
  /** Monotonically increasing counter; incremented on every publish. */
  revision: number
}
