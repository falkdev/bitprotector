import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { SchedulerPage } from './SchedulerPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeSchedule } from '@/test/factories'
import { renderWithApp } from '@/test/render'

function localeHour(hour: number): string {
  const locale =
    typeof navigator !== 'undefined' && navigator.language ? navigator.language : undefined
  return new Date(2000, 0, 1, hour, 0).toLocaleTimeString(locale, {
    hour: '2-digit',
    minute: '2-digit',
  })
}

function useScheduleHandlers(initialSchedules = [makeSchedule()]) {
  const schedules = [...initialSchedules]
  let createBody: unknown = null

  const handlers = [
    api.get('/scheduler/schedules', () => HttpResponse.json({ schedules })),
    api.post('/scheduler/schedules', async ({ request }) => {
      createBody = await request.json()
      const created = makeSchedule({
        id: schedules.length + 1,
        ...(createBody as Record<string, unknown>),
      })
      schedules.push(created)
      return HttpResponse.json(created)
    }),
  ]

  return { schedules, handlers, getCreateBody: () => createBody }
}

describe('SchedulerPage', () => {
  it('creates an integrity check schedule with a recurring interval', async () => {
    const user = userEvent.setup()
    const { handlers, getCreateBody } = useScheduleHandlers()

    server.use(...handlers)

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    // Select Integrity Check task type via radio card
    await user.click(screen.getByRole('button', { name: /Integrity Check/i }))

    // Fill interval (default timing method is "Recurring Interval")
    const intervalInput = screen.getByLabelText('Interval value')
    await user.clear(intervalInput)
    await user.type(intervalInput, '1')
    await user.selectOptions(screen.getByLabelText('Interval unit'), 'hours')

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Schedule created')).toBeInTheDocument()
    expect(getCreateBody()).toEqual({
      task_type: 'integrity_check',
      interval_seconds: 3600,
      enabled: true,
    })
  })

  it('shows validation feedback when interval value is empty', async () => {
    const user = userEvent.setup()

    server.use(api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [] })))

    renderWithApp(<SchedulerPage />)

    await screen.findByText('No schedules configured')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    // Clear the default interval value
    const intervalInput = screen.getByLabelText('Interval value')
    await user.clear(intervalInput)

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Interval must be a positive number.')).toBeInTheDocument()
  })

  it('creates a schedule with a cron preset', async () => {
    const user = userEvent.setup()
    const { handlers, getCreateBody } = useScheduleHandlers()

    server.use(...handlers)

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    // Switch to cron timing method
    await user.click(screen.getByRole('button', { name: 'Cron Schedule' }))

    // Select a preset (label uses locale time format)
    await user.click(screen.getByRole('button', { name: `Daily at ${localeHour(2)}` }))

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Schedule created')).toBeInTheDocument()
    expect(getCreateBody()).toEqual({
      task_type: 'sync',
      cron_expr: '0 2 * * *',
      enabled: true,
    })
  })

  it('creates a schedule with a custom cron expression', async () => {
    const user = userEvent.setup()
    const { handlers, getCreateBody } = useScheduleHandlers()

    server.use(...handlers)

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    // Switch to cron timing method
    await user.click(screen.getByRole('button', { name: 'Cron Schedule' }))

    // Click Custom…
    await user.click(screen.getByRole('button', { name: /Custom/ }))

    // Type custom cron expression
    await user.type(screen.getByLabelText('Custom cron expression'), '30 4 * * 1-5')

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Schedule created')).toBeInTheDocument()
    expect(getCreateBody()).toEqual({
      task_type: 'sync',
      cron_expr: '30 4 * * 1-5',
      enabled: true,
    })
  })

  it('shows validation error when cron is selected but no expression provided', async () => {
    const user = userEvent.setup()

    server.use(api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [] })))

    renderWithApp(<SchedulerPage />)

    await screen.findByText('No schedules configured')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    // Switch to cron timing method
    await user.click(screen.getByRole('button', { name: 'Cron Schedule' }))

    // Click Custom… but don't type anything
    await user.click(screen.getByRole('button', { name: /Custom/ }))

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(
      await screen.findByText('Select a preset or enter a custom cron expression.')
    ).toBeInTheDocument()
  })

  it('converts interval units correctly (2 hours = 7200 seconds)', async () => {
    const user = userEvent.setup()
    const { handlers, getCreateBody } = useScheduleHandlers()

    server.use(...handlers)

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))

    const intervalInput = screen.getByLabelText('Interval value')
    await user.clear(intervalInput)
    await user.type(intervalInput, '2')
    await user.selectOptions(screen.getByLabelText('Interval unit'), 'hours')

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Schedule created')).toBeInTheDocument()
    expect(getCreateBody()).toEqual({
      task_type: 'sync',
      interval_seconds: 7200,
      enabled: true,
    })
  })

  it('displays human-friendly schedule descriptions in the table', async () => {
    server.use(
      api.get('/scheduler/schedules', () =>
        HttpResponse.json({
          schedules: [
            makeSchedule({
              id: 1,
              task_type: 'sync',
              cron_expr: '0 2 * * *',
              interval_seconds: null,
            }),
            makeSchedule({
              id: 2,
              task_type: 'integrity_check',
              cron_expr: null,
              interval_seconds: 3600,
            }),
            makeSchedule({ id: 3, task_type: 'sync', cron_expr: null, interval_seconds: 120 }),
          ],
        })
      )
    )

    renderWithApp(<SchedulerPage />)

    const row1 = await screen.findByTestId('schedule-row-1')
    expect(within(row1).getByText('File Sync')).toBeInTheDocument()
    expect(within(row1).getByText(`Daily at ${localeHour(2)}`)).toBeInTheDocument()

    const row2 = screen.getByTestId('schedule-row-2')
    expect(within(row2).getByText('Integrity Check')).toBeInTheDocument()
    expect(within(row2).getByText('Every hour')).toBeInTheDocument()

    const row3 = screen.getByTestId('schedule-row-3')
    expect(within(row3).getByText('Every 2 minutes')).toBeInTheDocument()
  })

  it('edits an existing schedule (changes interval)', async () => {
    const user = userEvent.setup()
    const existing = makeSchedule({
      id: 1,
      task_type: 'sync',
      interval_seconds: 3600,
      cron_expr: null,
    })
    let updateBody: unknown = null

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [existing] })),
      api.put('/scheduler/schedules/1', async ({ request }) => {
        updateBody = await request.json()
        return HttpResponse.json({ ...existing, interval_seconds: 7200, ...(updateBody as object) })
      })
    )

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Edit' }))

    const intervalInput = screen.getByLabelText('Interval value')
    await user.clear(intervalInput)
    await user.type(intervalInput, '2')
    await user.selectOptions(screen.getByLabelText('Interval unit'), 'hours')

    await user.click(screen.getByRole('button', { name: 'Save Changes' }))

    expect(await screen.findByText('Schedule updated')).toBeInTheDocument()
    expect((updateBody as Record<string, unknown>).interval_seconds).toBe(7200)
  })

  it('deletes a schedule after confirmation', async () => {
    const user = userEvent.setup()
    const existing = makeSchedule({
      id: 1,
      task_type: 'integrity_check',
      interval_seconds: 3600,
      cron_expr: null,
    })

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [existing] })),
      api.delete('/scheduler/schedules/1', () => new HttpResponse(null, { status: 204 }))
    )

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Delete' }))

    const dialog = await screen.findByRole('alertdialog')
    await user.click(within(dialog).getByRole('button', { name: 'Delete' }))

    expect(await screen.findByText('Schedule deleted')).toBeInTheDocument()
    expect(screen.queryByTestId('schedule-row-1')).not.toBeInTheDocument()
  })

  it('toggles schedule enabled/disabled', async () => {
    const user = userEvent.setup()
    const existing = makeSchedule({ id: 1, enabled: true, interval_seconds: 3600, cron_expr: null })

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [existing] })),
      api.put('/scheduler/schedules/1', async ({ request }) => {
        const body = (await request.json()) as Record<string, unknown>
        return HttpResponse.json({ ...existing, enabled: body.enabled as boolean })
      })
    )

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    const row1 = screen.getByTestId('schedule-row-1')
    await user.click(within(row1).getByRole('button', { name: 'Enabled' }))

    expect(await screen.findByText('Schedule disabled')).toBeInTheDocument()
  })

  it('shows error toast when loading schedules fails', async () => {
    server.use(api.get('/scheduler/schedules', () => HttpResponse.json({}, { status: 500 })))

    renderWithApp(<SchedulerPage />)

    expect(await screen.findByText('Failed to load schedules')).toBeInTheDocument()
  })

  it('shows error toast when saving a schedule fails', async () => {
    const user = userEvent.setup()

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [] })),
      api.post('/scheduler/schedules', () => HttpResponse.json({}, { status: 500 }))
    )

    renderWithApp(<SchedulerPage />)
    await user.click(await screen.findByRole('button', { name: 'Add Schedule' }))
    await user.click(screen.getByRole('button', { name: /Integrity Check/i }))
    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Failed to save schedule')).toBeInTheDocument()
  })

  it('toggles enabled state with multiple schedules present', async () => {
    const user = userEvent.setup()
    const s1 = makeSchedule({ id: 1, enabled: true, interval_seconds: 3600, cron_expr: null })
    const s2 = makeSchedule({ id: 2, enabled: true, interval_seconds: 7200, cron_expr: null })

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [s1, s2] })),
      api.put('/scheduler/schedules/2', async ({ request }) => {
        const body = (await request.json()) as Record<string, unknown>
        return HttpResponse.json({ ...s2, enabled: body.enabled as boolean })
      })
    )

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await screen.findByTestId('schedule-row-2')

    const row2 = screen.getByTestId('schedule-row-2')
    await user.click(within(row2).getByRole('button', { name: 'Enabled' }))

    expect(await screen.findByText('Schedule disabled')).toBeInTheDocument()
    // Both rows still visible
    expect(screen.getByTestId('schedule-row-1')).toBeInTheDocument()
    expect(screen.getByTestId('schedule-row-2')).toBeInTheDocument()
  })

  it('sets enabled=false and max_duration_seconds when creating a schedule', async () => {
    const user = userEvent.setup()
    const { handlers, getCreateBody } = useScheduleHandlers()

    server.use(...handlers)

    renderWithApp(<SchedulerPage />)
    await screen.findByTestId('schedule-row-1')

    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))
    await user.click(screen.getByRole('button', { name: /Integrity Check/i }))

    // Uncheck the "Enable schedule immediately" checkbox
    await user.click(screen.getByRole('checkbox', { name: /Enable schedule immediately/i }))

    // Set a max duration
    await user.clear(screen.getByLabelText('Max duration in minutes'))
    await user.type(screen.getByLabelText('Max duration in minutes'), '2')

    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))
    expect(await screen.findByText('Schedule created')).toBeInTheDocument()

    expect(getCreateBody()).toEqual(
      expect.objectContaining({ enabled: false, max_duration_seconds: 120 })
    )
  })
})
