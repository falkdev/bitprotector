import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { authApi } from './auth'
import { server } from '@/test/msw/server'
import { api, apiError } from '@/test/msw/http'

describe('authApi', () => {
  it('login returns token payload', async () => {
    server.use(
      api.post('/auth/login', () =>
        HttpResponse.json({ token: 'jwt', username: 'alice', expires_at: '2026-01-01T00:00:00Z' })
      )
    )

    await expect(authApi.login({ username: 'alice', password: 'pw' })).resolves.toMatchObject({
      token: 'jwt',
    })
  })

  it('validate rejects with API error', async () => {
    server.use(api.get('/auth/validate', () => apiError(401, 'Unauthorized')))
    await expect(authApi.validate()).rejects.toBeTruthy()
  })
})
