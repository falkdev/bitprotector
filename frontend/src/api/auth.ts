import { apiClient } from './client'
import type { LoginRequest, LoginResponse, ValidateResponse } from '@/types/auth'

export const authApi = {
  login(data: LoginRequest): Promise<LoginResponse> {
    return apiClient.post<LoginResponse>('/auth/login', data).then((r) => r.data)
  },

  validate(): Promise<ValidateResponse> {
    return apiClient.get<ValidateResponse>('/auth/validate').then((r) => r.data)
  },
}
