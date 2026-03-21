export interface SetVirtualPathRequest {
  virtual_path: string
  symlink_base?: string
}

export interface BulkAssignEntry {
  file_id: number
  virtual_path: string
}

export interface BulkAssignRequest {
  entries: BulkAssignEntry[]
  symlink_base?: string
}

export interface BulkFromRealRequest {
  drive_pair_id: number
  folder_path: string
  virtual_base: string
  symlink_base?: string
}

export interface BulkAssignResult {
  succeeded: number[]
  failed: Array<{ file_id: number; error: string }>
}

export interface RefreshSymlinksResult {
  created: number
  removed: number
  errors: string[]
}
