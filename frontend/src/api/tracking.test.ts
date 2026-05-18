import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { trackingApi } from './tracking'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'
import {
  makeTrackedFile,
  makeTrackedFolder,
  makeTrackingListResponse,
  makeTrackingItem,
} from '@/test/factories'

/** Set up the fallback (404) mode with specific files and folders */
function useFallback(
  files: ReturnType<typeof makeTrackedFile>[],
  folders: ReturnType<typeof makeTrackedFolder>[]
) {
  server.use(
    api.get('/tracking/items', () => apiError(404, 'not found')),
    api.get('/files', () =>
      HttpResponse.json({ files, total: files.length, page: 1, per_page: 200 })
    ),
    api.get('/folders', () => HttpResponse.json(folders))
  )
}

describe('trackingApi', () => {
  it('list returns direct tracking endpoint response when available', async () => {
    const item = makeTrackingItem({ id: 1 })
    server.use(
      api.get('/tracking/items', () => HttpResponse.json(makeTrackingListResponse([item])))
    )
    await expect(trackingApi.list()).resolves.toMatchObject({ total: 1 })
  })

  it('list falls back to files+folders when tracking endpoint is missing', async () => {
    useFallback(
      [makeTrackedFile({ id: 10, relative_path: 'docs/a.txt' })],
      [makeTrackedFolder({ id: 11, folder_path: 'docs', virtual_path: '/v/docs' })]
    )

    const result = await trackingApi.list({ page: 1, per_page: 50 })
    expect(result.items.length).toBeGreaterThan(0)
    expect(result.total).toBeGreaterThan(0)
  })

  it('list propagates non-404 errors from tracking endpoint', async () => {
    server.use(api.get('/tracking/items', () => apiError(500, 'tracking failed')))
    await expect(trackingApi.list()).rejects.toBeTruthy()
  })

  describe('fallback — source filter', () => {
    it('source=folder returns only folder items', async () => {
      useFallback(
        [makeTrackedFile({ id: 1, relative_path: 'docs/file.txt' })],
        [makeTrackedFolder({ id: 2, folder_path: 'docs' })]
      )
      const result = await trackingApi.list({ source: 'folder' })
      expect(result.items.every((i) => i.kind === 'folder')).toBe(true)
    })

    it('source=direct returns only direct-tracked files', async () => {
      useFallback([makeTrackedFile({ id: 1, tracked_direct: true, tracked_via_folder: false })], [])
      const result = await trackingApi.list({ source: 'direct' })
      expect(result.items.every((i) => i.kind === 'file')).toBe(true)
    })
  })

  describe('fallback — has_virtual_path filter', () => {
    it('has_virtual_path=true returns only items with virtual path', async () => {
      useFallback(
        [
          makeTrackedFile({ id: 1, virtual_path: '/v/file.txt' }),
          makeTrackedFile({ id: 2, virtual_path: null }),
        ],
        []
      )
      const result = await trackingApi.list({ has_virtual_path: true })
      expect(result.items.every((i) => !!i.virtual_path)).toBe(true)
    })

    it('has_virtual_path=false returns only items without virtual path', async () => {
      useFallback(
        [
          makeTrackedFile({ id: 1, virtual_path: '/v/file.txt' }),
          makeTrackedFile({ id: 2, virtual_path: null }),
        ],
        []
      )
      const result = await trackingApi.list({ has_virtual_path: false })
      expect(result.items.every((i) => !i.virtual_path)).toBe(true)
    })
  })

  describe('fallback — virtual_prefix filter', () => {
    it('virtual_prefix filters by prefix match', async () => {
      useFallback(
        [
          makeTrackedFile({ id: 1, virtual_path: '/media/photo.jpg' }),
          makeTrackedFile({ id: 2, virtual_path: '/docs/report.txt' }),
        ],
        []
      )
      const result = await trackingApi.list({ virtual_prefix: '/media/' })
      expect(result.items).toHaveLength(1)
      expect(result.items[0].virtual_path).toBe('/media/photo.jpg')
    })

    it('virtual_prefix handles items with null virtual_path', async () => {
      useFallback([makeTrackedFile({ id: 1, virtual_path: null })], [])
      const result = await trackingApi.list({ virtual_prefix: '/media/' })
      expect(result.items).toHaveLength(0)
    })
  })

  describe('fallback — q search filter', () => {
    it('q matches against item path', async () => {
      useFallback(
        [
          makeTrackedFile({ id: 1, relative_path: 'reports/annual.pdf' }),
          makeTrackedFile({ id: 2, relative_path: 'photos/vacation.jpg' }),
        ],
        []
      )
      const result = await trackingApi.list({ q: 'annual' })
      expect(result.items).toHaveLength(1)
    })

    it('q matches against virtual_path when path does not match', async () => {
      useFallback(
        [makeTrackedFile({ id: 1, relative_path: 'x/y/z.txt', virtual_path: '/tagged/document' })],
        []
      )
      const result = await trackingApi.list({ q: 'tagged' })
      expect(result.items).toHaveLength(1)
    })

    it('q is case-insensitive', async () => {
      useFallback([makeTrackedFile({ id: 1, relative_path: 'Reports/Annual.PDF' })], [])
      const result = await trackingApi.list({ q: 'annual' })
      expect(result.items).toHaveLength(1)
    })
  })

  describe('fallback — item_kind filter', () => {
    it('item_kind=file returns only file items', async () => {
      useFallback([makeTrackedFile({ id: 1 })], [makeTrackedFolder({ id: 2 })])
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items.every((i) => i.kind === 'file')).toBe(true)
    })

    it('item_kind=folder returns only folder items', async () => {
      useFallback([makeTrackedFile({ id: 1 })], [makeTrackedFolder({ id: 2 })])
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items.every((i) => i.kind === 'folder')).toBe(true)
    })
  })

  describe('fallback — drive_id filter', () => {
    it('drive_id filters items by drive pair', async () => {
      useFallback(
        [
          makeTrackedFile({ id: 1, drive_pair_id: 1 }),
          makeTrackedFile({ id: 2, drive_pair_id: 2 }),
        ],
        []
      )
      const result = await trackingApi.list({ drive_id: 2 })
      expect(result.items.every((i) => i.drive_pair_id === 2)).toBe(true)
    })
  })

  describe('fallback — folder status', () => {
    it('folder status is mirrored when all files in folder are mirrored', async () => {
      useFallback(
        [
          makeTrackedFile({
            id: 1,
            drive_pair_id: 1,
            relative_path: 'docs/f.txt',
            is_mirrored: true,
          }),
        ],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            last_scanned_at: '2026-01-01T00:00:00Z',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items[0]).toMatchObject({ folder_status: 'mirrored' })
    })

    it('folder status is tracked when no files are mirrored', async () => {
      useFallback(
        [
          makeTrackedFile({
            id: 1,
            drive_pair_id: 1,
            relative_path: 'docs/f.txt',
            is_mirrored: false,
          }),
        ],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            last_scanned_at: '2026-01-01T00:00:00Z',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items[0]).toMatchObject({ folder_status: 'tracked' })
    })

    it('folder status is partial when some files are mirrored', async () => {
      useFallback(
        [
          makeTrackedFile({
            id: 1,
            drive_pair_id: 1,
            relative_path: 'docs/a.txt',
            is_mirrored: true,
          }),
          makeTrackedFile({
            id: 2,
            drive_pair_id: 1,
            relative_path: 'docs/b.txt',
            is_mirrored: false,
          }),
        ],
        [
          makeTrackedFolder({
            id: 3,
            drive_pair_id: 1,
            folder_path: 'docs',
            last_scanned_at: '2026-01-01T00:00:00Z',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items[0]).toMatchObject({ folder_status: 'partial' })
    })

    it('folder status is empty when scanned but no files in folder', async () => {
      useFallback(
        [],
        [
          makeTrackedFolder({
            id: 1,
            drive_pair_id: 1,
            folder_path: 'empty-dir',
            last_scanned_at: '2026-01-01T00:00:00Z',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items[0]).toMatchObject({ folder_status: 'empty' })
    })

    it('folder status is not_scanned when never scanned and no files', async () => {
      useFallback(
        [],
        [
          makeTrackedFolder({
            id: 1,
            drive_pair_id: 1,
            folder_path: 'new-dir',
            last_scanned_at: null,
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'folder' })
      expect(result.items[0]).toMatchObject({ folder_status: 'not_scanned' })
    })
  })

  describe('fallback — deriveFileVirtualPath', () => {
    it('uses file own virtual_path when set', async () => {
      useFallback(
        [makeTrackedFile({ id: 1, virtual_path: '/explicit/path', relative_path: 'docs/f.txt' })],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            virtual_path: '/v/docs',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items[0].virtual_path).toBe('/explicit/path')
    })

    it('derives virtual_path from folder when file is at folder root', async () => {
      useFallback(
        [makeTrackedFile({ id: 1, virtual_path: null, relative_path: 'docs', drive_pair_id: 1 })],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            virtual_path: '/v/docs',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items[0].virtual_path).toBe('/v/docs')
    })

    it('derives virtual_path by appending file suffix to folder virtual_path', async () => {
      useFallback(
        [
          makeTrackedFile({
            id: 1,
            virtual_path: null,
            relative_path: 'docs/sub/file.txt',
            drive_pair_id: 1,
          }),
        ],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            virtual_path: '/v/docs/',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items[0].virtual_path).toBe('/v/docs/sub/file.txt')
    })

    it('returns null virtual_path when no matching folder', async () => {
      useFallback(
        [
          makeTrackedFile({
            id: 1,
            virtual_path: null,
            relative_path: 'other/file.txt',
            drive_pair_id: 1,
          }),
        ],
        [
          makeTrackedFolder({
            id: 2,
            drive_pair_id: 1,
            folder_path: 'docs',
            virtual_path: '/v/docs',
          }),
        ]
      )
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items[0].virtual_path).toBeNull()
    })
  })

  describe('fallback — fileSource', () => {
    it('source is folder when tracked_via_folder=true and tracked_direct=false', async () => {
      useFallback([makeTrackedFile({ id: 1, tracked_direct: false, tracked_via_folder: true })], [])
      const result = await trackingApi.list({ item_kind: 'file' })
      expect(result.items[0].source).toBe('folder')
    })
  })
})
