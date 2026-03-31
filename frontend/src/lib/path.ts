export interface TrackedPathResolution {
  relativePath: string | null
  absolutePath: string | null
  error: string | null
}

export function isAbsoluteFilesystemPath(path: string): boolean {
  return path.trim().startsWith('/')
}

export function hasParentDirSegment(path: string): boolean {
  return path
    .split('/')
    .map((segment) => segment.trim())
    .some((segment) => segment === '..')
}

function normalizeSegments(path: string): string[] {
  return path
    .split('/')
    .map((segment) => segment.trim())
    .filter((segment) => segment.length > 0 && segment !== '.')
}

export function normalizeAbsoluteFilesystemPath(path: string): string {
  const segments = normalizeSegments(path)
  return segments.length === 0 ? '/' : `/${segments.join('/')}`
}

export function normalizeRelativeFilesystemPath(path: string): string {
  return normalizeSegments(path).join('/')
}

export function joinAbsoluteFilesystemPath(root: string, relativePath: string): string {
  const normalizedRoot = normalizeAbsoluteFilesystemPath(root)
  const normalizedRelative = normalizeRelativeFilesystemPath(relativePath)

  if (!normalizedRelative) {
    return normalizedRoot
  }

  return normalizedRoot === '/'
    ? `/${normalizedRelative}`
    : `${normalizedRoot}/${normalizedRelative}`
}

export function resolveTrackedPathInput(
  activeRoot: string | null | undefined,
  inputPath: string
): TrackedPathResolution {
  const trimmed = inputPath.trim()
  if (!trimmed) {
    return {
      relativePath: null,
      absolutePath: null,
      error: 'Path is required',
    }
  }

  if (hasParentDirSegment(trimmed)) {
    return {
      relativePath: null,
      absolutePath: null,
      error: 'Parent-directory traversal is not allowed',
    }
  }

  if (!activeRoot) {
    return {
      relativePath: null,
      absolutePath: null,
      error: 'Select a drive pair first',
    }
  }

  const normalizedRoot = normalizeAbsoluteFilesystemPath(activeRoot)
  const absolutePath = isAbsoluteFilesystemPath(trimmed)
    ? normalizeAbsoluteFilesystemPath(trimmed)
    : joinAbsoluteFilesystemPath(normalizedRoot, trimmed)
  const prefix = normalizedRoot === '/' ? '/' : `${normalizedRoot}/`

  if (absolutePath === normalizedRoot) {
    return {
      relativePath: null,
      absolutePath,
      error: 'Select a path inside the active drive root',
    }
  }

  if (!absolutePath.startsWith(prefix)) {
    return {
      relativePath: null,
      absolutePath,
      error: 'Selected path must be inside the active drive root',
    }
  }

  const relativePath = normalizedRoot === '/'
    ? absolutePath.slice(1)
    : absolutePath.slice(prefix.length)

  if (!relativePath) {
    return {
      relativePath: null,
      absolutePath,
      error: 'Select a path inside the active drive root',
    }
  }

  return {
    relativePath,
    absolutePath,
    error: null,
  }
}

export function resolveAbsolutePathForPicker(
  activeRoot: string | null | undefined,
  currentValue: string
): string {
  const trimmed = currentValue.trim()

  if (!trimmed) {
    return activeRoot ? normalizeAbsoluteFilesystemPath(activeRoot) : '/'
  }

  if (hasParentDirSegment(trimmed)) {
    return activeRoot ? normalizeAbsoluteFilesystemPath(activeRoot) : '/'
  }

  if (isAbsoluteFilesystemPath(trimmed)) {
    return normalizeAbsoluteFilesystemPath(trimmed)
  }

  if (activeRoot) {
    return joinAbsoluteFilesystemPath(activeRoot, trimmed)
  }

  return '/'
}

export function getActiveDrivePath(primaryPath: string, secondaryPath: string, activeRole: 'primary' | 'secondary'): string {
  return activeRole === 'primary' ? primaryPath : secondaryPath
}
