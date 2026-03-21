export interface TrackedFile {
  id: number
  drive_pair_id: number
  relative_path: string
  checksum: string | null
  file_size: number | null
  virtual_path: string | null
  is_mirrored: boolean
  last_verified: string | null
  created_at: string
  updated_at: string
}

export interface TrackedFileListResponse {
  files: TrackedFile[]
  total: number
  page: number
  per_page: number
}

export interface TrackFileRequest {
  drive_pair_id: number
  relative_path: string
}

export interface FilesQueryParams {
  drive_id?: number
  virtual_prefix?: string
  mirrored?: boolean
  page?: number
  per_page?: number
}
