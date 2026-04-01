export interface SetVirtualPathRequest {
  virtual_path: string
}

export interface BulkAssignEntry {
  file_id: number
  virtual_path: string
}

export interface BulkAssignRequest {
  entries: BulkAssignEntry[]
}

export interface BulkFromRealRequest {
  drive_pair_id: number
  folder_path: string
  publish_root: string
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
