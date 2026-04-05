export interface TrackedFolder {
  id: number
  drive_pair_id: number
  folder_path: string
  virtual_path: string | null
  created_at: string
}

export interface CreateFolderRequest {
  drive_pair_id: number
  folder_path: string
  virtual_path?: string
}

export interface UpdateFolderRequest {
  virtual_path?: string | null
}

export interface ScanFolderResult {
  new_files: number
  changed_files: number
}

export interface MirrorFolderResult {
  mirrored_files: number
}
