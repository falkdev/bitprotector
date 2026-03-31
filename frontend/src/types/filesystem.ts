export type FilesystemEntryKind = 'directory' | 'file'

export interface FilesystemEntry {
  name: string
  path: string
  kind: FilesystemEntryKind
  is_hidden: boolean
  is_selectable: boolean
  has_children: boolean
}

export interface BrowseFilesystemResponse {
  path: string
  canonical_path: string
  parent_path: string | null
  entries: FilesystemEntry[]
}

export interface BrowseFilesystemParams {
  path?: string
  include_hidden?: boolean
  directories_only?: boolean
}
