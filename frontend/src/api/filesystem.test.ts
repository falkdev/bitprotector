import { describe, expect, it } from 'vitest'
import { HttpResponse } from 'msw'
import { filesystemApi, getFilesystemBrowserErrorMessage } from './filesystem'
import { server } from '@/test/msw/server'
import { api } from '@/test/msw/http'

describe('getFilesystemBrowserErrorMessage', () => {
  it('returns a rebuild hint for 404 responses', () => {
    const error = {
      isAxiosError: true,
      message: 'Request failed with status code 404',
      response: {
        status: 404,
      },
    }

    expect(getFilesystemBrowserErrorMessage(error)).toBe(
      'The running BitProtector API does not expose the filesystem browser endpoint yet. Rebuild and restart the backend, then refresh the page.'
    )
  })

  it('prefers API validation messages when present', () => {
    const error = {
      isAxiosError: true,
      message: 'Request failed with status code 400',
      response: {
        status: 400,
        data: {
          error: {
            message: 'Path is not a directory',
          },
        },
      },
    }

    expect(getFilesystemBrowserErrorMessage(error)).toBe('Path is not a directory')
  })

  it('uses axios error message when no API error message present', () => {
    const error = {
      isAxiosError: true,
      message: 'Network Error',
      response: {
        status: 500,
        data: {},
      },
    }

    expect(getFilesystemBrowserErrorMessage(error)).toBe('Network Error')
  })

  it('falls back to generic error messages for non-axios errors', () => {
    expect(getFilesystemBrowserErrorMessage(new Error('Something went wrong'))).toBe(
      'Something went wrong'
    )
  })

  it('returns generic message for unknown error types', () => {
    expect(getFilesystemBrowserErrorMessage('unexpected string')).toBe(
      'Failed to load the filesystem browser'
    )
  })
})

describe('filesystemApi', () => {
  it('children returns browse response', async () => {
    const response = { path: '/', children: [], parent: null }
    server.use(api.get('/filesystem/children', () => HttpResponse.json(response)))

    await expect(filesystemApi.children()).resolves.toMatchObject({ path: '/' })
  })

  it('children wraps errors with human-readable message', async () => {
    server.use(
      api.get('/filesystem/children', () =>
        HttpResponse.json({ error: { message: 'Permission denied' } }, { status: 403 })
      )
    )

    await expect(filesystemApi.children()).rejects.toThrow('Permission denied')
  })
})
