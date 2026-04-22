import { useEffect, useRef, useState, useCallback } from 'react'

interface UsePollingOptions {
  interval?: number
  enabled?: boolean
  immediate?: boolean
}

export function usePolling<T>(fetchFn: () => Promise<T>, options: UsePollingOptions = {}) {
  const { interval = 5000, enabled = true, immediate = true } = options
  const [data, setData] = useState<T | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [paused, setPaused] = useState(!enabled)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const fetchFnRef = useRef(fetchFn)
  fetchFnRef.current = fetchFn

  const runFetch = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await fetchFnRef.current()
      setData(result)
    } catch (err) {
      setError(String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (paused) return
    if (immediate) {
      void runFetch()
    }
    timerRef.current = setInterval(() => void runFetch(), interval)
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [paused, interval, immediate, runFetch])

  const pause = useCallback(() => setPaused(true), [])
  const resume = useCallback(() => setPaused(false), [])
  const refresh = useCallback(() => void runFetch(), [runFetch])

  return { data, loading, error, paused, pause, resume, refresh }
}
