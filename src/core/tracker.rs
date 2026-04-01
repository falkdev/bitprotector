use crate::core::{checksum, drive, mirror, virtual_path};
use crate::db::repository::{DrivePair, Repository, TrackedFile, TrackedFolder};
use crate::logging::event_logger;
use anyhow::Context;
use std::fs;
use std::path::PathBuf;

/// Track a new file: compute checksum, record in database, and mirror it.
pub fn track_file(
    repo: &Repository,
    drive_pair: &DrivePair,
    relative_path: &str,
    virtual_path: Option<&str>,
) -> anyhow::Result<crate::db::repository::TrackedFile> {
    drive::require_pair_mutation_allowed(drive_pair)?;
    drive::ensure_drive_root_marker(drive_pair.active_path())?;
    if drive::path_is_available(drive_pair.standby_path()) {
        let _ = drive::ensure_drive_root_marker(drive_pair.standby_path());
    }

    let master_path = PathBuf::from(drive_pair.active_path()).join(relative_path);

    if !master_path.exists() {
        anyhow::bail!(
            "File does not exist on active {} drive: {}",
            drive_pair.active_role,
            master_path.display()
        );
    }

    let file_checksum =
        checksum::checksum_file(&master_path).context("Failed to compute file checksum")?;
    let file_size = master_path.metadata()?.len() as i64;

    let tracked = repo.create_tracked_file(
        drive_pair.id,
        relative_path,
        &file_checksum,
        file_size,
        None,
    )?;

    if let Some(virtual_path) = virtual_path {
        if let Err(error) = virtual_path::set_virtual_path(repo, tracked.id, virtual_path) {
            let _ = repo.delete_tracked_file(tracked.id);
            return Err(error);
        }
    }

    let _ = event_logger::log_file_tracked(repo, tracked.id, relative_path);
    repo.get_tracked_file(tracked.id)
}

/// Register a folder so it can be auto-scanned for new files.
pub fn track_folder(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder_path: &str,
    virtual_path: Option<&str>,
) -> anyhow::Result<TrackedFolder> {
    drive::require_pair_mutation_allowed(drive_pair)?;
    drive::ensure_drive_root_marker(drive_pair.active_path())?;

    let full_path = PathBuf::from(drive_pair.active_path()).join(folder_path);
    if !full_path.is_dir() {
        anyhow::bail!(
            "Folder does not exist on active drive: {}",
            full_path.display()
        );
    }
    let tracked = repo.create_tracked_folder(drive_pair.id, folder_path, None)?;

    if let Some(virtual_path) = virtual_path {
        if let Err(error) = virtual_path::set_folder_virtual_path(repo, tracked.id, virtual_path) {
            let _ = repo.delete_tracked_folder(tracked.id);
            return Err(error);
        }
    }

    repo.get_tracked_folder(tracked.id)
}

/// Scan a tracked folder on the primary drive and auto-track+mirror any untracked files.
/// Returns the list of newly tracked files.
pub fn auto_track_folder_files(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder: &TrackedFolder,
) -> anyhow::Result<Vec<TrackedFile>> {
    drive::require_pair_mutation_allowed(drive_pair)?;

    let folder_full_path = PathBuf::from(drive_pair.active_path()).join(&folder.folder_path);
    let (existing, _) = repo.list_tracked_files(Some(drive_pair.id), None, None, 1, i64::MAX)?;

    let mut newly_tracked = Vec::new();

    for entry in fs::read_dir(&folder_full_path)
        .with_context(|| format!("Cannot read folder: {}", folder_full_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(drive_pair.active_path())
            .context("Path outside active drive")?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Non-UTF8 path"))?
            .to_string();

        if existing.iter().any(|f| f.relative_path == relative_path) {
            continue;
        }

        let file = track_file(repo, drive_pair, &relative_path, None)?;
        if drive_pair.standby_accepts_sync() {
            mirror::mirror_file(drive_pair, &relative_path)?;
            repo.update_tracked_file_mirror_status(file.id, true)?;
        } else {
            repo.update_tracked_file_mirror_status(file.id, false)?;
        }
        newly_tracked.push(repo.get_tracked_file(file.id)?);
    }

    Ok(newly_tracked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::create_memory_pool;
    use crate::db::schema::initialize_schema;
    use tempfile::TempDir;

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
        repo.create_drive_pair(
            "test",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap()
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
        let publish_root = TempDir::new().unwrap();
        let publish_path = publish_root.path().join("virtual/doc.txt");

        fs::write(primary.path().join("doc.txt"), b"content").unwrap();
        let tracked = track_file(
            &repo,
            &pair,
            "doc.txt",
            Some(publish_path.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            tracked.virtual_path,
            Some(publish_path.to_string_lossy().to_string())
        );
        assert!(publish_path.is_symlink());
    }

    #[test]
    fn test_track_nonexistent_file_fails() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let result = track_file(&repo, &pair, "missing.txt", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_track_folder_registers_in_db() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("docs")).unwrap();
        let folder = track_folder(&repo, &pair, "docs", None).unwrap();
        assert_eq!(folder.folder_path, "docs");
        assert!(folder.virtual_path.is_none());
    }

    #[test]
    fn test_track_folder_nonexistent_fails() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let result = track_folder(&repo, &pair, "no_such_dir", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_track_folder_files_tracks_new_files() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("photos")).unwrap();
        fs::write(primary.path().join("photos/img1.jpg"), b"img1").unwrap();
        fs::write(primary.path().join("photos/img2.jpg"), b"img2").unwrap();
        let folder = track_folder(&repo, &pair, "photos", None).unwrap();

        let tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(tracked.len(), 2, "Both files should be auto-tracked");
        for f in &tracked {
            assert!(f.is_mirrored, "Auto-tracked files should be mirrored");
        }
    }

    #[test]
    fn test_track_folder_with_virtual_path() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("reports")).unwrap();
        let publish_root = TempDir::new().unwrap();
        let publish_path = publish_root.path().join("virtual/reports");
        let folder = track_folder(&repo, &pair, "reports", Some(publish_path.to_str().unwrap())).unwrap();

        assert_eq!(
            folder.virtual_path,
            Some(publish_path.to_string_lossy().to_string())
        );
        assert!(publish_path.is_symlink());
    }

    #[test]
    fn test_auto_track_skips_already_tracked_files() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("data")).unwrap();
        fs::write(primary.path().join("data/file.csv"), b"data").unwrap();

        // Track manually first
        track_file(&repo, &pair, "data/file.csv", None).unwrap();

        let folder = track_folder(&repo, &pair, "data", None).unwrap();
        let newly_tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(
            newly_tracked.len(),
            0,
            "Already-tracked file should be skipped"
        );
    }
}
