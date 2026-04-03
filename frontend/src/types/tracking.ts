export type TrackingItemKind = 'file' | 'folder'
export type TrackingSource = 'direct' | 'folder' | 'both'

export interface TrackingItem {
  kind: TrackingItemKind
  id: number
  drive_pair_id: number
  path: string
  virtual_path: string | null
  is_mirrored: boolean | null
  tracked_direct: boolean | null
  tracked_via_folder: boolean | null
  source: TrackingSource
  created_at: string
  updated_at: string
}

export interface TrackingListResponse {
  items: TrackingItem[]
  total: number
  page: number
  per_page: number
}

export interface TrackingListParams {
  drive_id?: number
  q?: string
  virtual_prefix?: string
  has_virtual_path?: boolean
  item_kind?: 'file' | 'folder' | 'all'
  source?: 'direct' | 'folder' | 'both' | 'all'
  page?: number
  per_page?: number
}
