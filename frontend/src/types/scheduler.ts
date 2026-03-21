export type ScheduleTaskType = 'sync' | 'integrity_check'

export interface ScheduleConfig {
  id: number
  task_type: ScheduleTaskType
  cron_expr: string | null
  interval_seconds: number | null
  enabled: boolean
  last_run: string | null
  next_run: string | null
  created_at: string
  updated_at: string
}

export interface ScheduleListResponse {
  schedules: ScheduleConfig[]
}

export interface CreateScheduleRequest {
  task_type: ScheduleTaskType
  cron_expr?: string
  interval_seconds?: number
  enabled?: boolean
}

export interface UpdateScheduleRequest {
  cron_expr?: string | null
  interval_seconds?: number | null
  enabled?: boolean
}
