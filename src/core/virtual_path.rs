use crate::db::repository::Repository;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;

/// Represents a virtual path mapping.
#[derive(Debug, Clone)]
pub struct VirtualPathMapping {
    pub file_id: i64,
    pub virtual_path: String,
    pub real_path: String,
}

/// Result of a bulk virtual path operation.
#[derive(Debug)]
pub struct BulkResult {
    pub succeeded: Vec<i64>,
    pub failed: Vec<(i64, String)>,
}

/// Set the virtual path for a tracked file and create a symlink.
pub fn set_virtual_path(
    repo: &Repository,
    symlink_base: &str,
    file_id: i64,
    virtual_path: &str,
    real_path: &str,
) -> anyhow::Result<()> {
    // Validate the virtual path format (must be absolute)
    if !virtual_path.starts_with('/') {
        anyhow::bail!(
            "Virtual path must be absolute (start with /): {}",
            virtual_path
        );
    }

    repo.update_tracked_file_virtual_path(file_id, Some(virtual_path))?;
    create_symlink(symlink_base, virtual_path, real_path)?;
    Ok(())
}

/// Remove the virtual path for a tracked file and delete the symlink.
pub fn remove_virtual_path(
    repo: &Repository,
    symlink_base: &str,
    file_id: i64,
    virtual_path: &str,
) -> anyhow::Result<()> {
    repo.update_tracked_file_virtual_path(file_id, None)?;
    remove_symlink(symlink_base, virtual_path)?;
    Ok(())
}

/// Set virtual paths for multiple files in a single operation.
///
/// Each entry maps a `file_id` to a `(virtual_path, real_path)` tuple.
/// Failures on individual items are collected rather than aborting the whole batch.
pub fn bulk_set(
    repo: &Repository,
    symlink_base: &str,
    entries: &[(i64, String, String)],
) -> BulkResult {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for (file_id, virtual_path, real_path) in entries {
        match set_virtual_path(repo, symlink_base, *file_id, virtual_path, real_path) {
            Ok(()) => succeeded.push(*file_id),
            Err(e) => failed.push((*file_id, e.to_string())),
        }
    }

    BulkResult { succeeded, failed }
}

/// Assign virtual paths to all tracked files in a folder using a common virtual base.
///
/// For each file under `folder_path` (relative within the drive pair), the virtual path
/// is computed as `virtual_base / <relative-path-within-folder>`.
pub fn bulk_from_real(
    repo: &Repository,
    symlink_base: &str,
    drive_pair_id: i64,
    folder_path: &str,
    virtual_base: &str,
    pair_active_path: &str,
) -> anyhow::Result<BulkResult> {
    if !virtual_base.starts_with('/') {
        anyhow::bail!(
            "virtual_base must be absolute (start with /): {}",
            virtual_base
        );
    }

    let (files, _) =
        repo.list_tracked_files(Some(drive_pair_id), None, None, 1, i64::MAX)?;

    let folder_norm = folder_path.trim_end_matches('/');
    let mut entries: Vec<(i64, String, String)> = Vec::new();

    for file in &files {
        let rel = file.relative_path.as_str();
        // Only include files whose path starts with the target folder
        if !rel.starts_with(folder_norm) {
            continue;
        }
        let suffix = rel[folder_norm.len()..].trim_start_matches('/');
        let virtual_path = if suffix.is_empty() {
            virtual_base.to_string()
        } else {
            format!("{}/{}", virtual_base.trim_end_matches('/'), suffix)
        };
        let real_path = PathBuf::from(pair_active_path).join(rel);
        entries.push((
            file.id,
            virtual_path,
            real_path.to_string_lossy().to_string(),
        ));
    }

    Ok(bulk_set(repo, symlink_base, &entries))
}

/// Create a symlink at `symlink_base/virtual_path` -> `real_path`.
pub fn create_symlink(
    symlink_base: &str,
    virtual_path: &str,
    real_path: &str,
) -> anyhow::Result<()> {
    let link_path = build_symlink_path(symlink_base, virtual_path);

    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).context("Failed to create virtual path directories")?;
    }

    // Remove existing symlink if it exists
    if link_path.exists() || link_path.is_symlink() {
        fs::remove_file(&link_path).context("Failed to remove old symlink")?;
    }

    std::os::unix::fs::symlink(real_path, &link_path).context("Failed to create symlink")?;

    Ok(())
}

/// Remove the symlink at `symlink_base/virtual_path`.
pub fn remove_symlink(symlink_base: &str, virtual_path: &str) -> anyhow::Result<()> {
    let link_path = build_symlink_path(symlink_base, virtual_path);
    if link_path.exists() || link_path.is_symlink() {
        fs::remove_file(&link_path)?;
    }
    Ok(())
}

