export interface DbBackupConfig {
  id: number
  backup_path: string
  drive_label: string | null
  priority: number
  enabled: boolean
  last_backup: string | null
  last_integrity_check: string | null
  last_integrity_status: string | null
  last_error: string | null
  created_at: string
}

export interface CreateBackupConfigRequest {
  backup_path: string
  drive_label?: string
  enabled?: boolean
}

export interface UpdateBackupConfigRequest {
  backup_path?: string
  drive_label?: string | null
  enabled?: boolean
}

export interface RunBackupResult {
  backup_config_id: number
  backup_path: string
  status: 'success' | 'failed'
  error: string | null
}

export interface DbBackupSettings {
  backup_enabled: boolean
  backup_interval_seconds: number
  integrity_enabled: boolean
  integrity_interval_seconds: number
  last_backup_run: string | null
  last_integrity_run: string | null
  updated_at: string
}

export interface UpdateBackupSettingsRequest {
  backup_enabled?: boolean
  backup_interval_seconds?: number
  integrity_enabled?: boolean
  integrity_interval_seconds?: number
}

export interface BackupIntegrityResult {
  backup_config_id: number
  backup_path: string
  status: 'ok' | 'repaired' | 'missing' | 'corrupt' | 'failed'
  checksum: string | null
  repaired_from_id: number | null
  error: string | null
}

export interface RestoreBackupRequest {
  source_path: string
}

export interface RestoreBackupResult {
  status: string
  restart_required: boolean
  safety_backup_path: string
  staged_restore_path: string
}
