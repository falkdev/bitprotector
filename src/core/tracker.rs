use crate::core::{checksum, drive, sync_queue, virtual_path};
use crate::db::repository::{DrivePair, Repository, TrackedFile, TrackedFolder};
use crate::logging::event_logger;
use anyhow::Context;
use rusqlite::Error as SqlError;
use std::fs;
use std::path::{Path, PathBuf};

/// Ensure the file at `path` has world-read access (and world-execute if the
/// owner can execute). This is required so that any service on the system can
/// read tracked files through virtual-path symlinks.
///
/// Requires `CAP_FOWNER` when the file is not owned by the calling process.
#[cfg(unix)]
fn ensure_world_readable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode();
    let normalized = mode | 0o004 | if mode & 0o100 != 0 { 0o001 } else { 0 };
    if normalized != mode {
        fs::set_permissions(path, fs::Permissions::from_mode(normalized))
            .with_context(|| format!("Failed to normalize permissions on {}", path.display()))?;
    }
    Ok(())
}

fn create_tracked_file_from_disk(
    repo: &Repository,
    drive_pair: &DrivePair,
    relative_path: &str,
    tracked_direct: bool,
    tracked_via_folder: bool,
) -> anyhow::Result<TrackedFile> {
    let master_path = PathBuf::from(drive_pair.active_path()).join(relative_path);

    if !master_path.exists() {
        anyhow::bail!(
            "File does not exist on active {} drive: {}",
            drive_pair.active_role,
            master_path.display()
        );
    }

    #[cfg(unix)]
    ensure_world_readable(&master_path)?;

    let file_checksum =
        checksum::checksum_file(&master_path, checksum::ChecksumStrategy::Streaming)
            .context("Failed to compute file checksum")?;
    let file_size = master_path.metadata()?.len() as i64;

    repo.create_tracked_file_with_source(
        drive_pair.id,
        relative_path,
        &file_checksum,
        file_size,
        None,
        tracked_direct,
        tracked_via_folder,
    )
}

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

    if let Ok(existing) = repo.get_tracked_file_by_path(drive_pair.id, relative_path) {
        #[cfg(unix)]
        {
            let file_path = PathBuf::from(drive_pair.active_path()).join(relative_path);
            ensure_world_readable(&file_path)?;
        }
        repo.update_tracked_file_sources(existing.id, true, false)?;
        if let Some(virtual_path) = virtual_path {
            virtual_path::set_virtual_path(repo, existing.id, virtual_path)?;
        }
        return repo.get_tracked_file(existing.id);
    }

    let tracked = create_tracked_file_from_disk(repo, drive_pair, relative_path, true, false)?;

    if let Some(virtual_path) = virtual_path {
        if let Err(error) = virtual_path::set_virtual_path(repo, tracked.id, virtual_path) {
            let _ = repo.delete_tracked_file(tracked.id);
            return Err(error);
        }
    }

    let _ = event_logger::log_file_tracked(
        repo,
        tracked.id,
        &format!("{}/{}", drive_pair.primary_path, relative_path),
    );
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

    repo.recompute_folder_provenance_for_drive(drive_pair.id)?;
    let result = repo.get_tracked_folder(tracked.id)?;
    let full_path = format!("{}/{}", drive_pair.primary_path, folder_path);
    let _ = event_logger::log_folder_tracked(repo, result.id, &full_path, drive_pair.id);
    Ok(result)
}

