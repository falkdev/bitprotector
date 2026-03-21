// Generic API error shape returned by the backend
export interface ApiError {
  error: {
    code: string
    message: string
  }
}

// Paginated file list response
export interface PaginatedResponse<T> {
  data: T[]
  total: number
  page: number
  per_page: number
}
