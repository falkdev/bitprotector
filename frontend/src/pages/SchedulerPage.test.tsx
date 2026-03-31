import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { HttpResponse } from 'msw'
import { describe, expect, it } from 'vitest'
import { SchedulerPage } from './SchedulerPage'
import { api } from '@/test/msw/http'
import { server } from '@/test/msw/server'
import { makeSchedule } from '@/test/factories'
import { renderWithApp } from '@/test/render'

describe('SchedulerPage', () => {
  it('creates a schedule through the mocked scheduler API', async () => {
    const user = userEvent.setup()
    const schedules = [makeSchedule()]
    let createBody: unknown = null

    server.use(
      api.get('/scheduler/schedules', () => HttpResponse.json({ schedules })),
      api.post('/scheduler/schedules', async ({ request }) => {
        createBody = await request.json()
        const created = makeSchedule({
          id: 2,
          task_type: 'integrity_check',
          cron_expr: null,
          interval_seconds: 3600,
        })
        schedules.push(created)
        return HttpResponse.json(created)
      })
    )

    renderWithApp(<SchedulerPage />)

    await screen.findByTestId('schedule-row-1')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))
    await user.selectOptions(screen.getByLabelText('Task Type'), 'integrity_check')
    await user.type(screen.getByLabelText('Interval Seconds'), '3600')
    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(await screen.findByText('Schedule created')).toBeInTheDocument()
    expect(createBody).toEqual({
      task_type: 'integrity_check',
      interval_seconds: 3600,
      enabled: true,
    })
  })

  it('shows validation feedback when neither cron nor interval is provided', async () => {
    const user = userEvent.setup()

    server.use(api.get('/scheduler/schedules', () => HttpResponse.json({ schedules: [] })))

    renderWithApp(<SchedulerPage />)

    await screen.findByText('No schedules configured')
    await user.click(screen.getByRole('button', { name: 'Add Schedule' }))
    await user.click(screen.getByRole('button', { name: 'Create Schedule' }))

    expect(
      await screen.findByText('Provide either a cron expression or an interval in seconds.')
    ).toBeInTheDocument()
  })
})
