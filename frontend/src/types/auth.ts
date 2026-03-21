export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  token: string
  username: string
  /** ISO 8601 / RFC 3339 timestamp when the token expires (24 h from login) */
  expires_at: string
}

export interface ValidateResponse {
  username: string
  valid: boolean
}
