import { apiClient } from './client'
import type {
  DrivePair,
  CreateDrivePairRequest,
  UpdateDrivePairRequest,
  MarkReplacementRequest,
  AssignReplacementRequest,
  AssignReplacementResponse,
} from '@/types/drive'

export const drivesApi = {
  list(): Promise<DrivePair[]> {
    return apiClient.get<DrivePair[]>('/drives').then((r) => r.data)
  },

  get(id: number): Promise<DrivePair> {
    return apiClient.get<DrivePair>(`/drives/${id}`).then((r) => r.data)
  },

  create(data: CreateDrivePairRequest): Promise<DrivePair> {
    return apiClient.post<DrivePair>('/drives', data).then((r) => r.data)
  },

  update(id: number, data: UpdateDrivePairRequest): Promise<DrivePair> {
    return apiClient.put<DrivePair>(`/drives/${id}`, data).then((r) => r.data)
  },

  delete(id: number): Promise<void> {
    return apiClient.delete(`/drives/${id}`).then(() => undefined)
  },

  markReplacement(id: number, data: MarkReplacementRequest): Promise<DrivePair> {
    return apiClient.post<DrivePair>(`/drives/${id}/replacement/mark`, data).then((r) => r.data)
  },

  cancelReplacement(id: number, data: MarkReplacementRequest): Promise<DrivePair> {
    return apiClient.post<DrivePair>(`/drives/${id}/replacement/cancel`, data).then((r) => r.data)
  },

  confirmFailure(id: number, data: MarkReplacementRequest): Promise<DrivePair> {
    return apiClient.post<DrivePair>(`/drives/${id}/replacement/confirm`, data).then((r) => r.data)
  },

  assignReplacement(id: number, data: AssignReplacementRequest): Promise<AssignReplacementResponse> {
    return apiClient.post<AssignReplacementResponse>(`/drives/${id}/replacement/assign`, data).then((r) => r.data)
  },
}
