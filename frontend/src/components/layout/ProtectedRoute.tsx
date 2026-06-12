import { type ReactNode, useEffect, useState } from 'react'
import { Navigate } from 'react-router-dom'
import { useAuth } from '@/hooks/useAuth'
import { LoadingSpinner } from '@/components/shared/LoadingSpinner'

interface ProtectedRouteProps {
  children: ReactNode
}

export function ProtectedRoute({ children }: ProtectedRouteProps) {
  const { isAuthenticated, validate } = useAuth()
  const [checking, setChecking] = useState(true)
  const [valid, setValid] = useState(false)

  useEffect(() => {
    if (!isAuthenticated || !checking) {
      return
    }

    let active = true
    validate().then((ok) => {
      if (!active) return
      setValid(ok)
      setChecking(false)
    })

    return () => {
      active = false
    }
  }, [checking, isAuthenticated, validate])

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />
  }

  if (checking) {
    return (
      <div className="flex h-screen items-center justify-center">
        <LoadingSpinner />
      </div>
    )
  }

  if (!valid) {
    return <Navigate to="/login" replace />
  }

  return <>{children}</>
}
