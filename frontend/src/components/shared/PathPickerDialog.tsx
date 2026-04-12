import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ChevronRight, Eye, EyeOff, FileText, Folder, FolderOpen, RefreshCw, X } from 'lucide-react'
import { Tree, type NodeRendererProps, type TreeApi } from 'react-arborist'
import { filesystemApi } from '@/api/filesystem'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { normalizeAbsoluteFilesystemPath } from '@/lib/path'
import { cn } from '@/lib/utils'
import type { FilesystemEntry, FilesystemEntryKind } from '@/types/filesystem'

type PickerMode = 'file' | 'directory'

interface PickerNode {
  id: string
  name: string
  path: string
  kind: FilesystemEntryKind
  isHidden: boolean
  isSelectable: boolean
  hasChildren: boolean
  isLoaded?: boolean
  children?: PickerNode[]
}

interface PathPickerDialogProps {
  open: boolean
  title: string
  description?: string
  mode: PickerMode
  value: string
  startPath?: string
  rootPath?: string
  confirmLabel?: string
  onClose: () => void
  onPick: (path: string) => void
  validatePath?: (path: string) => string | null
}

function createRootNode(rootPath: string): PickerNode {
  return {
    id: rootPath,
    name: rootPath === '/' ? '/' : rootPath,
    path: rootPath,
    kind: 'directory',
    isHidden: false,
    isSelectable: true,
    hasChildren: true,
    isLoaded: false,
    children: [],
  }
}

function toNode(entry: FilesystemEntry): PickerNode {
  return {
    id: entry.path,
    name: entry.name,
    path: entry.path,
    kind: entry.kind,
    isHidden: entry.is_hidden,
    isSelectable: entry.is_selectable,
    hasChildren: entry.has_children,
    isLoaded: false,
    children: entry.kind === 'directory' ? [] : undefined,
  }
}

function findNode(nodes: PickerNode[], path: string): PickerNode | null {
  for (const node of nodes) {
    if (node.path === path) {
      return node
    }
    if (node.children) {
      const nested = findNode(node.children, path)
      if (nested) {
        return nested
      }
    }
  }

  return null
}

function replaceChildren(nodes: PickerNode[], targetPath: string, children: PickerNode[]): PickerNode[] {
  return nodes.map((node) => {
    if (node.path === targetPath) {
      return {
        ...node,
        hasChildren: children.length > 0,
        isLoaded: true,
        children,
      }
    }

    if (!node.children) {
      return node
    }

    return {
      ...node,
      children: replaceChildren(node.children, targetPath, children),
    }
  })
}

function isPathInsideRoot(path: string, rootPath: string): boolean {
  if (rootPath === '/') return path.startsWith('/')
  return path === rootPath || path.startsWith(`${rootPath}/`)
}

function buildAncestorChainWithinRoot(path: string, rootPath: string): string[] {
  if (!path.startsWith('/')) {
    return [rootPath]
  }

  if (rootPath === '/') {
    if (path === '/') {
      return ['/']
    }

    const parts = path.split('/').filter(Boolean)
    const ancestors = ['/']
    let current = ''

    for (const part of parts) {
      current = `${current}/${part}`
      ancestors.push(current)
    }

    return ancestors
  }

  if (!isPathInsideRoot(path, rootPath)) {
    return [rootPath]
  }

  const suffix = path.slice(rootPath.length).split('/').filter(Boolean)
  const ancestors = [rootPath]
  let current = rootPath

  for (const part of suffix) {
    current = `${current}/${part}`
    ancestors.push(current)
  }

  return ancestors
}

function nextTick() {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, 0)
  })
}

