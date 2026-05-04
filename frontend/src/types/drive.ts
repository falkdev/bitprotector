export type DriveState = 'active' | 'quiescing' | 'failed' | 'rebuilding'
export type DriveRole = 'primary' | 'secondary'

export interface DrivePair {
  id: number
  name: string
  primary_path: string
  secondary_path: string
  primary_media_type: 'ssd' | 'hdd'
  secondary_media_type: 'ssd' | 'hdd'
  primary_state: DriveState
  secondary_state: DriveState
  active_role: DriveRole
  created_at: string
  updated_at: string
}

export interface CreateDrivePairRequest {
  name: string
  primary_path: string
  secondary_path: string
  primary_media_type?: 'ssd' | 'hdd'
  secondary_media_type?: 'ssd' | 'hdd'
  skip_validation?: boolean
}

export interface UpdateDrivePairRequest {
  name?: string
  primary_path?: string
  secondary_path?: string
  primary_media_type?: 'ssd' | 'hdd'
  secondary_media_type?: 'ssd' | 'hdd'
}

export interface MarkReplacementRequest {
  role: DriveRole
}

export interface AssignReplacementRequest {
  role: DriveRole
  new_path: string
  skip_validation?: boolean
}

export interface AssignReplacementResponse {
  drive_pair: DrivePair
  queued_rebuild_items: number
}
