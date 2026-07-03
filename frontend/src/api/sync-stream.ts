import { useAuthStore } from '@/stores/auth-store'
import { authNavigation } from './client'
import type { SyncSummary } from '@/types/sync'

const baseURL = import.meta.env.VITE_API_BASE_URL ?? '/api/v1'

export type SyncStreamHandler = (summary: SyncSummary) => void

/**
 * Open an SSE connection to `GET /sync/queue/stream`.
 *
 * - Calls `onSummary` for every received `data:` event.
 * - Automatically reconnects with exponential back-off (max 30 s) after
 *   network errors or dropped streams.
 * - Cleans up on 401 by logging out and redirecting, the same way the
 *   axios interceptor does.
 * - Returns an `abort()` function; call it on component unmount to stop
 *   reconnecting and close the stream.
 */
export function openSyncStream(onSummary: SyncStreamHandler): { abort: () => void } {
  let aborted = false
  let retryDelay = 1000
  const maxDelay = 30_000

  const connect = async () => {
    if (aborted) return

    const token = useAuthStore.getState().token
    let response: Response

    try {
      response = await fetch(`${baseURL}/sync/queue/stream`, {
        headers: {
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
          Accept: 'text/event-stream',
        },
        signal: aborted ? AbortSignal.abort() : undefined,
      })
    } catch {
      if (aborted) return
      scheduleReconnect()
      return
    }

    if (response.status === 401) {
      useAuthStore.getState().logout()
      authNavigation.redirectToLogin()
      return
    }

    if (!response.ok || !response.body) {
      scheduleReconnect()
      return
    }

    // Successful connection — reset back-off.
    retryDelay = 1000

    const reader = response.body.getReader()
    const decoder = new TextDecoder()
    let buffer = ''

    try {
      while (!aborted) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        // Keep the last (potentially incomplete) line in the buffer.
        buffer = lines.pop() ?? ''

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const json = line.slice(6).trim()
            if (!json) continue
            try {
              const summary = JSON.parse(json) as SyncSummary
              onSummary(summary)
            } catch {
              // Malformed JSON — skip.
            }
          }
        }
      }
    } catch {
      // Stream read error.
    } finally {
      reader.cancel().catch(() => {})
    }

    if (!aborted) {
      scheduleReconnect()
    }
  }

  const scheduleReconnect = () => {
    if (aborted) return
    setTimeout(() => {
      void connect()
    }, retryDelay)
    retryDelay = Math.min(retryDelay * 2, maxDelay)
  }

  void connect()

  return {
    abort() {
      aborted = true
    },
  }
}