export function PathPickerDialog({
  open,
  title,
  description,
  mode,
  value,
  startPath,
  rootPath,
  confirmLabel = 'Use Path',
  onClose,
  onPick,
  validatePath,
}: PathPickerDialogProps) {
  const scopedRootPath = useMemo(
    () => normalizeAbsoluteFilesystemPath(rootPath?.trim() || '/'),
    [rootPath]
  )
  const treeRef = useRef<TreeApi<PickerNode> | null>(null)
  const nodesRef = useRef<PickerNode[]>([createRootNode(scopedRootPath)])
  const loadingPathsRef = useRef<Set<string>>(new Set())
  const loadingPromisesRef = useRef<Map<string, Promise<void>>>(new Map())
  const [treeData, setTreeData] = useState<PickerNode[]>([createRootNode(scopedRootPath)])
  const [treeKey, setTreeKey] = useState(0)
  const [draftPath, setDraftPath] = useState(value)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [selectedKind, setSelectedKind] = useState<FilesystemEntryKind | null>(null)
  const [showHidden, setShowHidden] = useState(false)
  const [loading, setLoading] = useState(false)
  const [loadingPaths, setLoadingPaths] = useState<string[]>([])
  const [treeError, setTreeError] = useState<string | null>(null)

  useEffect(() => {
    nodesRef.current = treeData
  }, [treeData])

  const validateSelectionError = useMemo(() => {
    if (!draftPath.trim()) {
      return 'Path is required'
    }

    if (
      mode === 'file' &&
      selectedPath === draftPath.trim() &&
      selectedKind === 'directory'
    ) {
      return 'Select a file, not a folder'
    }

    return validatePath?.(draftPath.trim()) ?? null
  }, [draftPath, mode, selectedKind, selectedPath, validatePath])

  const loadChildren = useCallback(async (path: string, includeHidden: boolean, force = false) => {
    const existingNode = findNode(nodesRef.current, path)
    if (existingNode?.isLoaded && !force) {
      return
    }
    if (!force) {
      const inFlight = loadingPromisesRef.current.get(path)
      if (inFlight) {
        return inFlight
      }
    }

    const request = (async () => {
      loadingPathsRef.current.add(path)
      setLoadingPaths((current) => [...current, path])
      try {
        const response = await filesystemApi.children({
          path,
          include_hidden: includeHidden,
          directories_only: mode === 'directory',
        })
        const children = response.entries.map(toNode)
        setTreeData((current) =>
          path === scopedRootPath ? children : replaceChildren(current, path, children)
        )
      } finally {
        loadingPathsRef.current.delete(path)
        loadingPromisesRef.current.delete(path)
        setLoadingPaths((current) => current.filter((entry) => entry !== path))
      }
    })()

    loadingPromisesRef.current.set(path, request)
    return request
  }, [mode, scopedRootPath])

  const renderNode = useCallback(({ node, style }: NodeRendererProps<PickerNode>) => {
    const isDirectory = node.data.kind === 'directory'

    return (
      <div style={style}>
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation()
            node.handleClick(event)

            if (!isDirectory) {
              return
            }

            void (async () => {
              if (node.isOpen) {
                node.close()
                return
              }

              const currentNode = findNode(nodesRef.current, node.data.path)
              if (!currentNode?.isLoaded) {
                await loadChildren(node.data.path, showHidden)
              }

              node.open()
            })()
          }}
          className={cn(
            'flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors',
            node.isSelected ? 'bg-primary/10 text-primary' : 'hover:bg-accent'
          )}
          data-testid={`path-picker-node-${node.data.path}`}
        >
          <span className="flex w-4 shrink-0 items-center justify-center text-muted-foreground">
            {isDirectory ? (
              <ChevronRight
                className={cn('h-4 w-4 transition-transform', node.isOpen && 'rotate-90')}
              />
            ) : null}
          </span>
          {isDirectory ? (
            node.isOpen ? (
              <FolderOpen className="h-4 w-4 shrink-0 text-yellow-500" />
            ) : (
              <Folder className="h-4 w-4 shrink-0 text-yellow-500" />
            )
          ) : (
            <FileText className="h-4 w-4 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate">{node.data.name}</span>
        </button>
      </div>
    )
  }, [loadChildren, showHidden])

  const initializeTree = useCallback(async (initialPath: string, includeHidden: boolean) => {
    const normalizedInitialPath = normalizeAbsoluteFilesystemPath(initialPath || scopedRootPath)
    const scopedInitialPath = isPathInsideRoot(normalizedInitialPath, scopedRootPath)
      ? normalizedInitialPath
      : scopedRootPath

    setLoading(true)
    setTreeError(null)
    const nextTree = [createRootNode(scopedRootPath)]
    nodesRef.current = nextTree
    loadingPathsRef.current.clear()
    setTreeData(nextTree)
    setTreeKey((current) => current + 1)
    setSelectedPath(null)
    setSelectedKind(null)
    setDraftPath(scopedInitialPath)

    try {
      await nextTick()
      await loadChildren(scopedRootPath, includeHidden, true)
      await nextTick()
      treeRef.current?.open(scopedRootPath)

      const directoryTarget = mode === 'file'
        ? scopedInitialPath.split('/').slice(0, -1).join('/') || scopedRootPath
        : scopedInitialPath
      const ancestors = buildAncestorChainWithinRoot(directoryTarget, scopedRootPath)

      for (const ancestor of ancestors.slice(1)) {
        await loadChildren(ancestor, includeHidden, true)
        await nextTick()
        treeRef.current?.open(ancestor)
      }

      setSelectedPath(scopedInitialPath)
      const initialNode = findNode(nodesRef.current, scopedInitialPath)
      setSelectedKind(initialNode?.kind ?? null)
    } catch (error) {
      setTreeError(error instanceof Error ? error.message : 'Failed to load filesystem browser')
    } finally {
      setLoading(false)
    }
  }, [loadChildren, mode, scopedRootPath])

  useEffect(() => {
    if (!open) {
      return
    }

    setShowHidden(false)
    void initializeTree((startPath && startPath.trim()) || value.trim() || scopedRootPath, false)
  }, [initializeTree, open, scopedRootPath, startPath, value])

  if (!open) {
    return null
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4">
      <div className="flex h-[42rem] w-full max-w-4xl flex-col rounded-xl border border-border bg-card shadow-lg">
        <div className="flex items-start justify-between border-b border-border px-6 py-4">
          <div className="space-y-1">
            <h2 className="text-lg font-semibold">{title}</h2>
            {description ? (
              <p className="text-sm text-muted-foreground">{description}</p>
            ) : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            aria-label="Close path picker"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4 border-b border-border px-6 py-4">
          <div>
            <label htmlFor="path-picker-path" className="mb-1 block text-sm font-medium">
              Selected Path
            </label>
            <input
              id="path-picker-path"
              value={draftPath}
              onChange={(event) => setDraftPath(event.target.value)}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
              placeholder={mode === 'file' ? '/path/to/file' : '/path/to/folder'}
            />
          </div>
          <div className="flex items-center justify-between gap-3">
            <div className="min-h-[1.25rem] text-xs text-destructive">
              {validateSelectionError}
            </div>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => void initializeTree(draftPath.trim() || scopedRootPath, showHidden)}
                className="inline-flex items-center gap-1 rounded-md border border-border px-3 py-2 text-xs font-medium transition-colors hover:bg-accent"
              >
                <RefreshCw className="h-3.5 w-3.5" />
                Refresh
              </button>
              <button
                type="button"
                onClick={() => {
                  const nextShowHidden = !showHidden
                  setShowHidden(nextShowHidden)
                  void initializeTree(draftPath.trim() || scopedRootPath, nextShowHidden)
                }}
                className="inline-flex items-center gap-1 rounded-md border border-border px-3 py-2 text-xs font-medium transition-colors hover:bg-accent"
              >
                {showHidden ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
                {showHidden ? 'Hide Hidden' : 'Show Hidden'}
              </button>
            </div>
          </div>
        </div>

        <div className="relative flex-1 overflow-hidden px-4 py-4">
          {loading ? (
            <div className="flex h-full items-center justify-center">
              <LoadingSpinner />
            </div>
          ) : (
            <div className="h-full overflow-hidden rounded-lg border border-border bg-background">
              {treeError ? (
                <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
                  <p className="text-sm text-destructive">{treeError}</p>
                  <p className="text-xs text-muted-foreground">
                    You can still type a path above and confirm it manually.
                  </p>
                </div>
              ) : (
                <Tree<PickerNode>
                  key={treeKey}
                  ref={treeRef}
                  data={treeData}
                  width="100%"
                  height={420}
                  rowHeight={36}
                  indent={20}
                  padding={8}
                  openByDefault={false}
                  disableDrag
                  disableEdit
                  disableMultiSelection
                  selection={selectedPath ?? undefined}
                  onToggle={(path) => {
                    const node = findNode(nodesRef.current, path)
                    if (node?.kind === 'directory' && treeRef.current?.isOpen(path)) {
                      void loadChildren(path, showHidden)
                    }
                  }}
                  onSelect={(nodes) => {
                    const selected = nodes[0]?.data
                    if (!selected) {
                      setSelectedPath(null)
                      setSelectedKind(null)
                      return
                    }

                    setSelectedPath(selected.path)
                    setSelectedKind(selected.kind)
                    setDraftPath(selected.path)
                  }}
                  className="h-full"
                  rowClassName="focus:outline-none"
                >
                  {renderNode}
                </Tree>
              )}
            </div>
          )}
          {loadingPaths.length > 0 ? (
            <div className="pointer-events-none absolute bottom-6 right-6 rounded-full border border-border bg-card px-3 py-2 text-xs text-muted-foreground shadow">
              Loading…
            </div>
          ) : null}
        </div>

        <div className="flex justify-end gap-2 border-t border-border px-6 py-4">
          <button
            type="button"
            onClick={onClose}
            className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => onPick(draftPath.trim())}
            disabled={!!validateSelectionError || !draftPath.trim()}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}
