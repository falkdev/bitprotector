import { describe, expect, it } from 'vitest'
import { getFilesystemBrowserErrorMessage } from './filesystem'

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

  it('falls back to generic error messages', () => {
    expect(getFilesystemBrowserErrorMessage(new Error('Something went wrong'))).toBe(
      'Something went wrong'
    )
  })
})
