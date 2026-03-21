export interface DbBackupConfig {
  id: number
  backup_path: string
  drive_label: string | null
  max_copies: number
  enabled: boolean
  last_backup: string | null
  created_at: string
}

export interface CreateBackupConfigRequest {
  backup_path: string
  drive_label?: string
  max_copies?: number
  enabled?: boolean
}

export interface UpdateBackupConfigRequest {
  max_copies?: number
  enabled?: boolean
}

export interface RunBackupResult {
  backup_config_id: number
  backup_path: string
  status: 'success' | 'failed'
  error: string | null
}
