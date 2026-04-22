import { HttpResponse, http, type HttpHandler, type PathParams } from 'msw'

function apiUrl(path: string) {
  return `*/api/v1${path}`
}

type Resolver = Parameters<typeof http.get<PathParams>>[1]

function wrap(
  method: 'get' | 'post' | 'put' | 'delete',
  path: string,
  resolver: Resolver
): HttpHandler {
  return http[method](apiUrl(path), resolver)
}

export const api = {
  get(path: string, resolver: Resolver) {
    return wrap('get', path, resolver)
  },
  post(path: string, resolver: Resolver) {
    return wrap('post', path, resolver)
  },
  put(path: string, resolver: Resolver) {
    return wrap('put', path, resolver)
  },
  delete(path: string, resolver: Resolver) {
    return wrap('delete', path, resolver)
  },
}

export function apiError(status: number, message: string, code = 'TEST_ERROR') {
  return HttpResponse.json(
    {
      error: {
        code,
        message,
      },
    },
    { status }
  )
}
