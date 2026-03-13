use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Context;
use crate::db::repository::Repository;

/// Represents a virtual path mapping.
#[derive(Debug, Clone)]
pub struct VirtualPathMapping {
    pub file_id: i64,
    pub virtual_path: String,
    pub real_path: String,
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
        anyhow::bail!("Virtual path must be absolute (start with /): {}", virtual_path);
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

/// Create a symlink at `symlink_base/virtual_path` -> `real_path`.
pub fn create_symlink(symlink_base: &str, virtual_path: &str, real_path: &str) -> anyhow::Result<()> {
    let link_path = build_symlink_path(symlink_base, virtual_path);

    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent).context("Failed to create virtual path directories")?;
    }

    // Remove existing symlink if it exists
    if link_path.exists() || link_path.is_symlink() {
        fs::remove_file(&link_path).context("Failed to remove old symlink")?;
    }

    std::os::unix::fs::symlink(real_path, &link_path)
        .context("Failed to create symlink")?;

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
    let mut removed = 0u32;
    let mut errors: Vec<String> = Vec::new();

    let (files, _) = repo.list_tracked_files(None, None, None, 1, i64::MAX)?;
    for file in &files {
        if let Some(vp) = &file.virtual_path {
            if let Some(pair) = drive_pairs.get(&file.drive_pair_id) {
                let real_path = PathBuf::from(&pair.primary_path).join(&file.relative_path);
                match create_symlink(symlink_base, vp, real_path.to_str().unwrap_or("")) {
                    Ok(()) => created += 1,
                    Err(e) => errors.push(format!("File {}: {}", file.id, e)),
                }
            }
        }
    }

    Ok(SymlinkRefreshResult { created, removed, errors })
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
    use tempfile::TempDir;
    use crate::db::repository::{create_memory_pool, Repository, DrivePair};
    use crate::db::schema::initialize_schema;
    use std::io::Write;

    fn setup_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    fn make_pair_record(primary: &str, secondary: &str) -> DrivePair {
        DrivePair {
            id: 1,
            name: "test".to_string(),
            primary_path: primary.to_string(),
            secondary_path: secondary.to_string(),
            created_at: "".to_string(),
            updated_at: "".to_string(),
        }
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
        let file = repo.create_tracked_file(pair.id, "f.txt", "hash", 1, None).unwrap();

        let result = set_virtual_path(
            &repo,
            symlink_base.path().to_str().unwrap(),
            file.id,
            "relative/path",   // not absolute
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
