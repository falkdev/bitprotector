export interface TrackedFolder {
  id: number
  drive_pair_id: number
  folder_path: string
  virtual_path: string | null
  include_checksum_sidecars: boolean
  scanning: boolean
  scan_scanned_files: number
  scan_total_files: number
  mirroring: boolean
  deleting: boolean
  mirror_mirrored_files: number
  mirror_total_files: number
  last_scanned_at: string | null
  last_mirrored_at: string | null
  created_at: string
}

export interface CreateFolderRequest {
  drive_pair_id: number
  folder_path: string
  virtual_path?: string | null
  include_checksum_sidecars?: boolean
}

export interface UpdateFolderRequest {
  virtual_path?: string | null
}

export interface FolderScanStatus {
  scanning: boolean
  scanned: number
  total: number
}

export interface MirrorFolderResult {
  mirrored_files: number
  mirroring: boolean
  mirrored: number
  total: number
}

export interface FolderMirrorStatus {
  mirroring: boolean
  mirrored: number
  total: number
}
