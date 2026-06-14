import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { LoaderCircle, PanelLeftClose, PanelLeftOpen, Plus } from 'lucide-react'
import { toast } from 'sonner'
import { drivesApi } from '@/api/drives'
import { filesApi } from '@/api/files'
import { foldersApi } from '@/api/folders'
import { trackingApi } from '@/api/tracking'
import { virtualPathsApi } from '@/api/virtual-paths'
import { FileActions } from '@/components/file-browser/FileActions'
import { FileDetails } from '@/components/file-browser/FileDetails'
import { BreadcrumbNav } from '@/components/file-browser/BreadcrumbNav'
import { ConfirmDialog } from '@/components/shared/ConfirmDialog'
import { DataTable } from '@/components/shared/DataTable'
import { EmptyState } from '@/components/shared/EmptyState'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'
import { ModalLayer } from '@/components/shared/ModalLayer'
import { Pagination } from '@/components/shared/Pagination'
import { PageIntro } from '@/components/shared/PageIntro'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { TrackFileModal } from '@/components/tracking/TrackFileModal'
import { FolderFormModal } from '@/components/tracking/FolderFormModal'
import { formatDate } from '@/lib/format'
import { suggestVirtualPathFromParent } from '@/lib/path'
import { getErrorMessage } from '@/lib/utils'
import type { DrivePair } from '@/types/drive'
import type { TrackedFile, TrackFileRequest } from '@/types/file'
import type { FolderScanStatus, TrackedFolder } from '@/types/folder'
import type { TrackingItem, TrackingListParams } from '@/types/tracking'
import type { VirtualPathTreeNode } from '@/types/virtual-path'

type TreeNode = VirtualPathTreeNode & {
  loaded?: boolean
  loading?: boolean
  children?: TreeNode[]
}

type DetailPostDeleteAction =
  | {
      type: 'open'
      fileId: number
    }
  | {
      type: 'close'
    }

const EMPTY_TRACKING_ITEMS: TrackingItem[] = []

function trackingRowKey(item: TrackingItem): string {
  return `${item.kind}-${item.id}`
}

function nextFileAfterDeletion(
  items: TrackingItem[],
  selectedFileId: number,
  deletedFileIds: Set<number>
): number | null {
  const fileItems = items.filter((item) => item.kind === 'file')
  const selectedIndex = fileItems.findIndex((item) => item.id === selectedFileId)
  if (selectedIndex === -1) {
    return null
  }

  const next = fileItems.slice(selectedIndex + 1).find((item) => !deletedFileIds.has(item.id))
  return next?.id ?? null
}

function updateTreeChildren(nodes: TreeNode[], parent: string, children: TreeNode[]): TreeNode[] {
  return nodes.map((node) => {
    if (node.path === parent) {
      return {
        ...node,
        loaded: true,
        loading: false,
        children,
      }
    }
    if (!node.children) return node
    return {
      ...node,
      children: updateTreeChildren(node.children, parent, children),
    }
  })
}

function setTreeLoading(nodes: TreeNode[], targetPath: string, loading: boolean): TreeNode[] {
  return nodes.map((node) => {
    if (node.path === targetPath) {
      return { ...node, loading }
    }
    if (!node.children) return node
    return {
      ...node,
      children: setTreeLoading(node.children, targetPath, loading),
    }
  })
}

function toTrackedFile(item: TrackingItem): TrackedFile {
  return {
    id: item.id,
    drive_pair_id: item.drive_pair_id,
    relative_path: item.path,
    checksum: null,
    file_size: null,
    virtual_path: item.virtual_path,
    is_mirrored: item.is_mirrored ?? false,
    tracked_direct: item.tracked_direct ?? false,
    tracked_via_folder: item.tracked_via_folder ?? false,
    last_integrity_check_at: null,
    created_at: item.created_at,
    updated_at: item.updated_at,
  }
}

function SourceBadge({ source }: { source: TrackingItem['source'] }) {
  const label = source === 'folder' ? 'Folder' : 'Direct'
  const className =
    source === 'folder'
      ? 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400'
      : 'bg-blue-100 text-blue-700 dark:bg-primary/20 dark:text-primary'

  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${className}`}>{label}</span>
  )
}

function FolderStatusBadge({
  status,
  mirrored,
  total,
}: {
  status: NonNullable<TrackingItem['folder_status']>
  mirrored: number
  total: number
}) {
  const label =
    status === 'not_scanned'
      ? 'Not scanned'
      : status === 'mirrored'
        ? 'Mirrored'
        : status === 'tracked'
          ? 'Tracked'
          : status === 'partial'
            ? 'Partial'
            : 'Empty'
  const ratio =
    status === 'empty' || status === 'not_scanned'
      ? ''
      : status === 'partial'
        ? ` (${mirrored}/${total})`
        : ` (${total}/${total})`
  const className =
    status === 'not_scanned'
      ? 'bg-muted text-muted-foreground'
      : status === 'mirrored'
        ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
        : status === 'tracked'
          ? 'bg-slate-100 text-slate-700 dark:bg-slate-700/40 dark:text-slate-300'
          : status === 'partial'
            ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
            : 'bg-muted text-muted-foreground'

  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${className}`}>
      {label}
      {ratio}
    </span>
  )
}

