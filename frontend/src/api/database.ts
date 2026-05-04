import { apiClient } from './client'
import type {
  DbBackupConfig,
  CreateBackupConfigRequest,
  UpdateBackupConfigRequest,
  RunBackupResult,
  DbBackupSettings,
  UpdateBackupSettingsRequest,
  BackupIntegrityResult,
  RestoreBackupRequest,
  RestoreBackupResult,
} from '@/types/database'

export const databaseApi = {
  listBackups(): Promise<DbBackupConfig[]> {
    return apiClient.get<DbBackupConfig[]>('/database/backups').then((r) => r.data)
  },

  getBackup(id: number): Promise<DbBackupConfig> {
    return apiClient.get<DbBackupConfig>(`/database/backups/${id}`).then((r) => r.data)
  },

  createBackup(data: CreateBackupConfigRequest): Promise<DbBackupConfig> {
    return apiClient.post<DbBackupConfig>('/database/backups', data).then((r) => r.data)
  },

  updateBackup(id: number, data: UpdateBackupConfigRequest): Promise<DbBackupConfig> {
    return apiClient.put<DbBackupConfig>(`/database/backups/${id}`, data).then((r) => r.data)
  },

  deleteBackup(id: number): Promise<void> {
    return apiClient.delete(`/database/backups/${id}`).then(() => undefined)
  },

  runBackup(): Promise<RunBackupResult[]> {
    return apiClient.post<RunBackupResult[]>('/database/backups/run').then((r) => r.data)
  },

  getSettings(): Promise<DbBackupSettings> {
    return apiClient.get<DbBackupSettings>('/database/backups/settings').then((r) => r.data)
  },

  updateSettings(data: UpdateBackupSettingsRequest): Promise<DbBackupSettings> {
    return apiClient.put<DbBackupSettings>('/database/backups/settings', data).then((r) => r.data)
  },

  runIntegrityCheck(): Promise<BackupIntegrityResult[]> {
    return apiClient
      .post<BackupIntegrityResult[]>('/database/backups/integrity-check')
      .then((r) => r.data)
  },

  restoreBackup(data: RestoreBackupRequest): Promise<RestoreBackupResult> {
    return apiClient
      .post<RestoreBackupResult>('/database/backups/restore', data)
      .then((r) => r.data)
  },
}
