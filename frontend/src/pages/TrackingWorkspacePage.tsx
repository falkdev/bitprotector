import { useCallback, useEffect, useMemo, useState } from 'react'
import { FolderPlus, PanelLeftClose, PanelLeftOpen, Plus } from 'lucide-react'
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
import { Pagination } from '@/components/shared/Pagination'
import { PageIntro } from '@/components/shared/PageIntro'
import { PathPickerDialog } from '@/components/shared/PathPickerDialog'
import { TrackFileModal } from '@/components/tracking/TrackFileModal'
import { FolderFormModal } from '@/components/tracking/FolderFormModal'
import { formatDate } from '@/lib/format'
import type { DrivePair } from '@/types/drive'
import type { TrackedFile, TrackFileRequest } from '@/types/file'
import type { TrackedFolder } from '@/types/folder'
import type { TrackingItem, TrackingListParams } from '@/types/tracking'
import type { VirtualPathTreeNode } from '@/types/virtual-path'

type TreeNode = VirtualPathTreeNode & {
  loaded?: boolean
  loading?: boolean
  children?: TreeNode[]
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
  const className = source === 'folder' ? 'bg-yellow-100 text-yellow-700' : 'bg-blue-100 text-blue-700'

  return <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${className}`}>{label}</span>
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
      ? 'bg-gray-100 text-gray-600'
      : status === 'mirrored'
      ? 'bg-green-100 text-green-700'
      : status === 'tracked'
        ? 'bg-slate-100 text-slate-700'
        : status === 'partial'
          ? 'bg-amber-100 text-amber-700'
          : 'bg-gray-100 text-gray-600'

  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${className}`}>
      {label}
      {ratio}
    </span>
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
    setValue(folder?.virtual_path ?? '')
    setSaving(false)
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
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
        <div className="w-full max-w-lg rounded-lg bg-white p-6 shadow-xl">
          <h2 className="mb-1 text-lg font-semibold">Set Folder Virtual Path</h2>
          <p className="mb-4 truncate font-mono text-sm text-gray-500">{folder.folder_path}</p>
          <div className="space-y-3">
            <label htmlFor="folder-virtual-path" className="mb-1 block text-sm font-medium text-gray-700">
              Virtual Path
            </label>
            <div className="flex gap-2">
              <input
                id="folder-virtual-path"
                value={value}
                onChange={(event) => setValue(event.target.value)}
                placeholder="/docs"
                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm font-mono"
              />
              <button
                type="button"
                onClick={() => setShowPicker(true)}
                className="rounded-md border border-gray-300 px-3 py-2 text-sm hover:bg-gray-50"
              >
                Browse
              </button>
            </div>
            <p className="text-xs text-gray-500">Leave empty to clear the folder virtual path.</p>
          </div>
          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              className="rounded-md border border-gray-300 px-4 py-2 text-sm hover:bg-gray-50"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={() => void submit()}
              disabled={saving}
              className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-60"
            >
              {saving ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select Folder Virtual Path"
        description="Choose the absolute virtual path for this tracked folder."
        mode="directory"
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
    } catch {
      toast.error('Failed to load virtual path tree')
    } finally {
      setNodes((current) => setTreeLoading(current, parent, false))
    }
  }, [])

  useEffect(() => {
    setNodes([])
    setOpen({})
    void loadChildren('/')
  }, [loadChildren, refreshKey])

  const renderNode = (node: TreeNode, depth: number) => {
    const isOpen = !!open[node.path]

    return (
      <div key={node.path}>
        <button
          className={`flex w-full items-center gap-2 rounded px-2 py-1 text-left text-sm hover:bg-gray-100 ${
            selected === node.path ? 'bg-blue-50 text-blue-700 font-medium' : ''
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
          <span className="ml-auto text-xs text-gray-400">{node.item_count}</span>
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
        className={`flex w-full items-center gap-1 rounded px-2 py-1 text-sm font-medium hover:bg-gray-100 ${
          selected === '' ? 'bg-blue-50 text-blue-700' : ''
        }`}
        onClick={() => onSelect('')}
      >
        All virtual paths
      </button>
      {nodes.length === 0 ? (
        <div className="px-2 py-2 text-xs text-gray-500">No virtual paths assigned</div>
      ) : (
        nodes.map((node) => renderNode(node, 0))
      )}
    </div>
  )
}

export function TrackingWorkspacePage() {
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
  const [folderPathModal, setFolderPathModal] = useState<TrackedFolder | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<TrackingItem | null>(null)
  const [filePathModal, setFilePathModal] = useState<TrackedFile | null>(null)
  const [treeRefreshKey, setTreeRefreshKey] = useState(0)
  const [virtualPaneCollapsed, setVirtualPaneCollapsed] = useState(true)

  const virtualPrefix = params.virtual_prefix ?? ''
  const hasDrivePairs = drives.length > 0

  const load = useCallback(async (nextParams: TrackingListParams) => {
    setLoading(true)
    try {
      const nextResponse = await trackingApi.list(nextParams)
      setResponse(nextResponse)
    } catch {
      toast.error('Failed to load tracking workspace')
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
      } catch {
        toast.error('Failed to load drive pairs')
      }
    }
    void loadDrives()
    return () => {
      active = false
    }
  }, [])

  useEffect(() => {
    void load(params)
  }, [load, params])

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

  const items = response?.items ?? []
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
              last_scanned_at: null,
              created_at: item.created_at,
            }) satisfies TrackedFolder
        ),
    [items]
  )

  const handleTrack = async (data: TrackFileRequest) => {
    try {
      await filesApi.track(data)
      toast.success('File tracked')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Failed to track file')
    }
  }

  const handleMirror = async (file: TrackedFile) => {
    try {
      await filesApi.mirror(file.id)
      toast.success('Mirror requested')
      await load(params)
    } catch {
      toast.error('Mirror failed')
    }
  }

  const handleScanFolder = async (folder: TrackedFolder) => {
    try {
      const result = await foldersApi.scan(folder.id)
      toast.success(`Scan complete: ${result.new_files} new, ${result.changed_files} changed`)
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Scan failed')
    }
  }

  const handleMirrorFolder = async (folder: TrackedFolder) => {
    try {
      const result = await foldersApi.mirror(folder.id)
      toast.success(`Mirror complete: ${result.mirrored_files} file(s) mirrored`)
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Folder mirror failed')
    }
  }

  const handleSetFolderVirtualPath = async (folderId: number, virtualPath: string | null) => {
    try {
      await foldersApi.update(folderId, { virtual_path: virtualPath })
      toast.success('Folder virtual path updated')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Failed to update folder virtual path')
    }
  }

  const handleDelete = async () => {
    if (!deleteTarget) return

    try {
      if (deleteTarget.kind === 'file') {
        await filesApi.delete(deleteTarget.id)
        toast.success('File removed from tracking')
      } else {
        await foldersApi.delete(deleteTarget.id)
        toast.success('Folder removed')
      }
      setDeleteTarget(null)
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Delete failed')
    }
  }

  const openFileDetails = async (item: TrackingItem) => {
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
  }

  const handleSetFileVirtualPath = async (fileId: number, vpath: string) => {
    try {
      await virtualPathsApi.set(fileId, { virtual_path: vpath })
      toast.success('Virtual path updated')
      await load(params)
      setTreeRefreshKey((current) => current + 1)
    } catch {
      toast.error('Failed to update virtual path')
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
        className={`${virtualPaneCollapsed ? 'w-12 overflow-y-hidden' : 'w-64 overflow-y-auto'} shrink-0 overflow-x-hidden border-r border-gray-200 bg-white transition-[width] duration-200 ease-in-out`}
      >
        <div className={`${virtualPaneCollapsed ? 'flex items-center justify-center border-b px-1 py-2.5' : 'flex items-center justify-between border-b p-3'}`}>
          {!virtualPaneCollapsed ? (
            <h2 className="text-xs font-semibold uppercase tracking-wide text-gray-500">Virtual paths</h2>
          ) : null}
          <button
            type="button"
            onClick={() => setVirtualPaneCollapsed((current) => !current)}
            className="flex p-1 text-muted-foreground transition-colors hover:text-accent-foreground"
            data-testid="toggle-virtual-pane"
            title={virtualPaneCollapsed ? 'Expand virtual paths pane' : 'Collapse virtual paths pane'}
          >
            {virtualPaneCollapsed ? <PanelLeftOpen className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
          </button>
        </div>
        {virtualPaneCollapsed ? (
          <div className="flex h-[calc(100%-44px)] items-center justify-center">
            <span className="select-none text-sm font-semibold uppercase tracking-[0.14em] text-gray-400 [writing-mode:vertical-lr]">
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
        <div className="border-b bg-white px-4 py-3">
          <div className="mb-3 flex items-center justify-between gap-4">
            <BreadcrumbNav
              path={virtualPrefix}
              onNavigate={(path) => updateParams({ virtual_prefix: path || undefined, page: 1 })}
            />
            <div className="flex items-center gap-2">
              <button
                className="flex items-center gap-1.5 rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
                onClick={() => {
                  if (!hasDrivePairs) {
                    return
                  }
                  setShowFolderModal(true)
                }}
                disabled={!hasDrivePairs}
                data-testid="add-folder-button"
              >
                <FolderPlus className="h-4 w-4" /> Add Folder
              </button>
              <button
                className="flex items-center gap-1.5 rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-60"
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
            <p className="mb-3 text-xs text-muted-foreground" data-testid="tracking-no-drives-hint">
              Add a drive pair first to track files or folders.
            </p>
          ) : null}

          <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-6">
            <input
              value={params.q ?? ''}
              onChange={(event) => updateParams({ q: event.target.value || undefined })}
              placeholder="Search by path"
              className="rounded-md border border-gray-300 px-3 py-2 text-sm"
            />
            <select
              value={params.drive_id ?? ''}
              onChange={(event) =>
                updateParams({
                  drive_id: event.target.value ? Number(event.target.value) : undefined,
                })
              }
              className="rounded-md border border-gray-300 px-3 py-2 text-sm"
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
              className="rounded-md border border-gray-300 px-3 py-2 text-sm"
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
              className="rounded-md border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="all">All sources</option>
              <option value="direct">Direct</option>
              <option value="folder">Folder</option>
            </select>
            <select
              value={params.has_virtual_path == null ? 'all' : params.has_virtual_path ? 'yes' : 'no'}
              onChange={(event) => {
                const value = event.target.value
                updateParams({
                  has_virtual_path: value === 'all' ? undefined : value === 'yes',
                })
              }}
              className="rounded-md border border-gray-300 px-3 py-2 text-sm"
            >
              <option value="all">With + Without Virtual Path</option>
              <option value="yes">With Virtual Path</option>
              <option value="no">Without Virtual Path</option>
            </select>
            <input
              value={params.virtual_prefix ?? ''}
              onChange={(event) => updateParams({ virtual_prefix: event.target.value || undefined })}
              placeholder="Virtual path prefix (/docs)"
              className="rounded-md border border-gray-300 px-3 py-2 text-sm font-mono"
            />
          </div>
        </div>

        <div className="flex-1 overflow-auto p-4">
          {loading && !response ? (
            <div className="flex items-center justify-center py-16">
              <LoadingSpinner />
            </div>
          ) : (
            <div className="space-y-3">
              <DataTable
                tableTestId="tracking-table"
                columns={[
                  {
                    key: 'kind',
                    header: 'Kind',
                    cell: (item) =>
                      item.kind === 'file' ? (
                        <span className="rounded bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700">
                          File
                        </span>
                      ) : (
                        <span className="rounded bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-700">
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
                        <span className="text-gray-400">—</span>
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
                          <span className="rounded-full bg-green-100 px-1.5 py-0.5 text-xs text-green-700">
                            Mirrored
                          </span>
                        ) : (
                          <span className="rounded-full bg-slate-100 px-1.5 py-0.5 text-xs text-slate-700">
                            Tracked
                          </span>
                        )
                      }

                      const status = item.folder_status ?? 'not_scanned'
                      const total = item.folder_total_files ?? 0
                      const mirrored = item.folder_mirrored_files ?? 0
                      return <FolderStatusBadge status={status} total={total} mirrored={mirrored} />
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
                          onSetVirtualPath={(file) => setFilePathModal({ ...toTrackedFile(item), id: file.id })}
                        />
                      ) : (
                        <div className="flex items-center gap-2">
                          <button
                            onClick={() => {
                              const folder = folderItems.find((entry) => entry.id === item.id)
                              if (folder) setFolderPathModal(folder)
                            }}
                            className="rounded-md border border-gray-300 px-2 py-1 text-xs hover:bg-gray-50"
                          >
                            Set Path
                          </button>
                          <button
                            onClick={() => {
                              const folder = folderItems.find((entry) => entry.id === item.id)
                              if (!folder) return
                              const status = item.folder_status ?? 'not_scanned'
                              if (status === 'tracked' || status === 'partial') {
                                void handleMirrorFolder(folder)
                                return
                              }
                              void handleScanFolder(folder)
                            }}
                            className="rounded-md border border-gray-300 px-2 py-1 text-xs hover:bg-gray-50"
                            data-testid={`folder-action-${item.id}`}
                          >
                            {item.folder_status === 'tracked' || item.folder_status === 'partial'
                              ? 'Mirror'
                              : 'Scan'}
                          </button>
                          <button
                            onClick={() => setDeleteTarget(item)}
                            className="rounded-md border border-red-200 px-2 py-1 text-xs text-red-600 hover:bg-red-50"
                            data-testid={`delete-folder-${item.id}`}
                          >
                            Delete
                          </button>
                        </div>
                      ),
                  },
                ]}
                data={items}
                rowKey={(item) => `${item.kind}-${item.id}`}
                rowTestId={(item) => `${item.kind}-row-${item.id}`}
                onRowClick={(item) => {
                  if (item.kind === 'file') {
                    void openFileDetails(item)
                  }
                }}
                selectedRowKey={selectedFile ? `file-${selectedFile.id}` : null}
                emptyState={
                  <EmptyState
                    title="No tracked items"
                    description="Track files or add folders to start managing content here."
                  />
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
      </div>

      {selectedFile ? (
        <aside className="w-80 shrink-0 overflow-auto border-l border-gray-200 bg-white">
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
            await foldersApi.create(data)
            toast.success('Folder added')
            setShowFolderModal(false)
            await load(params)
            setTreeRefreshKey((current) => current + 1)
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
    setValue(file?.virtual_path ?? '')
    setSaving(false)
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
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
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
      </div>
      <PathPickerDialog
        open={showPicker}
        title="Select File Virtual Path"
        description="Choose the absolute virtual path for this tracked file."
        mode="file"
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
