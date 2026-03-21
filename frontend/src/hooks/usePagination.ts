import { useState, useCallback } from 'react'

export function usePagination(initialPage = 1, initialPerPage = 50) {
  const [page, setPage] = useState(initialPage)
  const [perPage] = useState(initialPerPage)

  const nextPage = useCallback(() => setPage((p) => p + 1), [])
  const prevPage = useCallback(() => setPage((p) => Math.max(1, p - 1)), [])
  const goToPage = useCallback((p: number) => setPage(Math.max(1, p)), [])
  const reset = useCallback(() => setPage(1), [])

  return { page, perPage, nextPage, prevPage, goToPage, reset }
}
