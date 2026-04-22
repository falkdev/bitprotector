import { apiClient } from './client'
import type {
  ScheduleConfig,
  ScheduleListResponse,
  CreateScheduleRequest,
  UpdateScheduleRequest,
} from '@/types/scheduler'

export const schedulerApi = {
  list(): Promise<ScheduleConfig[]> {
    return apiClient.get<ScheduleListResponse>('/scheduler/schedules').then((r) => r.data.schedules)
  },

  get(id: number): Promise<ScheduleConfig> {
    return apiClient.get<ScheduleConfig>(`/scheduler/schedules/${id}`).then((r) => r.data)
  },

  create(data: CreateScheduleRequest): Promise<ScheduleConfig> {
    return apiClient.post<ScheduleConfig>('/scheduler/schedules', data).then((r) => r.data)
  },

  update(id: number, data: UpdateScheduleRequest): Promise<ScheduleConfig> {
    return apiClient.put<ScheduleConfig>(`/scheduler/schedules/${id}`, data).then((r) => r.data)
  },

  delete(id: number): Promise<void> {
    return apiClient.delete(`/scheduler/schedules/${id}`).then(() => undefined)
  },
}