function FolderScanningStatus({ scanned, total }: { scanned: number; total: number }) {
  if (total > 0) {
    const progress = Math.min(100, Math.max(0, (scanned / total) * 100))

    return (
      <div className="min-w-32 space-y-1">
        <div className="flex items-center gap-2 text-xs font-medium text-blue-700 dark:text-blue-300">
          <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
          <span>Scanning...</span>
          <span className="font-mono">{`${scanned} / ${total}`}</span>
        </div>
        <div className="h-1.5 overflow-hidden rounded-full bg-blue-100 dark:bg-blue-900/30">
          <div
            className="h-full rounded-full bg-blue-600 transition-[width] duration-300 dark:bg-blue-400"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>
    )
  }

  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700 dark:bg-blue-900/30 dark:text-blue-300">
      <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
      Scanning...
    </span>
  )
}

function SelectAllCheckbox({
  checked,
  indeterminate,
  disabled,
  onChange,
}: {
  checked: boolean
  indeterminate: boolean
  disabled: boolean
  onChange: (checked: boolean) => void
}) {
  const inputRef = useRef<HTMLInputElement | null>(null)

  useEffect(() => {
    if (inputRef.current) {
      inputRef.current.indeterminate = indeterminate
    }
  }, [indeterminate])

  return (
    <input
      ref={inputRef}
      type="checkbox"
      checked={checked}
      disabled={disabled}
      onChange={(event) => onChange(event.target.checked)}
      aria-label="Select all rows"
      data-testid="select-all-rows"
      className="h-4 w-4 rounded border-input text-primary focus:ring-ring disabled:opacity-60"
    />
  )
}

function FolderVirtualPathModal({
  folder,
  onClose,
  onSave,
}: {
  folder: TrackedFolder | null
  onClose: () => void
  onSave: (folderId: number, virtualPath: string | null) => Promise<void>
}) {
  const [value, setValue] = useState('')
  const [saving, setSaving] = useState(false)
  const [showPicker, setShowPicker] = useState(false)

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setValue(folder?.virtual_path ?? '')
      setSaving(false)
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [folder])

  if (!folder) return null

  const submit = async () => {
    const trimmed = value.trim()
    if (trimmed && !trimmed.startsWith('/')) {
      toast.error('Virtual path must be absolute')
      return
    }

    setSaving(true)
    try {
      await onSave(folder.id, trimmed || null)
      onClose()
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <ModalLayer>
        <div className="w-full max-w-lg rounded-lg border border-border bg-background p-6 shadow-xl">
          <h2 className="mb-1 text-lg font-semibold">Set Folder Virtual Path</h2>
          <p className="mb-4 truncate font-mono text-sm text-muted-foreground">
            {folder.folder_path}
          </p>
          <div className="space-y-3">
            <label
              htmlFor="folder-virtual-path"
              className="mb-1 block text-sm font-medium text-foreground"
            >
              Virtual Path
            </label>
            <div className="flex gap-2">
              <input
                id="folder-virtual-path"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder="/docs"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono text-foreground"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                className="rounded-md border border-input px-3 py-2 text-sm hover:bg-accent"
              >
                Browse
              </button>
            </div>
            <p className="text-xs text-muted-foreground">
              Leave empty to clear the folder virtual path.
            </p>
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-input px-4 py-2 text-sm hover:bg-accent"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void submit()}
              disabled={saving}
              className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-60"
            >
              {saving ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={showPicker}
        title="Select Folder Virtual Path"
        description="Choose a parent directory; BitProtector appends the tracked folder name."
        mode="directory"
        value={value}
        startPath={value || '/'}
        confirmLabel="Use Virtual Path"
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue(suggestVirtualPathFromParent(path, folder.folder_path))
          setShowPicker(false)
        }}
      />
    </>
  )
}

function VirtualPathTree({
  selected,
  onSelect,
  refreshKey,
}: {
  selected: string
  onSelect: (path: string) => void
  refreshKey: number
}) {
  const [nodes, setNodes] = useState<TreeNode[]>([])
  const [open, setOpen] = useState<Record<string, boolean>>({})

  const loadChildren = useCallback(async (parent: string) => {
    if (parent === '/') {
      const response = await virtualPathsApi.tree('/')
      setNodes(response.children.map((child) => ({ ...child, loaded: false, children: [] })))
      return
    }

    setNodes((current) => setTreeLoading(current, parent, true))
    try {
      const response = await virtualPathsApi.tree(parent)
      const children = response.children.map((child) => ({ ...child, loaded: false, children: [] }))
      setNodes((current) => updateTreeChildren(current, parent, children))
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Failed to load virtual path tree')
      toast.error(errorMessage)
    } finally {
      setNodes((current) => setTreeLoading(current, parent, false))
    }
  }, [])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setNodes([])
      setOpen({})
      void loadChildren('/')
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [loadChildren, refreshKey])

  const renderNode = (node: TreeNode, depth: number) => {
    const isOpen = !!open[node.path]

    return (
      <div key={node.path}>
        <button
          className={`flex w-full items-center gap-2 rounded px-2 py-1 text-left text-sm hover:bg-accent ${
            selected === node.path ? 'bg-primary/10 text-primary font-medium' : ''
          }`}
          style={{ paddingLeft: `${8 + depth * 14}px` }}
          onClick={() => {
            onSelect(node.path)
            if (!node.has_children) return

            setOpen((current) => ({ ...current, [node.path]: !current[node.path] }))
            if (!node.loaded) {
              void loadChildren(node.path)
            }
          }}
          data-testid={`tree-node-${node.path}`}
        >
          <span className="truncate">{node.name}</span>
          <span className="ml-auto text-xs text-muted-foreground">{node.item_count}</span>
        </button>
        {isOpen && node.children && node.children.length > 0 ? (
          <div>{node.children.map((child) => renderNode(child, depth + 1))}</div>
        ) : null}
      </div>
    )
  }

  return (
    <div className="space-y-0.5 p-2" data-testid="file-tree">
      <button
        className={`flex w-full items-center gap-1 rounded px-2 py-1 text-sm font-medium hover:bg-accent ${
          selected === '' ? 'bg-primary/10 text-primary' : ''
        }`}
        onClick={() => onSelect('')}
      >
        All virtual paths
      </button>
      {nodes.length === 0 ? (
        <div className="px-2 py-2 text-xs text-muted-foreground">No virtual paths assigned</div>
      ) : (
        nodes.map((node) => renderNode(node, 0))
      )}
    </div>
  )
}

