import { apiClient } from './client'
import type { SingleIntegrityResult, CheckAllResponse } from '@/types/integrity'

export const integrityApi = {
  checkFile(id: number, recover = true): Promise<SingleIntegrityResult> {
    return apiClient
      .post<SingleIntegrityResult>(`/integrity/check/${id}`, null, { params: { recover } })
      .then((r) => r.data)
  },

  checkAll(driveId?: number, recover = true): Promise<CheckAllResponse> {
    return apiClient
      .get<CheckAllResponse>('/integrity/check-all', { params: { drive_id: driveId, recover } })
      .then((r) => r.data)
  },
}
