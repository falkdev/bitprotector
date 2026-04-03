import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { TrackingWorkspacePage } from './TrackingWorkspacePage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import {
  makeDrivePair,
  makeTrackingItem,
  makeTrackingListResponse,
} from '@/test/factories'
import { renderWithApp } from '@/test/render'

function mockBaseTrackingPage(items = [makeTrackingItem()]) {
  server.use(
    api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
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
        source: 'both',
        tracked_direct: true,
        tracked_via_folder: true,
      }),
      makeTrackingItem({
        id: 21,
        kind: 'folder',
        path: 'documents',
        source: 'folder',
        is_mirrored: null,
        tracked_direct: null,
        tracked_via_folder: null,
      }),
    ])

    renderWithApp(<TrackingWorkspacePage />)

    const fileRow = await screen.findByTestId('file-row-11')
    const folderRow = screen.getByTestId('folder-row-21')

    expect(within(fileRow).getByText('Both')).toBeInTheDocument()
    expect(within(folderRow).getAllByText('Folder').length).toBeGreaterThan(0)
  })

  it('keeps absolute publish prefixes when selecting virtual tree nodes', async () => {
    const seenPublishPrefixes: Array<string | null> = []

    server.use(
      api.get('/drives', () => HttpResponse.json([makeDrivePair()])),
      api.get('/tracking/items', ({ request }) => {
        const publishPrefix = new URL(request.url).searchParams.get('publish_prefix')
        seenPublishPrefixes.push(publishPrefix)
        return HttpResponse.json(makeTrackingListResponse([]))
      }),
      api.get('/virtual-paths/tree', ({ request }) => {
        const parent = new URL(request.url).searchParams.get('parent') ?? '/'
        if (parent === '/docs') {
          return HttpResponse.json({ parent: '/docs', children: [] })
        }
        return HttpResponse.json({
          parent: '/',
          children: [{ name: 'docs', path: '/docs', item_count: 7, has_children: true }],
        })
      })
    )

    const user = userEvent.setup()
    renderWithApp(<TrackingWorkspacePage />)

    const docsNode = await screen.findByTestId('tree-node-/docs')
    await user.click(docsNode)

    await waitFor(() => {
      expect(seenPublishPrefixes).toContain('/docs')
    })
  })

  it('shows folder publish path browse control in the set-path flow', async () => {
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

    await screen.findByText('Set Folder Publish Path')
    expect(screen.getByRole('button', { name: 'Browse' })).toBeInTheDocument()
  })
})
