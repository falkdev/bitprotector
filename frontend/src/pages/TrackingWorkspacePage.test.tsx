import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { TrackingWorkspacePage } from './TrackingWorkspacePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import {
  makeDrivePair,
  makeTrackedFile,
  makeTrackingItem,
  makeTrackingListResponse,
} from '@/test/factories'
import { renderWithApp } from '@/test/render'

function mockBaseTrackingPage(items = [makeTrackingItem()], drivePairs = [makeDrivePair()]) {
  server.use(
    api.get('/drives', () => HttpResponse.json(drivePairs)),
    api.get('/tracking/items', () => HttpResponse.json(makeTrackingListResponse(items))),
    api.get('/virtual-paths/tree', ({ request }) => {
      const parent = new URL(request.url).searchParams.get('parent') ?? '/'
      if (parent === '/docs') {
        return HttpResponse.json({ parent: '/docs', children: [] })
      }
      return HttpResponse.json({
        parent: '/',
        children: [{ name: 'docs', path: '/docs', item_count: 3, has_children: true }],
      })
    })
  )
}

describe('TrackingWorkspacePage', () => {
  it('renders a unified mixed list with source badges', async () => {
    mockBaseTrackingPage([
      makeTrackingItem({
        id: 11,
        kind: 'file',
        path: 'documents/report.pdf',
        source: 'direct',
        tracked_direct: true,
        tracked_via_folder: false,
      }),
      makeTrackingItem({
        id: 21,
        kind: 'folder',
        path: 'documents',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'partial',
        folder_total_files: 4,
        folder_mirrored_files: 2,
      }),
    ])

    renderWithApp(<TrackingWorkspacePage />)

    expect(await screen.findByTestId('page-title')).toHaveTextContent('Tracking Workspace')
    expect(screen.getByTestId('page-subtitle')).toHaveTextContent(
      'Track files and folders, manage virtual paths, and inspect item details.'
    )
    const fileRow = await screen.findByTestId('file-row-11')
    const folderRow = screen.getByTestId('folder-row-21')

    expect(within(fileRow).getByText('Direct')).toBeInTheDocument()
    expect(within(folderRow).getAllByText('Folder').length).toBeGreaterThan(0)
    expect(within(folderRow).getByText('Partial (2/4)')).toBeInTheDocument()
  })

  it('renders folder status badges with counts for all status variants', async () => {
    mockBaseTrackingPage([
      makeTrackingItem({
        id: 100,
        kind: 'folder',
        path: 'not-scanned-folder',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'not_scanned',
        folder_total_files: 0,
        folder_mirrored_files: 0,
      }),
      makeTrackingItem({
        id: 101,
        kind: 'folder',
        path: 'empty-folder',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'empty',
        folder_total_files: 0,
        folder_mirrored_files: 0,
      }),
      makeTrackingItem({
        id: 102,
        kind: 'folder',
        path: 'tracked-folder',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'tracked',
        folder_total_files: 10,
        folder_mirrored_files: 0,
      }),
      makeTrackingItem({
        id: 103,
        kind: 'folder',
        path: 'partial-folder',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'partial',
        folder_total_files: 10,
        folder_mirrored_files: 4,
      }),
      makeTrackingItem({
        id: 104,
        kind: 'folder',
        path: 'mirrored-folder',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
        folder_status: 'mirrored',
        folder_total_files: 10,
        folder_mirrored_files: 10,
      }),
    ])

    renderWithApp(<TrackingWorkspacePage />)

    const notScannedRow = await screen.findByTestId('folder-row-100')
    const emptyRow = await screen.findByTestId('folder-row-101')
    const trackedRow = screen.getByTestId('folder-row-102')
    const partialRow = screen.getByTestId('folder-row-103')
    const mirroredRow = screen.getByTestId('folder-row-104')

    expect(within(notScannedRow).getByText('Not scanned')).toBeInTheDocument()
    expect(within(notScannedRow).queryByText('(0/0)')).not.toBeInTheDocument()
    expect(within(emptyRow).getByText('Empty')).toBeInTheDocument()
    expect(within(emptyRow).queryByText('(0/0)')).not.toBeInTheDocument()
    expect(within(trackedRow).getByText('Tracked (10/10)')).toBeInTheDocument()
    expect(within(partialRow).getByText('Partial (4/10)')).toBeInTheDocument()
    expect(within(mirroredRow).getByText('Mirrored (10/10)')).toBeInTheDocument()
  })

  it('removes the legacy Both source option from filters', async () => {
    mockBaseTrackingPage()
    renderWithApp(<TrackingWorkspacePage />)
    await screen.findByTestId('tracking-table')
    expect(screen.queryByRole('option', { name: 'Both' })).not.toBeInTheDocument()
  })

  it('disables top-level tracking actions and shows helper text when no drive pairs exist', async () => {
    const user = userEvent.setup()

    mockBaseTrackingPage([makeTrackingItem()], [])
    renderWithApp(<TrackingWorkspacePage />)

    const trackFileButton = await screen.findByTestId('track-file-btn')
    const addFolderButton = screen.getByTestId('add-folder-button')

    expect(trackFileButton).toBeDisabled()
    expect(addFolderButton).toBeDisabled()
    expect(await screen.findByTestId('tracking-no-drives-hint')).toHaveTextContent(
      'Add a drive pair first to track files or folders.'
    )

    await user.click(trackFileButton)
    await user.click(addFolderButton)

    expect(screen.queryByText('Track new file')).not.toBeInTheDocument()
    expect(screen.queryByText('Add Tracked Folder')).not.toBeInTheDocument()
  })

  it('applies all filter dropdown selections to tracking queries', async () => {
    const seenParams: Array<{
      drive_id: string | null
      item_kind: string | null
      source: string | null
      has_virtual_path: string | null
    }> = []

    server.use(
      api.get('/drives', () =>
        HttpResponse.json([makeDrivePair(), makeDrivePair({ id: 2, name: 'Archive Mirror' })])
      ),
      api.get('/tracking/items', ({ request }) => {
        const params = new URL(request.url).searchParams
        seenParams.push({
          drive_id: params.get('drive_id'),
          item_kind: params.get('item_kind'),
          source: params.get('source'),
          has_virtual_path: params.get('has_virtual_path'),
        })
        return HttpResponse.json(makeTrackingListResponse([]))
      }),
      api.get('/virtual-paths/tree', () => HttpResponse.json({ parent: '/', children: [] }))
    )

    const user = userEvent.setup()
    renderWithApp(<TrackingWorkspacePage />)

    await waitFor(() => {
      expect(seenParams.length).toBeGreaterThan(0)
    })
    const [driveSelect, kindSelect, sourceSelect, virtualPathSelect] =
      screen.getAllByRole('combobox')

    await user.selectOptions(driveSelect, '2')
    await user.selectOptions(kindSelect, 'folder')
    await user.selectOptions(sourceSelect, 'folder')
    await user.selectOptions(virtualPathSelect, 'yes')
    await user.selectOptions(virtualPathSelect, 'no')

    await waitFor(() => {
      expect(
        seenParams.some(
          (params) =>
            params.drive_id === '2' &&
            params.item_kind === 'folder' &&
            params.source === 'folder' &&
            params.has_virtual_path === 'false'
        )
      ).toBe(true)
    })
  })

  it('removes the redundant virtual path prefix text filter', async () => {
    mockBaseTrackingPage()
    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('tracking-table')
    expect(screen.queryByPlaceholderText('Virtual path prefix (/docs)')).not.toBeInTheDocument()
  })

  it('shows the bulk action bar for multi-select and can deselect all', async () => {
    const user = userEvent.setup()

    mockBaseTrackingPage([
      makeTrackingItem({ id: 11, kind: 'file', path: 'docs/a.txt' }),
      makeTrackingItem({ id: 12, kind: 'file', path: 'docs/b.txt' }),
    ])

    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('file-row-11')
    await user.click(screen.getByTestId('select-row-file-11'))
    expect(screen.getByTestId('tracking-bulk-actions')).toBeInTheDocument()
    expect(screen.getByTestId('selected-count')).toHaveTextContent('1 selected')

    await user.click(screen.getByTestId('select-row-file-12'))
    expect(screen.getByTestId('selected-count')).toHaveTextContent('2 selected')

    await user.click(screen.getByTestId('bulk-deselect'))
    expect(screen.queryByTestId('tracking-bulk-actions')).not.toBeInTheDocument()
  })

  it('mirrors selected files and folders from the bulk action bar', async () => {
    const user = userEvent.setup()
    let mirroredFiles = 0
    let mirroredFolders = 0

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () =>
        HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({ id: 11, kind: 'file', path: 'docs/a.txt' }),
            makeTrackingItem({
              id: 21,
              kind: 'folder',
              path: 'docs',
              source: 'folder',
              is_mirrored: null,
              tracked_direct: null,
              tracked_via_folder: null,
            }),
          ])
        )
      ),
      api.post('/files/11/mirror', () => {
        mirroredFiles += 1
        return HttpResponse.json({ mirrored: true })
      }),
      api.post('/folders/21/mirror', () => {
        mirroredFolders += 1
        return HttpResponse.json({ mirrored_files: 3 })
      }),
      api.get('/virtual-paths/tree', () => HttpResponse.json({ parent: '/', children: [] }))
    )

    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('file-row-11')
    await user.click(screen.getByTestId('select-row-file-11'))
    await user.click(screen.getByTestId('select-row-folder-21'))
    await user.click(screen.getByTestId('bulk-mirror'))

    await waitFor(() => {
      expect(mirroredFiles).toBe(1)
      expect(mirroredFolders).toBe(1)
    })
  })

  it('deletes mixed selected items from the bulk action bar', async () => {
    const user = userEvent.setup()
    let listCalls = 0
    let deletedFiles = 0
    let deletedFolders = 0

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () => {
        listCalls += 1
        if (listCalls === 1) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({ id: 11, kind: 'file', path: 'docs/a.txt' }),
              makeTrackingItem({
                id: 21,
                kind: 'folder',
                path: 'docs',
                source: 'folder',
                is_mirrored: null,
                tracked_direct: null,
                tracked_via_folder: null,
              }),
            ])
          )
        }
        return HttpResponse.json(makeTrackingListResponse([]))
      }),
      api.delete('/files/11', () => {
        deletedFiles += 1
        return new HttpResponse(null, { status: 204 })
      }),
      api.delete('/folders/21', () => {
        deletedFolders += 1
        return new HttpResponse(null, { status: 204 })
      }),
      api.get('/virtual-paths/tree', () => HttpResponse.json({ parent: '/', children: [] }))
    )

    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('file-row-11')
    await user.click(screen.getByTestId('select-row-file-11'))
    await user.click(screen.getByTestId('select-row-folder-21'))
    await user.click(screen.getByTestId('bulk-delete'))
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Confirm' })
    )

    await waitFor(() => {
      expect(deletedFiles).toBe(1)
      expect(deletedFolders).toBe(1)
      expect(screen.queryByTestId('file-row-11')).not.toBeInTheDocument()
      expect(screen.queryByTestId('folder-row-21')).not.toBeInTheDocument()
    })
  })

  it('moves details to the next file after delete and closes details after deleting the last file', async () => {
    const user = userEvent.setup()
    let listCalls = 0

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () => {
        listCalls += 1
        if (listCalls === 1) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({ id: 11, kind: 'file', path: 'docs/a.txt' }),
              makeTrackingItem({ id: 12, kind: 'file', path: 'docs/b.txt' }),
            ])
          )
        }
        if (listCalls === 2) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({ id: 12, kind: 'file', path: 'docs/b.txt' }),
            ])
          )
        }
        return HttpResponse.json(makeTrackingListResponse([]))
      }),
      api.get('/files/:id', ({ params }) => {
        const id = Number(params.id)
        return HttpResponse.json(
          makeTrackedFile({
            id,
            relative_path: id === 11 ? 'docs/a.txt' : 'docs/b.txt',
          })
        )
      }),
      api.delete('/files/11', () => new HttpResponse(null, { status: 204 })),
      api.delete('/files/12', () => new HttpResponse(null, { status: 204 })),
      api.get('/virtual-paths/tree', () => HttpResponse.json({ parent: '/', children: [] }))
    )

    renderWithApp(<TrackingWorkspacePage />)

    const firstRow = await screen.findByTestId('file-row-11')
    await user.click(firstRow)
    await waitFor(() => {
      expect(within(screen.getByTestId('file-details')).getByText('docs/a.txt')).toBeInTheDocument()
    })

    await user.click(screen.getByTestId('select-row-file-11'))
    await user.click(screen.getByTestId('bulk-delete'))
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Confirm' })
    )

    await waitFor(() => {
      expect(within(screen.getByTestId('file-details')).getByText('docs/b.txt')).toBeInTheDocument()
    })

    await user.click(screen.getByTestId('select-row-file-12'))
    await user.click(screen.getByTestId('bulk-delete'))
    await user.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Confirm' })
    )

    await waitFor(() => {
      expect(screen.queryByTestId('file-details')).not.toBeInTheDocument()
    })
  })

  it('filters table rows from virtual-tree clicks using folder-derived virtual paths', async () => {
    const seenVirtualPrefixes: Array<string | null> = []

    const docsItem = makeTrackingItem({
      id: 41,
      kind: 'file',
      path: 'documents/report.pdf',
      virtual_path: '/virtual/docs/report.pdf',
      source: 'folder',
      tracked_direct: false,
      tracked_via_folder: true,
    })
    const otherItem = makeTrackingItem({
      id: 42,
      kind: 'file',
      path: 'media/clip.mp4',
      virtual_path: '/virtual/media/clip.mp4',
      source: 'direct',
      tracked_direct: true,
      tracked_via_folder: false,
    })

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', ({ request }) => {
        const virtualPrefix = new URL(request.url).searchParams.get('virtual_prefix')
        seenVirtualPrefixes.push(virtualPrefix)
        if (virtualPrefix === '/virtual/docs') {
          return HttpResponse.json(makeTrackingListResponse([docsItem]))
        }
        return HttpResponse.json(makeTrackingListResponse([docsItem, otherItem]))
      }),
      api.get('/virtual-paths/tree', ({ request }) => {
        const parent = new URL(request.url).searchParams.get('parent') ?? '/'
        if (parent === '/virtual') {
          return HttpResponse.json({
            parent: '/virtual',
            children: [{ name: 'docs', path: '/virtual/docs', item_count: 3, has_children: false }],
          })
        }
        return HttpResponse.json({
          parent: '/',
          children: [{ name: 'virtual', path: '/virtual', item_count: 7, has_children: true }],
        })
      })
    )

    const user = userEvent.setup()
    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('file-row-41')
    expect(screen.getByTestId('file-row-42')).toBeInTheDocument()

    await user.click(screen.getByTestId('toggle-virtual-pane'))
    await user.click(await screen.findByTestId('tree-node-/virtual'))
    await user.click(await screen.findByTestId('tree-node-/virtual/docs'))

    await waitFor(() => {
      expect(seenVirtualPrefixes).toContain('/virtual/docs')
    })
    expect(screen.getByTestId('file-row-41')).toBeInTheDocument()
    expect(screen.queryByTestId('file-row-42')).not.toBeInTheDocument()
  })

  it('shows folder virtual path browse control in the set-path flow', async () => {
    const user = userEvent.setup()

    mockBaseTrackingPage([
      makeTrackingItem({
        id: 31,
        kind: 'folder',
        path: 'library',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
      }),
    ])

    renderWithApp(<TrackingWorkspacePage />)

    await screen.findByTestId('folder-row-31')
    await user.click(screen.getByRole('button', { name: 'Set Path' }))

    await screen.findByText('Set Folder Virtual Path')
    expect(screen.getByRole('button', { name: 'Browse' })).toBeInTheDocument()
  })

  it('switches folder action from Scan to Mirror after scan marks files unmirrored', async () => {
    const user = userEvent.setup()
    let listCalls = 0

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () => {
        listCalls += 1
        if (listCalls === 1) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({
                id: 31,
                kind: 'folder',
                path: 'library',
                source: 'folder',
                is_mirrored: null,
                tracked_direct: null,
                tracked_via_folder: null,
                folder_status: 'not_scanned',
                folder_total_files: 0,
                folder_mirrored_files: 0,
              }),
            ])
          )
        }
        if (listCalls === 2) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({
                id: 31,
                kind: 'folder',
                path: 'library',
                source: 'folder',
                is_mirrored: null,
                tracked_direct: null,
                tracked_via_folder: null,
                folder_status: 'tracked',
                folder_total_files: 2,
                folder_mirrored_files: 0,
              }),
            ])
          )
        }
        return HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({
              id: 31,
              kind: 'folder',
              path: 'library',
              source: 'folder',
              is_mirrored: null,
              tracked_direct: null,
              tracked_via_folder: null,
              folder_status: 'mirrored',
              folder_total_files: 2,
              folder_mirrored_files: 2,
            }),
          ])
        )
      }),
      api.post('/folders/31/scan', () =>
        HttpResponse.json({
          new_files: 2,
          changed_files: 0,
        })
      ),
      api.post('/folders/31/mirror', () =>
        HttpResponse.json({
          mirrored_files: 2,
        })
      ),
      api.get('/virtual-paths/tree', () =>
        HttpResponse.json({
          parent: '/',
          children: [],
        })
      )
    )

    renderWithApp(<TrackingWorkspacePage />)
    await screen.findByTestId('folder-row-31')

    await user.click(screen.getByRole('button', { name: 'Scan' }))
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Mirror' })).toBeInTheDocument()
    })

    await user.click(screen.getByRole('button', { name: 'Mirror' }))
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Scan' })).toBeInTheDocument()
    })
  })

  it('refreshes the virtual tree after folder scan mutations', async () => {
    const user = userEvent.setup()
    let listCalls = 0
    let treeCalls = 0

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () => {
        listCalls += 1
        if (listCalls === 1) {
          return HttpResponse.json(
            makeTrackingListResponse([
              makeTrackingItem({
                id: 51,
                kind: 'folder',
                path: 'library',
                source: 'folder',
                is_mirrored: null,
                tracked_direct: null,
                tracked_via_folder: null,
                folder_status: 'not_scanned',
                folder_total_files: 0,
                folder_mirrored_files: 0,
              }),
            ])
          )
        }
        return HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({
              id: 51,
              kind: 'folder',
              path: 'library',
              source: 'folder',
              is_mirrored: null,
              tracked_direct: null,
              tracked_via_folder: null,
              folder_status: 'tracked',
              folder_total_files: 1,
              folder_mirrored_files: 0,
            }),
          ])
        )
      }),
      api.post('/folders/51/scan', () =>
        HttpResponse.json({
          new_files: 1,
          changed_files: 0,
        })
      ),
      api.get('/virtual-paths/tree', () => {
        treeCalls += 1
        return HttpResponse.json({
          parent: '/',
          children: [{ name: 'docs', path: '/docs', item_count: 1, has_children: false }],
        })
      })
    )

    renderWithApp(<TrackingWorkspacePage />)
    await screen.findByTestId('folder-row-51')
    await user.click(screen.getByTestId('toggle-virtual-pane'))
    await waitFor(() => {
      expect(treeCalls).toBeGreaterThan(0)
    })
    const treeCallsBeforeScan = treeCalls

    await user.click(screen.getByRole('button', { name: 'Scan' }))
    await waitFor(() => {
      expect(treeCalls).toBeGreaterThan(treeCallsBeforeScan)
    })
  })

  it('starts collapsed and can expand then collapse the virtual paths pane', async () => {
    const user = userEvent.setup()
    mockBaseTrackingPage()
    renderWithApp(<TrackingWorkspacePage />)

    expect(screen.queryByTestId('tree-node-/docs')).not.toBeInTheDocument()
    await user.click(screen.getByTestId('toggle-virtual-pane'))
    await screen.findByTestId('tree-node-/docs')
    await user.click(screen.getByTestId('toggle-virtual-pane'))
    expect(screen.queryByTestId('tree-node-/docs')).not.toBeInTheDocument()

    await user.click(screen.getByTestId('toggle-virtual-pane'))
    expect(await screen.findByTestId('tree-node-/docs')).toBeInTheDocument()
  })

  it('shows full BLAKE3 checksum in file details', async () => {
    const user = userEvent.setup()
    const checksum = 'd74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24'

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () =>
        HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({
              id: 11,
              kind: 'file',
              path: 'documents/report.pdf',
            }),
          ])
        )
      ),
      api.get('/files/:id', () =>
        HttpResponse.json(
          makeTrackedFile({
            id: 11,
            relative_path: 'documents/report.pdf',
            checksum,
          })
        )
      ),
      api.get('/virtual-paths/tree', () =>
        HttpResponse.json({
          parent: '/',
          children: [],
        })
      )
    )

    renderWithApp(<TrackingWorkspacePage />)

    const fileRow = await screen.findByTestId('file-row-11')
    await user.click(fileRow)

    expect(await screen.findByText('Checksum (BLAKE3)')).toBeInTheDocument()
    expect(screen.getByText(checksum)).toBeInTheDocument()
    expect(
      within(screen.getByTestId('file-details')).getByText('Primary Mirror')
    ).toBeInTheDocument()
    expect(
      within(screen.getByTestId('file-details')).getByText('Last integrity check')
    ).toBeInTheDocument()
  })

  it('keeps effective virtual path in file details when file endpoint returns null virtual_path', async () => {
    const user = userEvent.setup()
    const effectiveVirtualPath = '/virtual/docs/report.pdf'

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', () =>
        HttpResponse.json(
          makeTrackingListResponse([
            makeTrackingItem({
              id: 61,
              kind: 'file',
              path: 'docs/report.pdf',
              virtual_path: effectiveVirtualPath,
              source: 'folder',
              tracked_direct: false,
              tracked_via_folder: true,
            }),
          ])
        )
      ),
      api.get('/files/:id', () =>
        HttpResponse.json(
          makeTrackedFile({
            id: 61,
            relative_path: 'docs/report.pdf',
            virtual_path: null,
          })
        )
      ),
      api.get('/virtual-paths/tree', () => HttpResponse.json({ parent: '/', children: [] }))
    )

    renderWithApp(<TrackingWorkspacePage />)
    const row = await screen.findByTestId('file-row-61')
    await user.click(row)

    await waitFor(() => {
      expect(
        within(screen.getByTestId('file-details')).getByText(effectiveVirtualPath)
      ).toBeInTheDocument()
    })
  })
})