/// Scan a tracked folder and auto-track any untracked files.
/// New files are queued for mirroring when the standby side can accept sync.
/// Returns the list of newly tracked files.
pub fn auto_track_folder_files(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder: &TrackedFolder,
) -> anyhow::Result<Vec<TrackedFile>> {
    drive::require_pair_mutation_allowed(drive_pair)?;

    let folder_full_path = PathBuf::from(drive_pair.active_path()).join(&folder.folder_path);
    let mut newly_tracked = Vec::new();

    // Traverse recursively using an explicit stack so we don't need walkdir.
    let mut dirs_to_visit: Vec<PathBuf> = vec![folder_full_path];

    while let Some(dir) = dirs_to_visit.pop() {
        for entry in
            fs::read_dir(&dir).with_context(|| format!("Cannot read folder: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }

            let relative_path = path
                .strip_prefix(drive_pair.active_path())
                .context("Path outside active drive")?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Non-UTF8 path"))?
                .to_string();

            if repo
                .get_tracked_file_by_path(drive_pair.id, &relative_path)
                .is_ok()
            {
                continue;
            }

            let file =
                create_tracked_file_from_disk(repo, drive_pair, &relative_path, false, true)?;
            let full_path = format!("{}/{}", drive_pair.primary_path, relative_path);
            let _ = event_logger::log_file_tracked(repo, file.id, &full_path);
            if drive_pair.standby_accepts_sync() {
                let _ = sync_queue::create_for_new_tracking(repo, file.id)?;
            } else {
                repo.update_tracked_file_mirror_status(file.id, false)?;
            }
            newly_tracked.push(repo.get_tracked_file(file.id)?);
        }
    }

    repo.recompute_folder_provenance_for_drive(drive_pair.id)?;
    repo.mark_tracked_folder_scanned(folder.id)?;
    Ok(newly_tracked)
}

pub struct FolderScanSummary {
    pub new_files: usize,
    pub changed_files: usize,
}

fn relative_path_for_entry(path: &Path, drive_pair: &DrivePair) -> anyhow::Result<String> {
    path.strip_prefix(drive_pair.active_path())?
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Non-UTF8 path"))
}

pub fn count_folder_files(drive_pair: &DrivePair, folder: &TrackedFolder) -> anyhow::Result<i64> {
    let folder_full_path = PathBuf::from(drive_pair.active_path()).join(&folder.folder_path);
    let mut count = 0i64;
    let mut dirs_to_visit = vec![folder_full_path];

    while let Some(dir) = dirs_to_visit.pop() {
        for entry in
            fs::read_dir(&dir).with_context(|| format!("Cannot read folder: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
                continue;
            }
            if path.is_file() {
                count += 1;
            }
        }
    }

    Ok(count)
}

pub fn scan_tracked_folder<F>(
    repo: &Repository,
    drive_pair: &DrivePair,
    folder: &TrackedFolder,
    mut on_progress: F,
) -> anyhow::Result<FolderScanSummary>
where
    F: FnMut(i64, i64) -> anyhow::Result<()>,
{
    drive::require_pair_mutation_allowed(drive_pair)?;

    let folder_full_path = PathBuf::from(drive_pair.active_path()).join(&folder.folder_path);
    let total_files = count_folder_files(drive_pair, folder)?;
    let mut scanned_files = 0i64;
    let mut new_files = 0usize;
    let mut changed_files = 0usize;
    let mut dirs_to_visit = vec![folder_full_path];

    while let Some(dir) = dirs_to_visit.pop() {
        for entry in
            fs::read_dir(&dir).with_context(|| format!("Cannot read folder: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs_to_visit.push(path);
                continue;
            }
            if !path.is_file() {
                continue;
            }

            let relative_path = relative_path_for_entry(&path, drive_pair)?;

            match repo.get_tracked_file_by_path(drive_pair.id, &relative_path) {
                Ok(existing) => {
                    let current_hash =
                        checksum::checksum_file(&path, checksum::ChecksumStrategy::Streaming)?;
                    if current_hash != existing.checksum {
                        let new_size = path.metadata()?.len() as i64;
                        repo.update_tracked_file_checksum(existing.id, &current_hash, new_size)?;
                        repo.update_tracked_file_mirror_status(existing.id, false)?;
                        let _ = event_logger::log_event(
                            repo,
                            "change_detected",
                            Some(existing.id),
                            &format!(
                                "Change detected on active {} drive: {}/{}",
                                drive_pair.active_role, drive_pair.primary_path, relative_path
                            ),
                            Some(&current_hash),
                        );
                        if drive_pair.standby_accepts_sync() {
                            let _ = sync_queue::create_from_change(repo, existing.id)?;
                        }
                        changed_files += 1;
                    }
                }
                Err(error) => match error.downcast_ref::<SqlError>() {
                    Some(SqlError::QueryReturnedNoRows) => {
                        let file = create_tracked_file_from_disk(
                            repo,
                            drive_pair,
                            &relative_path,
                            false,
                            true,
                        )?;
                        let full_path = format!("{}/{}", drive_pair.primary_path, relative_path);
                        let _ = event_logger::log_file_tracked(repo, file.id, &full_path);
                        if drive_pair.standby_accepts_sync() {
                            let _ = sync_queue::create_for_new_tracking(repo, file.id)?;
                        } else {
                            repo.update_tracked_file_mirror_status(file.id, false)?;
                        }
                        new_files += 1;
                    }
                    _ => return Err(error),
                },
            }

            scanned_files += 1;
            on_progress(scanned_files, total_files)?;
        }
    }

    repo.recompute_folder_provenance_for_drive(drive_pair.id)?;
    repo.mark_tracked_folder_scanned(folder.id)?;

    Ok(FolderScanSummary {
        new_files,
        changed_files,
    })
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
        let virtual_root = TempDir::new().unwrap();
        let virtual_path_on_disk = virtual_root.path().join("virtual/doc.txt");

        fs::write(primary.path().join("doc.txt"), b"content").unwrap();
        let tracked = track_file(
            &repo,
            &pair,
            "doc.txt",
            Some(virtual_path_on_disk.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            tracked.virtual_path,
            Some(virtual_path_on_disk.to_string_lossy().to_string())
        );
        assert!(virtual_path_on_disk.is_symlink());
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
        let updated_folder = repo.get_tracked_folder(folder.id).unwrap();
        assert!(
            updated_folder.last_scanned_at.is_some(),
            "Successful scan should stamp folder scan history"
        );
        for f in &tracked {
            assert!(
                !f.is_mirrored,
                "Auto-tracked files should be queued, not mirrored"
            );
            let (queue_items, total) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
            assert!(total >= 1, "Auto-tracked files should enqueue mirror work");
            assert!(queue_items.iter().any(|item| item.tracked_file_id == f.id));
        }
    }

    #[test]
    fn test_track_folder_with_virtual_path() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("reports")).unwrap();
        let virtual_root = TempDir::new().unwrap();
        let virtual_path_on_disk = virtual_root.path().join("virtual/reports");
        let folder = track_folder(
            &repo,
            &pair,
            "reports",
            Some(virtual_path_on_disk.to_str().unwrap()),
        )
        .unwrap();

        assert_eq!(
            folder.virtual_path,
            Some(virtual_path_on_disk.to_string_lossy().to_string())
        );
        assert!(virtual_path_on_disk.is_symlink());
    }

    #[test]
    fn test_scan_tracked_folder_records_new_and_changed_files() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("docs")).unwrap();
        fs::write(primary.path().join("docs/existing.txt"), b"before").unwrap();
        let existing = track_file(&repo, &pair, "docs/existing.txt", None).unwrap();
        fs::write(primary.path().join("docs/existing.txt"), b"after").unwrap();
        fs::write(primary.path().join("docs/new.txt"), b"new").unwrap();
        let folder = track_folder(&repo, &pair, "docs", None).unwrap();

        let mut progress_updates = Vec::new();
        let summary = scan_tracked_folder(&repo, &pair, &folder, |scanned, total| {
            progress_updates.push((scanned, total));
            repo.update_scan_progress(folder.id, scanned, total)
        })
        .unwrap();

        assert_eq!(summary.new_files, 1);
        assert_eq!(summary.changed_files, 1);
        assert_eq!(progress_updates.last().copied(), Some((2, 2)));

        let updated_existing = repo.get_tracked_file(existing.id).unwrap();
        assert_eq!(
            updated_existing.checksum,
            checksum::checksum_bytes(b"after")
        );
        assert!(!updated_existing.is_mirrored);

        let scanned_folder = repo.get_tracked_folder(folder.id).unwrap();
        assert_eq!(scanned_folder.scan_scanned_files, 2);
        assert_eq!(scanned_folder.scan_total_files, 2);
        assert!(scanned_folder.last_scanned_at.is_some());
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

    #[test]
    fn test_auto_track_folder_files_recurses_into_subdirs() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);

        // Build: photos/flat.jpg, photos/nested/deep.jpg, photos/a/b/c/verydeep.jpg
        fs::create_dir_all(primary.path().join("photos/nested")).unwrap();
        fs::create_dir_all(primary.path().join("photos/a/b/c")).unwrap();
        fs::write(primary.path().join("photos/flat.jpg"), b"flat").unwrap();
        fs::write(primary.path().join("photos/nested/deep.jpg"), b"deep").unwrap();
        fs::write(
            primary.path().join("photos/a/b/c/verydeep.jpg"),
            b"verydeep",
        )
        .unwrap();

        let folder = track_folder(&repo, &pair, "photos", None).unwrap();
        let tracked = auto_track_folder_files(&repo, &pair, &folder).unwrap();
        assert_eq!(
            tracked.len(),
            3,
            "Recursive scan must find files in nested subdirectories"
        );

        let relative_paths: Vec<_> = tracked.iter().map(|f| f.relative_path.as_str()).collect();
        assert!(
            relative_paths.contains(&"photos/flat.jpg"),
            "flat.jpg must be tracked"
        );
        assert!(
            relative_paths.contains(&"photos/nested/deep.jpg"),
            "nested/deep.jpg must be tracked"
        );
        assert!(
            relative_paths.contains(&"photos/a/b/c/verydeep.jpg"),
            "a/b/c/verydeep.jpg must be tracked"
        );
    }

    #[test]
    fn test_auto_track_folder_queues_adopt_mirror() {
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("photos")).unwrap();
        fs::write(primary.path().join("photos/img.jpg"), b"photo content").unwrap();

        let folder = track_folder(&repo, &pair, "photos", None).unwrap();
        auto_track_folder_files(&repo, &pair, &folder).unwrap();

        let (queue, total) = repo.list_sync_queue(Some("pending"), 1, 20).unwrap();
        assert_eq!(total, 1);
        assert_eq!(
            queue[0].action, "adopt_mirror",
            "Folder scan should enqueue adopt_mirror, not mirror"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_track_file_normalizes_permissions_rw_group() {
        use std::os::unix::fs::PermissionsExt;
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let file = primary.path().join("data.mkv");
        fs::write(&file, b"content").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o660)).unwrap();

        track_file(&repo, &pair, "data.mkv", None).unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o664, "0o660 should become 0o664 (world-read added)");
    }

    #[cfg(unix)]
    #[test]
    fn test_track_file_normalizes_permissions_rwx_group() {
        use std::os::unix::fs::PermissionsExt;
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let file = primary.path().join("script.sh");
        fs::write(&file, b"#!/bin/sh").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o770)).unwrap();

        track_file(&repo, &pair, "script.sh", None).unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o775,
            "0o770 should become 0o775 (world-read+execute added)"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_track_file_does_not_add_world_execute_for_non_executable() {
        use std::os::unix::fs::PermissionsExt;
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let file = primary.path().join("notes.txt");
        fs::write(&file, b"notes").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o660)).unwrap();

        track_file(&repo, &pair, "notes.txt", None).unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o664,
            "Non-executable file must not gain world-execute"
        );
        assert_eq!(mode & 0o001, 0, "World-execute bit must not be set");
    }

    #[cfg(unix)]
    #[test]
    fn test_track_file_already_world_readable_unchanged() {
        use std::os::unix::fs::PermissionsExt;
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        let file = primary.path().join("pub.txt");
        fs::write(&file, b"public").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        track_file(&repo, &pair, "pub.txt", None).unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o644,
            "Already world-readable file should remain unchanged"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_auto_track_folder_normalizes_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (primary, secondary, repo) = setup();
        let pair = make_pair(&primary, &secondary, &repo);
        fs::create_dir(primary.path().join("media")).unwrap();
        let file = primary.path().join("media/movie.mkv");
        fs::write(&file, b"data").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o660)).unwrap();

        let folder = track_folder(&repo, &pair, "media", None).unwrap();
        auto_track_folder_files(&repo, &pair, &folder).unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o664,
            "auto_track should normalize file to world-readable"
        );
    }
}
