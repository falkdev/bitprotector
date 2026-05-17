import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { FileActions } from './FileActions'
import { makeTrackedFile } from '@/test/factories'

describe('FileActions', () => {
  const file = makeTrackedFile({ id: 42 })

  it('renders the three action buttons', () => {
    render(
      <table>
        <tbody>
          <tr>
            <td>
              <FileActions
                file={file}
                onMirror={vi.fn()}
                onDelete={vi.fn()}
                onSetVirtualPath={vi.fn()}
              />
            </td>
          </tr>
        </tbody>
      </table>
    )
    expect(screen.getByTestId('file-actions-42')).toBeInTheDocument()
    expect(screen.getByTestId('action-set-virtual-path')).toBeInTheDocument()
    expect(screen.getByTestId('action-mirror')).toBeInTheDocument()
    expect(screen.getByTestId('action-delete')).toBeInTheDocument()
  })

  it('calls onSetVirtualPath with the file when that button is clicked', async () => {
    const onSetVirtualPath = vi.fn()
    const user = userEvent.setup()
    render(
      <table>
        <tbody>
          <tr>
            <td>
              <FileActions
                file={file}
                onMirror={vi.fn()}
                onDelete={vi.fn()}
                onSetVirtualPath={onSetVirtualPath}
              />
            </td>
          </tr>
        </tbody>
      </table>
    )
    await user.click(screen.getByTestId('action-set-virtual-path'))
    expect(onSetVirtualPath).toHaveBeenCalledWith(file)
  })

  it('calls onMirror with the file when mirror button is clicked', async () => {
    const onMirror = vi.fn()
    const user = userEvent.setup()
    render(
      <table>
        <tbody>
          <tr>
            <td>
              <FileActions
                file={file}
                onMirror={onMirror}
                onDelete={vi.fn()}
                onSetVirtualPath={vi.fn()}
              />
            </td>
          </tr>
        </tbody>
      </table>
    )
    await user.click(screen.getByTestId('action-mirror'))
    expect(onMirror).toHaveBeenCalledWith(file)
  })

  it('calls onDelete with the file when delete button is clicked', async () => {
    const onDelete = vi.fn()
    const user = userEvent.setup()
    render(
      <table>
        <tbody>
          <tr>
            <td>
              <FileActions
                file={file}
                onMirror={vi.fn()}
                onDelete={onDelete}
                onSetVirtualPath={vi.fn()}
              />
            </td>
          </tr>
        </tbody>
      </table>
    )
    await user.click(screen.getByTestId('action-delete'))
    expect(onDelete).toHaveBeenCalledWith(file)
  })
})
