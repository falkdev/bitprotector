import { Component, type ReactNode } from 'react'
import { AlertTriangle } from 'lucide-react'

interface Props {
  children: ReactNode
  fallback?: ReactNode
}

interface State {
  hasError: boolean
  error: Error | null
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, error: null }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  override render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback
      return (
        <div className="flex flex-col items-center justify-center py-16 text-center">
          <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
          <h3 className="text-base font-semibold">Something went wrong</h3>
          <p className="mt-1 text-sm text-muted-foreground max-w-xs">
            {this.state.error?.message ?? 'An unexpected error occurred.'}
          </p>
          <button
            className="mt-4 rounded-md bg-primary px-4 py-2 text-sm text-white hover:bg-primary/90 transition-colors"
            onClick={() => this.setState({ hasError: false, error: null })}
          >
            Try again
          </button>
        </div>
      )
    }
    return this.props.children
  }
}
