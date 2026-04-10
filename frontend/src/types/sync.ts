export type SyncStatus = 'pending' | 'in_progress' | 'completed' | 'failed'
export type SyncAction =
  | 'mirror'
  | 'restore_master'
  | 'restore_mirror'
  | 'verify'
  | 'user_action_required'

export type SyncResolution = 'keep_master' | 'keep_mirror' | 'provide_new'

export interface SyncQueueItem {
  id: number
  tracked_file_id: number
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

export interface ProcessQueueResult {
  processed: number
}

export interface ClearCompletedQueueResult {
  deleted: number
}