export function TrackingWorkspacePage() {
  const [scanningFolderIds, setScanningFolderIds] = useState<Set<number>>(new Set())
  const [mirroringFolderIds, setMirroringFolderIds] = useState<Set<number>>(new Set())
  const [folderScanStatuses, setFolderScanStatuses] = useState<Record<number, FolderScanStatus>>({})
  const [response, setResponse] = useState<{
    items: TrackingItem[]
    total: number
    page: number
    per_page: number
  } | null>(null)
  const [loading, setLoading] = useState(true)
  const [drives, setDrives] = useState<DrivePair[]>([])
  const [params, setParams] = useState<TrackingListParams>({
    page: 1,
    per_page: 50,
    item_kind: 'all',
    source: 'all',
  })
  const [showTrackModal, setShowTrackModal] = useState(false)
  const [showFolderModal, setShowFolderModal] = useState(false)
  const [selectedFile, setSelectedFile] = useState<TrackedFile | null>(null)
  const [selectedRowKeys, setSelectedRowKeys] = useState<Set<string>>(new Set())
  const [folderPathModal, setFolderPathModal] = useState<TrackedFolder | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<TrackingItem | null>(null)
  const [confirmBulkDeleteOpen, setConfirmBulkDeleteOpen] = useState(false)
  const [filePathModal, setFilePathModal] = useState<TrackedFile | null>(null)
  const [treeRefreshKey, setTreeRefreshKey] = useState(0)
  const [virtualPaneCollapsed, setVirtualPaneCollapsed] = useState(true)
  const [bulkMirroring, setBulkMirroring] = useState(false)
  const [bulkDeleting, setBulkDeleting] = useState(false)
  const [postDeleteDetailAction, setPostDeleteDetailAction] =
    useState<DetailPostDeleteAction | null>(null)

  const virtualPrefix = params.virtual_prefix ?? ''
  const hasDrivePairs = drives.length > 0
  const hasActiveFilters =
    !!params.q ||
    params.drive_id != null ||
    (params.item_kind ?? 'all') !== 'all' ||
    (params.source ?? 'all') !== 'all' ||
    params.has_virtual_path != null ||
    !!virtualPrefix

  const load = useCallback(async (nextParams: TrackingListParams) => {
    setLoading(true)
    try {
      const nextResponse = await trackingApi.list(nextParams)
      setResponse(nextResponse)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Failed to load tracking workspace')
      toast.error(errorMessage)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    let active = true
    const loadDrives = async () => {
      try {
        const next = await drivesApi.list()
        if (active) setDrives(next)
      } catch (error) {
        // Extract meaningful error message from backend response
        const errorMessage = getErrorMessage(error, 'Failed to load drive pairs')
        toast.error(errorMessage)
      }
    }
    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void load(params)
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [load, params])

  const activeFolderScanIds = useMemo(
    () =>
      Object.entries(folderScanStatuses)
        .filter(([, status]) => status.scanning)
        .map(([id]) => Number(id))
        .sort((a, b) => a - b),
    [folderScanStatuses]
  )
  const activeFolderScanIdsKey = useMemo(() => activeFolderScanIds.join(','), [activeFolderScanIds])

  useEffect(() => {
    if (!activeFolderScanIdsKey) return

    const scanFolderIds = activeFolderScanIdsKey.split(',').map(Number)

    let active = true

    const poll = async () => {
      try {
        const scanStatuses = await Promise.all(
          scanFolderIds.map(async (folderId) => ({
            folderId,
            status: await foldersApi.scanActive(folderId),
          }))
        )
        if (!active) return

        const completedFolderIds = scanStatuses
          .filter(({ status }) => !status.scanning)
          .map(({ folderId }) => folderId)

        setFolderScanStatuses((current) => {
          const next = { ...current }
          for (const { folderId, status } of scanStatuses) {
            if (status.scanning) {
              next[folderId] = status
            } else {
              delete next[folderId]
            }
          }
          return next
        })

        if (completedFolderIds.length > 0) {
          await load(params)
          if (!active) return
          setTreeRefreshKey((current) => current + 1)
        }
      } catch {
        // Polling is best-effort; the table loader reports fetch failures.
      }
    }

    void poll()
    const timer = window.setInterval(() => {
      void poll()
    }, 1000)

    return () => {
      active = false
      window.clearInterval(timer)
    }
  }, [activeFolderScanIdsKey, load, params])

  const driveName = useCallback(
    (id: number) => drives.find((drive) => drive.id === id)?.name ?? `Drive #${id}`,
    [drives]
  )

  const updateParams = (updates: Partial<TrackingListParams>) => {
    setParams((current) => ({
      ...current,
      ...updates,
      page: updates.page ?? 1,
    }))
  }

  const items = response?.items ?? EMPTY_TRACKING_ITEMS
  const folderItems = useMemo(
    () =>
      items
        .filter((item) => item.kind === 'folder')
        .map(
          (item) =>
            ({
              id: item.id,
              drive_pair_id: item.drive_pair_id,
              folder_path: item.path,
              virtual_path: item.virtual_path,
              scanning: false,
              scan_scanned_files: 0,
              scan_total_files: 0,
              last_scanned_at: null,
              created_at: item.created_at,
            }) satisfies TrackedFolder
        ),
    [items]
  )
  const selectedItems = useMemo(
    () => items.filter((item) => selectedRowKeys.has(trackingRowKey(item))),
    [items, selectedRowKeys]
  )
  const allVisibleSelected =
    items.length > 0 && items.every((item) => selectedRowKeys.has(trackingRowKey(item)))
  const someVisibleSelected =
    !allVisibleSelected && items.some((item) => selectedRowKeys.has(trackingRowKey(item)))
  const selectedFileCount = selectedItems.filter((item) => item.kind === 'file').length
  const selectedFolderCount = selectedItems.filter((item) => item.kind === 'folder').length

  const toggleRowSelection = useCallback((item: TrackingItem, checked: boolean) => {
    const key = trackingRowKey(item)
    setSelectedRowKeys((current) => {
      const next = new Set(current)
      if (checked) {
        next.add(key)
      } else {
        next.delete(key)
      }
      return next
    })
  }, [])

  const toggleAllVisible = useCallback(
    (checked: boolean) => {
      setSelectedRowKeys((current) => {
        const next = new Set(current)
        for (const item of items) {
          const key = trackingRowKey(item)
          if (checked) {
            next.add(key)
          } else {
            next.delete(key)
          }
        }
        return next
      })
    },
    [items]
  )

  useEffect(() => {
    const visibleKeys = new Set(items.map((item) => trackingRowKey(item)))

    const timer = window.setTimeout(() => {
      setSelectedRowKeys((current) => {
        const next = new Set(Array.from(current).filter((key) => visibleKeys.has(key)))
        return next.size === current.size ? current : next
      })
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [items])

  useEffect(() => {
    if (selectedItems.length === 0 && confirmBulkDeleteOpen) {
      const timer = window.setTimeout(() => {
        setConfirmBulkDeleteOpen(false)
      }, 0)

      return () => {
        window.clearTimeout(timer)
      }
    }
  }, [confirmBulkDeleteOpen, selectedItems.length])

  const handleTrack = async (data: TrackFileRequest) => {
    try {
      await filesApi.track(data)
      toast.success('File tracked')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Failed to track file')
      toast.error(errorMessage)
    }
  }

  const handleMirror = async (file: TrackedFile) => {
    try {
      await filesApi.mirror(file.id)
      toast.success('Mirror requested')
      await load(params)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Mirror failed')
      toast.error(errorMessage)
    }
  }

  const handleScanFolder = async (folder: TrackedFolder) => {
    setScanningFolderIds((current) => new Set(current).add(folder.id))
    try {
      const status = await foldersApi.scan(folder.id)
      setFolderScanStatuses((current) => ({
        ...current,
        [folder.id]: status,
      }))
      toast.success('Folder scan started')
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Scan failed')
      toast.error(errorMessage)
    } finally {
      setScanningFolderIds((current) => {
        const next = new Set(current)
        next.delete(folder.id)
        return next
      })
    }
  }

  const handleMirrorFolder = async (folder: TrackedFolder) => {
    setMirroringFolderIds((current) => new Set(current).add(folder.id))
    try {
      const result = await foldersApi.mirror(folder.id)
      toast.success(`Mirror complete: ${result.mirrored_files} file(s) mirrored`)
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Folder mirror failed')
      toast.error(errorMessage)
    } finally {
      setMirroringFolderIds((current) => {
        const next = new Set(current)
        next.delete(folder.id)
        return next
      })
    }
  }

  const handleSetFolderVirtualPath = async (folderId: number, virtualPath: string | null) => {
    try {
      await foldersApi.update(folderId, { virtual_path: virtualPath })
      toast.success('Folder virtual path updated')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Failed to update folder virtual path')
      toast.error(errorMessage)
    }
  }

  const openFileDetails = useCallback(async (item: TrackingItem) => {
    if (item.kind !== 'file') return
    try {
      const file = await filesApi.get(item.id)
      setSelectedFile({
        ...file,
        virtual_path: file.virtual_path ?? item.virtual_path,
      })
    } catch {
      setSelectedFile(toTrackedFile(item))
    }
  }, [])

  const performDelete = useCallback(
    async (targets: TrackingItem[]) => {
      if (targets.length === 0) return

      setBulkDeleting(true)
      const deleted: TrackingItem[] = []
      let failedCount = 0

      for (const target of targets) {
        try {
          if (target.kind === 'file') {
            await filesApi.delete(target.id)
          } else {
            await foldersApi.delete(target.id)
          }
          deleted.push(target)
        } catch {
          failedCount += 1
        }
      }

      const deletedCount = deleted.length
      if (deletedCount > 0) {
        const deletedKeys = new Set(deleted.map((item) => trackingRowKey(item)))
        setSelectedRowKeys((current) => {
          const next = new Set(current)
          for (const key of deletedKeys) {
            next.delete(key)
          }
          return next
        })

        const deletedFileIds = new Set(
          deleted.filter((item) => item.kind === 'file').map((item) => item.id)
        )
        if (selectedFile && deletedFileIds.has(selectedFile.id)) {
          const nextFileId = nextFileAfterDeletion(items, selectedFile.id, deletedFileIds)
          setPostDeleteDetailAction(
            nextFileId ? { type: 'open', fileId: nextFileId } : { type: 'close' }
          )
        }

        if (targets.length === 1) {
          toast.success(
            targets[0].kind === 'file' ? 'File removed from tracking' : 'Folder removed'
          )
        } else {
          toast.success(`Removed ${deletedCount} item(s) from tracking`)
        }
        setTreeRefreshKey((current) => current + 1)
      }

      if (failedCount > 0) {
        toast.error(
          targets.length === 1 ? 'Delete failed' : `Failed to remove ${failedCount} item(s)`
        )
      }

      setDeleteTarget(null)
      setConfirmBulkDeleteOpen(false)
      await load(params)
      setBulkDeleting(false)
    },
    [items, load, params, selectedFile]
  )

  const handleDelete = async () => {
    if (!deleteTarget) return
    await performDelete([deleteTarget])
  }

  const handleDeleteSelected = async () => {
    await performDelete(selectedItems)
  }

  const handleMirrorSelected = async () => {
    if (selectedItems.length === 0) return

    setBulkMirroring(true)
    let mirroredCount = 0
    let failedCount = 0

    for (const item of selectedItems) {
      try {
        if (item.kind === 'file') {
          await filesApi.mirror(item.id)
        } else {
          await foldersApi.mirror(item.id)
        }
        mirroredCount += 1
      } catch {
        failedCount += 1
      }
    }

    if (mirroredCount > 0) {
      toast.success(`Mirror requested for ${mirroredCount} item(s)`)
    }
    if (failedCount > 0) {
      toast.error(`Failed to mirror ${failedCount} item(s)`)
    }

    await load(params)
    setBulkMirroring(false)
  }

  useEffect(() => {
    if (!postDeleteDetailAction) return

    const timer = window.setTimeout(() => {
      if (postDeleteDetailAction.type === 'close') {
        setSelectedFile(null)
        setPostDeleteDetailAction(null)
        return
      }

      const nextItem = items.find(
        (item) => item.kind === 'file' && item.id === postDeleteDetailAction.fileId
      )
      setPostDeleteDetailAction(null)
      if (!nextItem) {
        setSelectedFile(null)
        return
      }
      void openFileDetails(nextItem)
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [items, openFileDetails, postDeleteDetailAction])

  const handleSetFileVirtualPath = async (fileId: number, vpath: string) => {
    try {
      await virtualPathsApi.set(fileId, { virtual_path: vpath })
      toast.success('Virtual path updated')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch (error) {
      // Extract meaningful error message from backend response
      const errorMessage = getErrorMessage(error, 'Failed to update virtual path')
      toast.error(errorMessage)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4" data-testid="file-browser-page">
      <PageIntro
        title="Tracking Workspace"
        subtitle="Track files and folders, manage virtual paths, and inspect item details."
      />

      <div className="flex min-h-0 flex-1 gap-0">
        <aside
          className={`${virtualPaneCollapsed ? 'w-12 overflow-y-hidden' : 'w-64 overflow-y-auto'} shrink-0 overflow-x-hidden border-r border-border bg-background transition-[width] duration-200 ease-in-out`}
        >
          <div
            className={`${virtualPaneCollapsed ? 'flex items-center justify-center border-b px-1 py-2.5' : 'flex items-center justify-between border-b p-3'}`}
          >
            {!virtualPaneCollapsed ? (
              <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Virtual paths
              </h2>
            ) : null}
            <button
              type="button"
              onClick={() => setVirtualPaneCollapsed((current) => !current)}
              className="flex p-1 text-muted-foreground transition-colors hover:text-accent-foreground"
              data-testid="toggle-virtual-pane"
              title={
                virtualPaneCollapsed ? 'Expand virtual paths pane' : 'Collapse virtual paths pane'
              }
            >
              {virtualPaneCollapsed ? (
                <PanelLeftOpen className="h-4 w-4" />
              ) : (
                <PanelLeftClose className="h-4 w-4" />
              )}
            </button>
          </div>
          {virtualPaneCollapsed ? (
            <div className="flex h-[calc(100%-44px)] items-center justify-center">
              <span className="select-none text-sm font-semibold uppercase tracking-[0.14em] text-muted-foreground [writing-mode:vertical-lr]">
                Virtual Paths
              </span>
            </div>
          ) : (
            <VirtualPathTree
              selected={virtualPrefix}
              onSelect={(path) => updateParams({ virtual_prefix: path || undefined, page: 1 })}
              refreshKey={treeRefreshKey}
            />
          )}
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          <div className="border-b border-border bg-background px-4 py-3">
            <div className="mb-3 flex flex-wrap items-start justify-between gap-3">
              <BreadcrumbNav
                path={virtualPrefix}
                onNavigate={(path) => updateParams({ virtual_prefix: path || undefined, page: 1 })}
              />
              <div className="flex flex-wrap items-center gap-2">
                <button
                  className="inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
                  onClick={() => {
                    if (!hasDrivePairs) {
                      return
                    }
                    setShowFolderModal(true)
                  }}
                  disabled={!hasDrivePairs}
                  data-testid="add-folder-button"
                >
                  <Plus className="h-4 w-4" /> Add Folder
                </button>
                <button
                  className="inline-flex shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
                  onClick={() => {
                    if (!hasDrivePairs) {
                      return
                    }
                    setShowTrackModal(true)
                  }}
                  disabled={!hasDrivePairs}
                  data-testid="track-file-btn"
                >
                  <Plus className="h-4 w-4" /> Track File
                </button>
              </div>
            </div>
            {!hasDrivePairs ? (
              <p
                className="mb-3 text-xs text-muted-foreground"
                data-testid="tracking-no-drives-hint"
              >
                Add a drive pair first to track files or folders.
              </p>
            ) : null}

            <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-5 [&>*]:min-w-0">
              <input
                value={params.q ?? ''}
                onChange={(event) => updateParams({ q: event.target.value || undefined })}
                placeholder="Search by path"
                className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground"
              />
              <select
                value={params.drive_id ?? ''}
                onChange={(event) =>
                  updateParams({
                    drive_id: event.target.value ? Number(event.target.value) : undefined,
                  })
                }
                className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground"
              >
                <option value="">All drives</option>
                {drives.map((drive) => (
                  <option key={drive.id} value={drive.id}>
                    {drive.name}
                  </option>
                ))}
              </select>
              <select
                value={params.item_kind ?? 'all'}
                onChange={(event) =>
                  updateParams({ item_kind: event.target.value as TrackingListParams['item_kind'] })
                }
                className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground"
              >
                <option value="all">All items</option>
                <option value="file">Files</option>
                <option value="folder">Folders</option>
              </select>
              <select
                value={params.source ?? 'all'}
                onChange={(event) =>
                  updateParams({ source: event.target.value as TrackingListParams['source'] })
                }
                className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground"
              >
                <option value="all">All sources</option>
                <option value="direct">Direct</option>
                <option value="folder">Folder</option>
              </select>
              <select
                value={
                  params.has_virtual_path == null ? 'all' : params.has_virtual_path ? 'yes' : 'no'
                }
                onChange={(event) => {
                  const value = event.target.value
                  updateParams({
                    has_virtual_path: value === 'all' ? undefined : value === 'yes',
                  })
                }}
                className="w-full min-w-0 max-w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground"
              >
                <option value="all">With + Without Virtual Path</option>
                <option value="yes">With Virtual Path</option>
                <option value="no">Without Virtual Path</option>
              </select>
            </div>
            {hasActiveFilters ? (
              <div className="flex justify-end pt-1">
                <button
                  type="button"
                  onClick={() =>
                    updateParams({
                      q: undefined,
                      drive_id: undefined,
                      item_kind: 'all',
                      source: 'all',
                      has_virtual_path: undefined,
                      virtual_prefix: undefined,
                      page: 1,
                    })
                  }
                  className="text-xs text-muted-foreground underline hover:text-foreground"
                >
                  Clear filters
                </button>
              </div>
            ) : null}
          </div>

          <div className="flex-1 overflow-auto p-4">
            {loading && !response ? (
              <div className="flex items-center justify-center py-16">
                <LoadingSpinner />
              </div>
            ) : (
              <div
                className={`space-y-3 transition-opacity duration-150${
                  loading ? ' pointer-events-none opacity-60' : ''
                }`}
              >
                <DataTable
                  tableTestId="tracking-table"
                  columns={[
                    {
                      key: 'select',
                      header: (
                        <SelectAllCheckbox
                          checked={allVisibleSelected}
                          indeterminate={someVisibleSelected}
                          disabled={items.length === 0}
                          onChange={(checked) => toggleAllVisible(checked)}
                        />
                      ),
                      className: 'w-10',
                      cell: (item) => (
                        <input
                          type="checkbox"
                          checked={selectedRowKeys.has(trackingRowKey(item))}
                          onClick={(event) => event.stopPropagation()}
                          onChange={(event) => toggleRowSelection(item, event.target.checked)}
                          aria-label={`Select ${item.kind} ${item.path}`}
                          data-testid={`select-row-${trackingRowKey(item)}`}
                          className="h-4 w-4 rounded border-input text-primary focus:ring-ring"
                        />
                      ),
                    },
                    {
                      key: 'kind',
                      header: 'Kind',
                      cell: (item) =>
                        item.kind === 'file' ? (
                          <span className="rounded bg-primary/15 px-2 py-0.5 text-xs font-medium text-primary">
                            File
                          </span>
                        ) : (
                          <span className="rounded bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">
                            Folder
                          </span>
                        ),
                    },
                    {
                      key: 'path',
                      header: 'Path',
                      cell: (item) => <span className="font-mono text-xs">{item.path}</span>,
                    },
                    {
                      key: 'drive',
                      header: 'Drive Pair',
                      cell: (item) => driveName(item.drive_pair_id),
                    },
                    {
                      key: 'virtual_path',
                      header: 'Virtual Path',
                      cell: (item) =>
                        item.virtual_path ? (
                          <span className="font-mono text-xs">{item.virtual_path}</span>
                        ) : (
                          <span className="text-muted-foreground">—</span>
                        ),
                    },
                    {
                      key: 'source',
                      header: 'Source',
                      cell: (item) => <SourceBadge source={item.source} />,
                    },
                    {
                      key: 'status',
                      header: 'Status',
                      cell: (item) => {
                        if (item.kind === 'file') {
                          return item.is_mirrored ? (
                            <span className="rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-700 dark:bg-green-900/30 dark:text-green-400">
                              Mirrored
                            </span>
                          ) : (
                            <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700 dark:bg-slate-700/40 dark:text-slate-300">
                              Tracked
                            </span>
                          )
                        }

                        const scanStatus = folderScanStatuses[item.id]
                        const isFolderScanning =
                          scanningFolderIds.has(item.id) || scanStatus?.scanning === true

                        if (isFolderScanning) {
                          return (
                            <FolderScanningStatus
                              scanned={scanStatus?.scanned ?? 0}
                              total={scanStatus?.total ?? 0}
                            />
                          )
                        }

                        const status = item.folder_status ?? 'not_scanned'
                        const total = item.folder_total_files ?? 0
                        const mirrored = item.folder_mirrored_files ?? 0
                        return (
                          <FolderStatusBadge status={status} total={total} mirrored={mirrored} />
                        )
                      },
                    },
                    {
                      key: 'created',
                      header: 'Created',
                      cell: (item) => formatDate(item.created_at),
                    },
                    {
                      key: 'actions',
                      header: '',
                      cell: (item) =>
                        item.kind === 'file' ? (
                          <FileActions
                            file={toTrackedFile(item)}
                            onMirror={handleMirror}
                            onDelete={(file) => setDeleteTarget({ ...item, id: file.id })}
                            onSetVirtualPath={(file) =>
                              setFilePathModal({ ...toTrackedFile(item), id: file.id })
                            }
                          />
                        ) : (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={(event) => {
                                event.stopPropagation()
                                const folder = folderItems.find((entry) => entry.id === item.id)
                                if (folder) setFolderPathModal(folder)
                              }}
                              className="rounded-md border border-input px-2 py-1 text-xs hover:bg-accent"
                            >
                              Set Path
                            </button>
                            {(() => {
                              const folder = folderItems.find((entry) => entry.id === item.id)
                              const wantsMirror =
                                item.folder_status === 'tracked' || item.folder_status === 'partial'
                              const scanStatus = folderScanStatuses[item.id]
                              const isFolderScanning =
                                scanningFolderIds.has(item.id) || scanStatus?.scanning === true
                              const isFolderMirroring = mirroringFolderIds.has(item.id)
                              const isFolderBusy = isFolderScanning || isFolderMirroring

                              return (
                                <button
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    if (!folder || isFolderBusy) return
                                    if (wantsMirror) {
                                      void handleMirrorFolder(folder)
                                      return
                                    }
                                    void handleScanFolder(folder)
                                  }}
                                  disabled={isFolderBusy}
                                  className="inline-flex items-center gap-1 rounded-md border border-input px-2 py-1 text-xs hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
                                  data-testid={`folder-action-${item.id}`}
                                >
                                  {isFolderScanning ? (
                                    <>
                                      <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
                                      Scanning...
                                    </>
                                  ) : isFolderMirroring ? (
                                    <>
                                      <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
                                      Mirroring...
                                    </>
                                  ) : wantsMirror ? (
                                    'Mirror'
                                  ) : (
                                    'Scan'
                                  )}
                                </button>
                              )
                            })()}
                            <button
                              onClick={(event) => {
                                event.stopPropagation()
                                setDeleteTarget(item)
                              }}
                              className="rounded-md border border-destructive/40 px-2 py-1 text-xs text-destructive hover:bg-destructive/10"
                              data-testid={`delete-folder-${item.id}`}
                            >
                              Delete
                            </button>
                          </div>
                        ),
                    },
                  ]}
                  data={items}
                  rowKey={(item) => trackingRowKey(item)}
                  rowTestId={(item) => `${item.kind}-row-${item.id}`}
                  onRowClick={(item) => {
                    if (item.kind === 'file') {
                      void openFileDetails(item)
                    }
                  }}
                  selectedRowKey={selectedFile ? `file-${selectedFile.id}` : null}
                  selectedRowKeys={selectedRowKeys}
                  emptyState={
                    hasActiveFilters ? (
                      <EmptyState
                        title="No results"
                        description="No items match the current filters. Try adjusting or clearing the filters."
                      />
                    ) : (
                      <EmptyState
                        title="No tracked items"
                        description="Track files or add folders to start managing content here."
                      />
                    )
                  }
                />

                {response ? (
                  <Pagination
                    page={response.page}
                    perPage={response.per_page}
                    total={response.total}
                    onPageChange={(page) => setParams((current) => ({ ...current, page }))}
                  />
                ) : null}
              </div>
            )}
          </div>
          {selectedItems.length > 0 ? (
            <div
              className="border-t border-border bg-background/95 px-4 py-3 backdrop-blur"
              data-testid="tracking-bulk-actions"
            >
              <div className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm text-muted-foreground" data-testid="selected-count">
                  {selectedItems.length} selected{' '}
                  <span className="ml-1 text-muted-foreground/60">
                    ({selectedFileCount} file{selectedFileCount !== 1 ? 's' : ''},{' '}
                    {selectedFolderCount} folder{selectedFolderCount !== 1 ? 's' : ''})
                  </span>
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setSelectedRowKeys(new Set())}
                    className="shrink-0 whitespace-nowrap rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent"
                    data-testid="bulk-deselect"
                  >
                    Deselect all
                  </button>
                  <button
                    type="button"
                    onClick={() => void handleMirrorSelected()}
                    disabled={bulkMirroring || bulkDeleting}
                    className="shrink-0 whitespace-nowrap rounded-md border border-input px-3 py-1.5 text-sm hover:bg-accent disabled:cursor-not-allowed disabled:opacity-60"
                    data-testid="bulk-mirror"
                  >
                    {bulkMirroring ? 'Mirroring...' : 'Mirror selected'}
                  </button>
                  <button
                    type="button"
                    onClick={() => setConfirmBulkDeleteOpen(true)}
                    disabled={bulkDeleting || bulkMirroring}
                    className="shrink-0 whitespace-nowrap rounded-md border border-destructive/40 px-3 py-1.5 text-sm text-destructive hover:bg-destructive/10 disabled:cursor-not-allowed disabled:opacity-60"
                    data-testid="bulk-delete"
                  >
                    Delete selected
                  </button>
                </div>
              </div>
            </div>
          ) : null}
        </div>

        {selectedFile ? (
          <aside className="w-80 shrink-0 overflow-auto border-l border-border bg-background">
            <FileDetails
              file={selectedFile}
              drivePairName={driveName(selectedFile.drive_pair_id)}
              onClose={() => setSelectedFile(null)}
            />
          </aside>
        ) : null}
      </div>

      <TrackFileModal
        open={showTrackModal}
        onClose={() => setShowTrackModal(false)}
        onTrack={handleTrack}
        drives={drives}
      />
      {showFolderModal ? (
        <FolderFormModal
          drives={drives}
          onClose={() => setShowFolderModal(false)}
          onSave={async (data) => {
            try {
              await foldersApi.create(data)
              toast.success('Folder added')
              setShowFolderModal(false)
              await load(params)
              setTreeRefreshKey((current) => current + 1)
            } catch (error) {
              // Extract meaningful error message from backend response
              const errorMessage = getErrorMessage(error, 'Failed to add folder')
              toast.error(errorMessage)
            }
          }}
        />
      ) : null}
      <FolderVirtualPathModal
        folder={folderPathModal}
        onClose={() => setFolderPathModal(null)}
        onSave={handleSetFolderVirtualPath}
      />
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null)
        }}
        title={deleteTarget?.kind === 'folder' ? 'Remove tracked folder' : 'Remove tracked file'}
        description={
          deleteTarget?.kind === 'folder'
            ? `Remove tracked folder "${deleteTarget.path}"?`
            : `Remove tracked file "${deleteTarget?.path}"?`
        }
        destructive
        onConfirm={handleDelete}
      />
      <ConfirmDialog
        open={confirmBulkDeleteOpen && selectedItems.length > 0}
        onOpenChange={(open) => {
          if (!open) setConfirmBulkDeleteOpen(false)
        }}
        title="Remove selected items"
        description={`Remove ${selectedItems.length} selected item(s) from tracking?`}
        destructive
        onConfirm={handleDeleteSelected}
      />
      <FileDetailsPathModalBridge
        file={filePathModal}
        onClose={() => setFilePathModal(null)}
        onSave={handleSetFileVirtualPath}
      />
    </div>
  )
}

function FileDetailsPathModalBridge({
  file,
  onClose,
  onSave,
}: {
  file: TrackedFile | null
  onClose: () => void
  onSave: (fileId: number, virtualPath: string) => Promise<void>
}) {
  if (!file) return null

  return <FileVirtualPathModal file={file} onClose={onClose} onSave={onSave} />
}

function FileVirtualPathModal({
  file,
  onClose,
  onSave,
}: {
  file: TrackedFile | null
  onClose: () => void
  onSave: (fileId: number, virtualPath: string) => Promise<void>
}) {
  const [value, setValue] = useState('')
  const [saving, setSaving] = useState(false)
  const [showPicker, setShowPicker] = useState(false)

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setValue(file?.virtual_path ?? '')
      setSaving(false)
    }, 0)

    return () => {
      window.clearTimeout(timer)
    }
  }, [file])

  if (!file) return null

  const submit = async () => {
    const trimmed = value.trim()
    if (!trimmed) return
    if (!trimmed.startsWith('/')) {
      toast.error('Virtual path must be absolute')
      return
    }

    setSaving(true)
    try {
      await onSave(file.id, trimmed)
      onClose()
    } finally {
      setSaving(false)
    }
  }

  return (
    <>
      <ModalLayer>
        <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
          <h2 className="text-lg font-semibold">Set File Virtual Path</h2>
          <p className="mt-1 font-mono text-sm text-muted-foreground">{file.relative_path}</p>

          <div className="mt-4 space-y-3">
            <label htmlFor="file-virtual-path" className="mb-1 block text-sm font-medium">
              Virtual Path
            </label>
            <div className="flex gap-2">
              <input
                id="file-virtual-path"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder="/docs/report.pdf"
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                className="rounded-md border border-border px-3 py-2 text-sm hover:bg-accent"
              >
                Browse
              </button>
            </div>
          </div>

          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-border px-4 py-2 text-sm hover:bg-accent"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void submit()}
              disabled={saving || !value.trim()}
              className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {saving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </ModalLayer>
      <PathPickerDialog
        open={showPicker}
        title="Select File Virtual Path"
        description="Choose the absolute virtual path for this tracked file."
        mode="file"
        virtualFileMode={true}
        value={value}
        startPath={value || '/'}
        confirmLabel="Use Virtual Path"
        onClose={() => setShowPicker(false)}
        onPick={(path) => {
          setValue(path)
          setShowPicker(false)
        }}
      />
    </>
  )
}
