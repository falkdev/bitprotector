use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Context;
use crate::core::checksum;
use crate::db::repository::{Repository, DrivePair};

/// Track a new file: compute checksum, record in database, and mirror it.
pub fn track_file(
    repo: &Repository,
    drive_pair: &DrivePair,
    relative_path: &str,
    virtual_path: Option<&str>,
) -> anyhow::Result<crate::db::repository::TrackedFile> {
    let master_path = PathBuf::from(&drive_pair.primary_path).join(relative_path);

    if !master_path.exists() {
        anyhow::bail!("File does not exist on primary drive: {}", master_path.display());
    }

    let file_checksum = checksum::checksum_file(&master_path)
        .context("Failed to compute file checksum")?;
    let file_size = master_path.metadata()?.len() as i64;

    let tracked = repo.create_tracked_file(
        drive_pair.id,
        relative_path,
        &file_checksum,
        file_size,
        virtual_path,
    )?;

    Ok(tracked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::db::repository::create_memory_pool;
    use crate::db::schema::initialize_schema;

    fn setup() -> (TempDir, TempDir, Repository) {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        let repo = Repository::new(pool);
        (primary, secondary, repo)
    }

    fn make_pair(primary: &TempDir, secondary: &TempDir, repo: &Repository) -> DrivePair {
        repo.create_drive_pair("test", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap()
    }

    #[test]
    fn test_track_file_records_correct_metadata() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);

        let content = b"hello bitprotector";
        fs::write(primary.path().join("test.txt"), content).unwrap();

        let tracked = track_file(&repo, &pair, "test.txt", None).unwrap();

        assert_eq!(tracked.relative_path, "test.txt");
        assert_eq!(tracked.checksum, checksum::checksum_bytes(content));
        assert_eq!(tracked.file_size, content.len() as i64);
        assert!(!tracked.is_mirrored);
    }

    #[test]
    fn test_track_file_with_virtual_path() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);

        fs::write(primary.path().join("doc.txt"), b"content").unwrap();
        let tracked = track_file(&repo, &pair, "doc.txt", Some("/virtual/doc.txt")).unwrap();
        assert_eq!(tracked.virtual_path, Some("/virtual/doc.txt".to_string()));
    }

    #[test]
    fn test_track_nonexistent_file_fails() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let result = track_file(&repo, &pair, "missing.txt", None);
        assert!(result.is_err());
    }
}
