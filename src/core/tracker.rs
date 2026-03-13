use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Context;
use crate::core::{checksum, mirror};
use crate::db::repository::{Repository, DrivePair, TrackedFolder, TrackedFile};
use crate::logging::event_logger;

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

    let _ = event_logger::log_file_tracked(repo, tracked.id, relative_path);
    Ok(tracked)
}

/// Register a folder so it can be auto-scanned for new files.
pub fn track_folder(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder_path: &str,
    auto_virtual_path: bool,
    default_virtual_base: Option<&str>,
) -> anyhow::Result<TrackedFolder> {
    let full_path = PathBuf::from(&drive_pair.primary_path).join(folder_path);
    if !full_path.is_dir() {
        anyhow::bail!("Folder does not exist on primary drive: {}", full_path.display());
    }
    repo.create_tracked_folder(drive_pair.id, folder_path, auto_virtual_path, default_virtual_base)
}

/// Compute the virtual path for a file being auto-tracked inside a folder.
/// Strips the folder prefix from the relative_path and prepends default_virtual_base.
fn compute_virtual_path(folder: &TrackedFolder, relative_path: &str) -> Option<String> {
    if !folder.auto_virtual_path {
        return None;
    }
    let base = folder.default_virtual_base.as_deref().unwrap_or("/virtual");
    let folder_prefix = format!("{}/", folder.folder_path.trim_end_matches('/'));
    let within_folder = relative_path.strip_prefix(&folder_prefix).unwrap_or(relative_path);
    Some(format!("{}/{}", base.trim_end_matches('/'), within_folder))
}

/// Scan a tracked folder on the primary drive and auto-track+mirror any untracked files.
/// Returns the list of newly tracked files.
pub fn auto_track_folder_files(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder: &TrackedFolder,
) -> anyhow::Result<Vec<TrackedFile>> {
    let folder_full_path = PathBuf::from(&drive_pair.primary_path).join(&folder.folder_path);
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
            .strip_prefix(&drive_pair.primary_path)
            .context("Path outside primary drive")?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Non-UTF8 path"))?
            .to_string();

        if existing.iter().any(|f| f.relative_path == relative_path) {
            continue;
        }

        let virtual_path = compute_virtual_path(folder, &relative_path);
        let file = track_file(repo, drive_pair, &relative_path, virtual_path.as_deref())?;
        mirror::mirror_file(drive_pair, &relative_path)?;
        repo.update_tracked_file_mirror_status(file.id, true)?;
        newly_tracked.push(repo.get_tracked_file(file.id)?);
    }

    Ok(newly_tracked)
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

    #[test]
    fn test_track_folder_registers_in_db() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("docs")).unwrap();
        let folder = track_folder(&repo, &pair, "docs", false, None).unwrap();
        assert_eq!(folder.folder_path, "docs");
        assert!(!folder.auto_virtual_path);
    }

    #[test]
    fn test_track_folder_nonexistent_fails() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let result = track_folder(&repo, &pair, "no_such_dir", false, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_auto_track_folder_files_tracks_new_files() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("photos")).unwrap();
        fs::write(primary.path().join("photos/img1.jpg"), b"img1").unwrap();
        fs::write(primary.path().join("photos/img2.jpg"), b"img2").unwrap();
        let folder = track_folder(&repo, &pair, "photos", false, None).unwrap();

        let tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(tracked.len(), 2, "Both files should be auto-tracked");
        for f in &tracked {
            assert!(f.is_mirrored, "Auto-tracked files should be mirrored");
        }
    }

    #[test]
    fn test_auto_track_applies_default_virtual_path() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("reports")).unwrap();
        fs::write(primary.path().join("reports/q1.pdf"), b"q1 content").unwrap();
        let folder = track_folder(&repo, &pair, "reports", true, Some("/virtual/reports")).unwrap();

        let tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(tracked.len(), 1);
        let vp = tracked[0].virtual_path.as_deref().unwrap();
        assert_eq!(vp, "/virtual/reports/q1.pdf");
    }

    #[test]
    fn test_auto_track_skips_already_tracked_files() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("data")).unwrap();
        fs::write(primary.path().join("data/file.csv"), b"data").unwrap();

        // Track manually first
        track_file(&repo, &pair, "data/file.csv", None).unwrap();

        let folder = track_folder(&repo, &pair, "data", false, None).unwrap();
        let newly_tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(newly_tracked.len(), 0, "Already-tracked file should be skipped");
    }
}
