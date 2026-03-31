use anyhow::Context;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTargetKind {
    File,
    Directory,
}

fn contains_parent_dir(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

pub fn resolve_path_within_drive_root(
    drive_root: &str,
    input_path: &str,
    target_kind: PathTargetKind,
) -> anyhow::Result<String> {
    let trimmed = input_path.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Path is required");
    }

    let raw_path = Path::new(trimmed);
    if contains_parent_dir(raw_path) {
        anyhow::bail!("Parent-directory traversal is not allowed");
    }

    let root = Path::new(drive_root)
        .canonicalize()
        .with_context(|| format!("Failed to access drive root: {}", drive_root))?;
    let candidate = if raw_path.is_absolute() {
        PathBuf::from(raw_path)
    } else {
        root.join(raw_path)
    };

    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("Path does not exist: {}", candidate.display()))?;
    let relative = canonical.strip_prefix(&root).with_context(|| {
        format!(
            "Selected path is outside the active drive root: {}",
            canonical.display()
        )
    })?;

    if relative.as_os_str().is_empty() {
        anyhow::bail!("Selecting the drive root itself is not supported");
    }

    match target_kind {
        PathTargetKind::File if !canonical.is_file() => {
            anyhow::bail!("Selected path is not a file: {}", canonical.display())
        }
        PathTargetKind::Directory if !canonical.is_dir() => {
            anyhow::bail!("Selected path is not a directory: {}", canonical.display())
        }
        _ => {}
    }

    relative
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Selected path is not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolves_relative_file_path() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("doc.txt"), b"hello").unwrap();

        let resolved = resolve_path_within_drive_root(
            root.path().to_str().unwrap(),
            "doc.txt",
            PathTargetKind::File,
        )
        .unwrap();

        assert_eq!(resolved, "doc.txt");
    }

    #[test]
    fn rejects_parent_directory_traversal() {
        let root = TempDir::new().unwrap();
        let err = resolve_path_within_drive_root(
            root.path().to_str().unwrap(),
            "../etc/passwd",
            PathTargetKind::File,
        )
        .unwrap_err();

        assert!(err.to_string().contains("traversal"));
    }

    #[test]
    fn rejects_symlink_escape() {
        let root = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            root.path().join("secret-link.txt"),
        )
        .unwrap();

        let err = resolve_path_within_drive_root(
            root.path().to_str().unwrap(),
            "secret-link.txt",
            PathTargetKind::File,
        )
        .unwrap_err();

        assert!(err.to_string().contains("outside the active drive root"));
    }
}
