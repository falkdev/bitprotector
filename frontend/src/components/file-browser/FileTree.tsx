import { ChevronRight, Folder, FolderOpen } from 'lucide-react'
import { cn } from '@/lib/utils'

interface TreeNode {
  name: string
  path: string
  children: TreeNode[]
  fileCount: number
}

function buildTree(paths: string[]): TreeNode[] {
  const root: TreeNode = { name: '', path: '', children: [], fileCount: 0 }

  for (const p of paths) {
    if (!p) continue
    const parts = p.split('/').filter(Boolean)
    let node = root
    let currentPath = ''
    for (const part of parts) {
      currentPath = currentPath ? `${currentPath}/${part}` : part
      let child = node.children.find((c) => c.name === part)
      if (!child) {
        child = { name: part, path: currentPath, children: [], fileCount: 0 }
        node.children.push(child)
      }
      child.fileCount++
      node = child
    }
  }

  return root.children
}

interface FileTreeNodeProps {
  node: TreeNode
  selected: string
  onSelect: (path: string) => void
  depth: number
}

function FileTreeNode({ node, selected, onSelect, depth }: FileTreeNodeProps) {
  const isSelected = selected === node.path
  const hasChildren = node.children.length > 0

  return (
    <div>
      <button
        className={cn(
          'flex w-full items-center gap-1 rounded px-2 py-1 text-sm text-left hover:bg-gray-100',
          isSelected && 'bg-blue-50 text-blue-700 font-medium'
        )}
        style={{ paddingLeft: `${8 + depth * 16}px` }}
        onClick={() => onSelect(node.path)}
        data-testid={`tree-node-${node.path}`}
      >
        {hasChildren ? (
          isSelected ? (
            <FolderOpen className="h-4 w-4 shrink-0 text-yellow-500" />
          ) : (
            <Folder className="h-4 w-4 shrink-0 text-yellow-500" />
          )
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0 text-gray-400" />
        )}
        <span className="truncate">{node.name}</span>
        <span className="ml-auto text-xs text-gray-400">{node.fileCount}</span>
      </button>
      {isSelected && hasChildren && (
        <div>
          {node.children.map((child) => (
            <FileTreeNode
              key={child.path}
              node={child}
              selected={selected}
              onSelect={onSelect}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  )
}

interface FileTreeProps {
  virtualPaths: string[]
  selected: string
  onSelect: (path: string) => void
}

export function FileTree({ virtualPaths, selected, onSelect }: FileTreeProps) {
  const nodes = buildTree(virtualPaths)

  if (nodes.length === 0) {
    return (
      <div className="p-4 text-sm text-gray-500 text-center">
        No publish paths assigned
      </div>
    )
  }

  return (
    <div className="space-y-0.5 p-2" data-testid="file-tree">
      <button
        className={cn(
          'flex w-full items-center gap-1 rounded px-2 py-1 text-sm font-medium hover:bg-gray-100',
          selected === '' && 'bg-blue-50 text-blue-700'
        )}
        onClick={() => onSelect('')}
      >
        <Folder className="h-4 w-4 shrink-0 text-yellow-500" />
        All files
        <span className="ml-auto text-xs text-gray-400">{virtualPaths.length}</span>
      </button>
      {nodes.map((node) => (
        <FileTreeNode
          key={node.path}
          node={node}
          selected={selected}
          onSelect={onSelect}
          depth={0}
        />
      ))}
    </div>
  )
}
