export type IntegrityStatus =
  | 'ok'
  | 'master_corrupted'
  | 'mirror_corrupted'
  | 'both_corrupted'
  | 'master_missing'
  | 'mirror_missing'
  | 'primary_drive_unavailable'
  | 'secondary_drive_unavailable'

export interface SingleIntegrityResult {
  file_id: number
  status: IntegrityStatus
  master_valid: boolean
  mirror_valid: boolean
  recovered: boolean
}

export type IntegrityRunStatus = 'running' | 'stopping' | 'stopped' | 'completed' | 'failed'

export interface IntegrityRun {
  id: number
  scope_drive_pair_id: number | null
  recover: boolean
  trigger: string
  status: IntegrityRunStatus
  total_files: number
  processed_files: number
  attention_files: number
  recovered_files: number
  active_workers: number
  stop_requested: boolean
  started_at: string
  ended_at: string | null
  error_message: string | null
}

export interface IntegrityRunResult {
  id: number
  run_id: number
  file_id: number
  drive_pair_id: number
  relative_path: string
  status: IntegrityStatus | 'internal_error'
  recovered: boolean
  needs_attention: boolean
  checked_at: string
}

export interface IntegrityRunResultsResponse {
  run: IntegrityRun | null
  results: IntegrityRunResult[]
  total: number
  page: number
  per_page: number
}