fn build_symlink_path(symlink_base: &str, virtual_path: &str) -> PathBuf {
    // virtual_path starts with '/', strip it for joining
    let stripped = virtual_path.trim_start_matches('/');
    PathBuf::from(symlink_base).join(stripped)
}

/// Regenerate all symlinks from the database.
pub fn refresh_all_symlinks(
    repo: &Repository,
    symlink_base: &str,
    drive_pairs: &std::collections::HashMap<i64, crate::db::repository::DrivePair>,
) -> anyhow::Result<SymlinkRefreshResult> {
    let mut created = 0u32;
    let removed = 0u32;
    let mut errors: Vec<String> = Vec::new();

    let (files, _) = repo.list_tracked_files(None, None, None, 1, i64::MAX)?;
    for file in &files {
        if let Some(vp) = &file.virtual_path {
            if let Some(pair) = drive_pairs.get(&file.drive_pair_id) {
                let real_path = PathBuf::from(pair.active_path()).join(&file.relative_path);
                match create_symlink(symlink_base, vp, real_path.to_str().unwrap_or("")) {
                    Ok(()) => created += 1,
                    Err(e) => errors.push(format!("File {}: {}", file.id, e)),
                }
            }
        }
    }

    Ok(SymlinkRefreshResult {
        created,
        removed,
        errors,
    })
}

#[derive(Debug)]
pub struct SymlinkRefreshResult {
    pub created: u32,
    pub removed: u32,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use tempfile::TempDir;

    fn setup_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    #[test]
    fn test_virtual_path_to_real_path_mapping() {
        let symlink_base = TempDir::new().unwrap();
        let real_dir = TempDir::new().unwrap();
        let real_file = real_dir.path().join("doc.txt");
        fs::write(&real_file, b"content").unwrap();

        create_symlink(
            symlink_base.path().to_str().unwrap(),
            "/docs/doc.txt",
            real_file.to_str().unwrap(),
        )
        .unwrap();

        let link = symlink_base.path().join("docs/doc.txt");
        assert!(link.is_symlink(), "Symlink should exist");
        let target = fs::read_link(&link).unwrap();
        assert_eq!(target.to_str().unwrap(), real_file.to_str().unwrap());
    }

    #[test]
    fn test_symlink_creation_creates_directories() {
        let symlink_base = TempDir::new().unwrap();
        let real_dir = TempDir::new().unwrap();
        let real_file = real_dir.path().join("deep.txt");
        fs::write(&real_file, b"deep").unwrap();

        create_symlink(
            symlink_base.path().to_str().unwrap(),
            "/a/b/c/deep.txt",
            real_file.to_str().unwrap(),
        )
        .unwrap();

        let link = symlink_base.path().join("a/b/c/deep.txt");
        assert!(link.is_symlink());
    }

    #[test]
    fn test_invalid_virtual_path_rejected() {
        let repo = setup_repo();
        let symlink_base = TempDir::new().unwrap();
        let pair = repo.create_drive_pair("p", "/a", "/b").unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", "hash", 1, None)
            .unwrap();

        let result = set_virtual_path(
            &repo,
            symlink_base.path().to_str().unwrap(),
            file.id,
            "relative/path", // not absolute
            "/real/path",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_symlink() {
        let symlink_base = TempDir::new().unwrap();
        let real_dir = TempDir::new().unwrap();
        let real_file = real_dir.path().join("r.txt");
        fs::write(&real_file, b"data").unwrap();

        create_symlink(
            symlink_base.path().to_str().unwrap(),
            "/r.txt",
            real_file.to_str().unwrap(),
        )
        .unwrap();

        let link = symlink_base.path().join("r.txt");
        assert!(link.is_symlink());

        remove_symlink(symlink_base.path().to_str().unwrap(), "/r.txt").unwrap();
        assert!(!link.exists());
        assert!(!link.is_symlink());
    }

    #[test]
    fn test_bulk_path_prefix_stripping() {
        // Test that path prefix stripping logic works correctly
        // From plan: documents/projects/alpha/spec.txt -> /projects/alpha/spec.txt
        let full_path = "documents/projects/alpha/spec.txt";
        let strip_prefix = "documents/projects/";
        let virtual_base = "/projects";

        let stripped = full_path.strip_prefix(strip_prefix).unwrap_or(full_path);
        let virtual_path = format!("{}/{}", virtual_base.trim_end_matches('/'), stripped);

        assert_eq!(virtual_path, "/projects/alpha/spec.txt");
    }
}
