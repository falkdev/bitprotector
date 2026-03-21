export interface TrackedFolder {
  id: number
  drive_pair_id: number
  folder_path: string
  auto_virtual_path: boolean
  default_virtual_base: string | null
  created_at: string
}

export interface CreateFolderRequest {
  drive_pair_id: number
  folder_path: string
  auto_virtual_path?: boolean
  default_virtual_base?: string
}

export interface ScanFolderResult {
  new_files: number
  changed_files: number
}
