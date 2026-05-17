import { beforeEach, describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { makeDrivePair } from '@/test/factories'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'
import { useDrivesStore } from './drives-store'

function resetStore() {
  useDrivesStore.setState({ drives: [], loading: false, error: null })
}

describe('drives-store', () => {
  beforeEach(() => {
    resetStore()
  })

  it('fetch sets drives and clears loading on success', async () => {
    const drive = makeDrivePair()
    server.use(api.get('/drives', () => HttpResponse.json([drive])))

    await useDrivesStore.getState().fetch()

    expect(useDrivesStore.getState().drives).toEqual([drive])
    expect(useDrivesStore.getState().loading).toBe(false)
    expect(useDrivesStore.getState().error).toBeNull()
  })

  it('fetch sets error on failure', async () => {
    server.use(
      api.get('/drives', () => HttpResponse.json({ error: 'server error' }, { status: 500 }))
    )

    await useDrivesStore.getState().fetch()

    expect(useDrivesStore.getState().error).toBeTruthy()
    expect(useDrivesStore.getState().loading).toBe(false)
  })

  it('create appends drive to list', async () => {
    const drive = makeDrivePair()
    server.use(api.post('/drives', () => HttpResponse.json(drive)))

    const result = await useDrivesStore.getState().create({
      name: drive.name,
      primary_path: drive.primary_path,
      secondary_path: drive.secondary_path,
    })

    expect(result).toMatchObject({ id: drive.id, name: drive.name })
    expect(useDrivesStore.getState().drives).toHaveLength(1)
  })

  it('update replaces drive in list', async () => {
    const original = makeDrivePair({ id: 1, name: 'Old Name' })
    const updated = makeDrivePair({ id: 1, name: 'New Name' })
    useDrivesStore.setState({ drives: [original] })
    server.use(api.put('/drives/1', () => HttpResponse.json(updated)))

    const result = await useDrivesStore.getState().update(1, { name: 'New Name' })

    expect(result.name).toBe('New Name')
    expect(useDrivesStore.getState().drives[0].name).toBe('New Name')
  })

  it('update replaces only the matching drive when multiple drives exist', async () => {
    const drive1 = makeDrivePair({ id: 1, name: 'Drive 1' })
    const drive2 = makeDrivePair({ id: 2, name: 'Drive 2' })
    const updated = makeDrivePair({ id: 1, name: 'Updated Drive 1' })
    useDrivesStore.setState({ drives: [drive1, drive2] })
    server.use(api.put('/drives/1', () => HttpResponse.json(updated)))

    await useDrivesStore.getState().update(1, { name: 'Updated Drive 1' })

    expect(useDrivesStore.getState().drives[0].name).toBe('Updated Drive 1')
    expect(useDrivesStore.getState().drives[1].name).toBe('Drive 2')
  })

  it('remove deletes drive from list', async () => {
    const drive = makeDrivePair({ id: 1 })
    useDrivesStore.setState({ drives: [drive] })
    server.use(api.delete('/drives/1', () => new HttpResponse(null, { status: 204 })))

    await useDrivesStore.getState().remove(1)

    expect(useDrivesStore.getState().drives).toHaveLength(0)
  })

  it('refresh updates existing drive in list', () => {
    const original = makeDrivePair({ id: 1, name: 'Old' })
    const other = makeDrivePair({ id: 2, name: 'Other' })
    useDrivesStore.setState({ drives: [original, other] })
    const updated = makeDrivePair({ id: 1, name: 'Updated' })

    useDrivesStore.getState().refresh(updated)

    expect(useDrivesStore.getState().drives[0].name).toBe('Updated')
    expect(useDrivesStore.getState().drives[1].name).toBe('Other')
  })

  it('refresh adds drive if not found in list', () => {
    useDrivesStore.setState({ drives: [] })
    const drive = makeDrivePair({ id: 99 })

    useDrivesStore.getState().refresh(drive)

    expect(useDrivesStore.getState().drives).toContain(drive)
  })
})
