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

export interface BatchIntegrityResult {
  file_id: number
  status: IntegrityStatus
  recovered: boolean
}

export interface CheckAllResponse {
  results: BatchIntegrityResult[]
}
