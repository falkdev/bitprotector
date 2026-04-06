import { apiClient } from './client'
import type { IntegrityRun, IntegrityRunResultsResponse, SingleIntegrityResult } from '@/types/integrity'

export const integrityApi = {
  checkFile(id: number, recover = true): Promise<SingleIntegrityResult> {
    return apiClient
      .post<SingleIntegrityResult>(`/integrity/check/${id}`, null, { params: { recover } })
      .then((r) => r.data)
  },

  startRun(driveId?: number, recover = true): Promise<IntegrityRun> {
    return apiClient
      .post<IntegrityRun>('/integrity/runs', { drive_id: driveId, recover })
      .then((r) => r.data)
  },

  activeRun(): Promise<{ run: IntegrityRun | null }> {
    return apiClient
      .get<{ run: IntegrityRun | null }>('/integrity/runs/active')
      .then((r) => r.data)
  },

  stopRun(id: number): Promise<IntegrityRun> {
    return apiClient
      .post<IntegrityRun>(`/integrity/runs/${id}/stop`)
      .then((r) => r.data)
  },

  latestResults(options?: {
    issues_only?: boolean
    page?: number
    per_page?: number
  }): Promise<IntegrityRunResultsResponse> {
    return apiClient
      .get<IntegrityRunResultsResponse>('/integrity/runs/latest', { params: options })
      .then((r) => r.data)
  },

  runResults(
    id: number,
    options?: { issues_only?: boolean; page?: number; per_page?: number }
  ): Promise<IntegrityRunResultsResponse> {
    return apiClient
      .get<IntegrityRunResultsResponse>(`/integrity/runs/${id}/results`, { params: options })
      .then((r) => r.data)
  },
}
