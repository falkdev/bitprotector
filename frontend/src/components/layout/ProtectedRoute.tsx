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
    if (!isAuthenticated) {
      setChecking(false)
      return
    }
    setChecking(true)
    validate().then((ok) => {
      setValid(ok)
      setChecking(false)
    })
  }, [isAuthenticated, validate])

  if (checking) {
    return (
      <div className="flex h-screen items-center justify-center">
        <LoadingSpinner />
      </div>
    )
  }

  if (!isAuthenticated || !valid) {
    return <Navigate to="/login" replace />
  }

  return <>{children}</>
}
