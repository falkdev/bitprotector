import type { ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { PathPickerDialog } from './PathPickerDialog'

const childrenMock = vi.fn()

vi.mock('@/api/filesystem', () => ({
  filesystemApi: {
    children: (...args: unknown[]) => childrenMock(...args),
  },
}))

vi.mock('react-arborist', async () => {
  const React = await import('react')

  const Tree = React.forwardRef(function MockTree(
    {
      data,
      onSelect,
      onToggle,
      selection,
      children: NodeRenderer,
    }: {
      data: Array<{
        name: string
        path: string
        kind: 'directory' | 'file'
        children?: unknown[]
      }>
      onSelect?: (
        nodes: Array<{ data: { name: string; path: string; kind: 'directory' | 'file' } }>
      ) => void
      onToggle?: (id: string) => void
      selection?: string
      children?: (props: {
        node: {
          id: string
          data: { name: string; path: string; kind: 'directory' | 'file' }
          isOpen: boolean
          isSelected: boolean
          handleClick: (...args: unknown[]) => void
          toggle: () => void
          open: () => void
          close: () => void
        }
        style: Record<string, never>
      }) => ReactNode
    },
    ref: React.ForwardedRef<{ open: (id: string) => void; isOpen: (id: string) => boolean }>
  ) {
    const [openPaths, setOpenPaths] = React.useState<Set<string>>(() => new Set(['/']))

    const open = React.useCallback(
      (id: string) => {
        setOpenPaths((current) => {
          if (current.has(id)) {
            return current
          }
          const next = new Set(current)
          next.add(id)
          return next
        })
        onToggle?.(id)
      },
      [onToggle]
    )

    const close = React.useCallback(
      (id: string) => {
        setOpenPaths((current) => {
          if (!current.has(id)) {
            return current
          }
          const next = new Set(current)
          next.delete(id)
          return next
        })
        onToggle?.(id)
      },
      [onToggle]
    )

    const toggle = React.useCallback(
      (id: string) => {
        if (openPaths.has(id)) {
          close(id)
        } else {
          open(id)
        }
      },
      [close, open, openPaths]
    )

    React.useImperativeHandle(
      ref,
      () => ({
        open,
        isOpen: (id: string) => openPaths.has(id),
      }),
      [open, openPaths]
    )

    const renderNodes = (
      nodes: Array<{
        name: string
        path: string
        kind: 'directory' | 'file'
        children?: unknown[]
      }>
    ) =>
      nodes.map((node) => {
        const isOpen = openPaths.has(node.path)

        return (
          <div key={node.path}>
            {NodeRenderer ? (
              <NodeRenderer
                node={{
                  id: node.path,
                  data: node,
                  isOpen,
                  isSelected: selection === node.path,
                  handleClick: () => onSelect?.([{ data: node }]),
                  toggle: () => toggle(node.path),
                  open: () => open(node.path),
                  close: () => close(node.path),
                }}
                style={{}}
              />
            ) : null}
            {isOpen && Array.isArray(node.children) && node.children.length > 0
              ? renderNodes(node.children as never[])
              : null}
          </div>
        )
      })

    return <div>{renderNodes(data)}</div>
  })

  return { Tree }
})

function makeEntry(path: string, kind: 'directory' | 'file') {
  const name = path === '/' ? '/' : (path.split('/').pop() ?? path)
  return {
    name,
    path,
    kind,
    is_hidden: false,
    is_selectable: true,
    has_children: kind === 'directory',
  }
}

describe('PathPickerDialog', () => {
  beforeEach(() => {
    childrenMock.mockReset()
  })

  it('loads the root directory when opened', async () => {
    childrenMock.mockResolvedValue({
      path: '/',
      canonical_path: '/',
      parent_path: null,
      entries: [makeEntry('/home', 'directory')],
    })

    render(
      <PathPickerDialog
        open
        title="Pick a path"
        mode="directory"
        value=""
        onClose={() => {}}
        onPick={() => {}}
      />
    )

    await waitFor(() =>
      expect(childrenMock).toHaveBeenCalledWith({
        path: '/',
        include_hidden: false,
        directories_only: true,
      })
    )
    expect(await screen.findByText('home')).toBeInTheDocument()
  })

  it('loads a constrained root when rootPath is provided', async () => {
    childrenMock.mockResolvedValue({
      path: '/mnt/primary',
      canonical_path: '/mnt/primary',
      parent_path: '/mnt',
      entries: [makeEntry('/mnt/primary/docs', 'directory')],
    })

    render(
      <PathPickerDialog
        open
        title="Pick a path"
        mode="directory"
        value=""
        rootPath="/mnt/primary"
        onClose={() => {}}
        onPick={() => {}}
      />
    )

    await waitFor(() =>
      expect(childrenMock).toHaveBeenCalledWith({
        path: '/mnt/primary',
        include_hidden: false,
        directories_only: true,
      })
    )
    expect(await screen.findByText('docs')).toBeInTheDocument()
  })

  it('loads children lazily when a directory is expanded', async () => {
    childrenMock
      .mockResolvedValueOnce({
        path: '/',
        canonical_path: '/',
        parent_path: null,
        entries: [makeEntry('/mnt', 'directory')],
      })
      .mockResolvedValueOnce({
        path: '/mnt',
        canonical_path: '/mnt',
        parent_path: '/',
        entries: [makeEntry('/mnt/archive', 'directory')],
      })

    const user = userEvent.setup()
    render(
      <PathPickerDialog
        open
        title="Pick a path"
        mode="directory"
        value=""
        onClose={() => {}}
        onPick={() => {}}
      />
    )

    await screen.findByText('mnt')
    await user.click(screen.getByTestId('path-picker-node-/mnt'))

    await waitFor(() =>
      expect(childrenMock).toHaveBeenLastCalledWith({
        path: '/mnt',
        include_hidden: false,
        directories_only: true,
      })
    )
    expect(await screen.findByText('archive')).toBeInTheDocument()
  })

  it('disables confirmation when a directory is selected in file mode', async () => {
    childrenMock.mockResolvedValue({
      path: '/',
      canonical_path: '/',
      parent_path: null,
      entries: [makeEntry('/docs', 'directory')],
    })

    const user = userEvent.setup()
    render(
      <PathPickerDialog
        open
        title="Pick a file"
        mode="file"
        value=""
        onClose={() => {}}
        onPick={() => {}}
      />
    )

    await screen.findByText('docs')
    await user.click(screen.getByTestId('path-picker-node-/docs'))

    expect(await screen.findByText('Select a file, not a folder')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Use Path' })).toBeDisabled()
  })

  it('shows a helpful message when the filesystem endpoint is unavailable', async () => {
    childrenMock.mockRejectedValue(
      new Error(
        'The running BitProtector API does not expose the filesystem browser endpoint yet. Rebuild and restart the backend, then refresh the page.'
      )
    )

    render(
      <PathPickerDialog
        open
        title="Pick a path"
        mode="directory"
        value=""
        onClose={() => {}}
        onPick={() => {}}
      />
    )

    expect(
      await screen.findByText(
        'The running BitProtector API does not expose the filesystem browser endpoint yet. Rebuild and restart the backend, then refresh the page.'
      )
    ).toBeInTheDocument()
    expect(
      screen.getByText('You can still type a path above and confirm it manually.')
    ).toBeInTheDocument()
  })
})
