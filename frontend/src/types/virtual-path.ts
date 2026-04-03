export interface SetVirtualPathRequest {
  virtual_path: string
}

export interface VirtualPathTreeNode {
  name: string
  path: string
  item_count: number
  has_children: boolean
}

export interface VirtualPathTreeResponse {
  parent: string
  children: VirtualPathTreeNode[]
}
