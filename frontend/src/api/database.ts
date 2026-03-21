import { apiClient } from './client'
import type {
  DbBackupConfig,
  CreateBackupConfigRequest,
  UpdateBackupConfigRequest,
  RunBackupResult,
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

  runBackup(dbPath: string): Promise<RunBackupResult[]> {
    return apiClient
      .post<RunBackupResult[]>('/database/backups/run', null, { params: { db_path: dbPath } })
      .then((r) => r.data)
  },
}
