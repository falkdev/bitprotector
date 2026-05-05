use crate::core::checksum;
use crate::core::sync_queue;
use crate::db::repository::{DrivePair, Repository, TrackedFile};
use crate::logging::event_logger;
use std::path::PathBuf;

/// Represents a detected file change.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub relative_path: String,
    pub drive_pair_id: i64,
    pub old_checksum: String,
    pub new_checksum: String,
}

/// Detect changes in a tracked file by comparing current checksum to stored.
pub fn detect_change(
    primary_path: &str,
    relative_path: &str,
    stored_checksum: &str,
) -> anyhow::Result<Option<String>> {
    let full_path = PathBuf::from(primary_path).join(relative_path);
    if !full_path.exists() {
        return Ok(None);
    }
    let current = checksum::checksum_file(&full_path, checksum::ChecksumStrategy::Streaming)?;
    if current != stored_checksum {
        Ok(Some(current))
    } else {
        Ok(None)
    }
}

/// Scan all tracked files for a drive pair and return those whose checksum has changed.
pub fn scan_all_changes(
    repo: &Repository,
    drive_pair: &DrivePair,
) -> anyhow::Result<Vec<(TrackedFile, String)>> {
    let mut changes = Vec::new();
    let per_page = 200i64;
    let mut page = 1i64;
    loop {
        let (files, total) =
            repo.list_tracked_files(Some(drive_pair.id), None, None, page, per_page)?;
        if files.is_empty() {
            break;
        }
        for file in files {
            if let Some(new_hash) = detect_change(
                drive_pair.active_path(),
                &file.relative_path,
                &file.checksum,
            )? {
                changes.push((file, new_hash));
            }
        }
        if (page * per_page) >= total {
            break;
        }
        page += 1;
    }
    Ok(changes)
}

pub fn scan_and_record_changes(
    repo: &Repository,
    drive_pair: &DrivePair,
) -> anyhow::Result<Vec<(TrackedFile, String)>> {
    let changes = scan_all_changes(repo, drive_pair)?;
    for (file, new_hash) in &changes {
        let full_path = PathBuf::from(drive_pair.active_path()).join(&file.relative_path);
        let new_size = full_path.metadata()?.len() as i64;
        repo.update_tracked_file_checksum(file.id, new_hash, new_size)?;
        repo.update_tracked_file_mirror_status(file.id, false)?;
        let _ = event_logger::log_event(
            repo,
            "change_detected",
            Some(file.id),
            &format!(
                "Change detected on active {} drive: {}/{}",
                drive_pair.active_role, drive_pair.primary_path, file.relative_path
            ),
            Some(new_hash),
        );
        if drive_pair.standby_accepts_sync() {
            let _ = sync_queue::create_from_change(repo, file.id)?;
        }
    }
    Ok(changes)
}

/// Start a filesystem watcher on a directory. Returns the watcher handle (keep alive to continue watching).
/// `on_event` is called for each filesystem event detected.
pub fn watch_folder<F>(folder_path: &str, on_event: F) -> anyhow::Result<notify::RecommendedWatcher>
where
    F: Fn(notify::Event) + Send + 'static,
{
    use notify::{RecursiveMode, Watcher};
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            on_event(event);
        }
    })?;
    watcher.watch(std::path::Path::new(folder_path), RecursiveMode::Recursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::drive::{self, DriveRole};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_no_change_when_file_unchanged() {
        let dir = TempDir::new().unwrap();
        let content = b"unchanged content";
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let stored = checksum::checksum_bytes(content);

        let change = detect_change(dir.path().to_str().unwrap(), "f.txt", &stored).unwrap();
        assert!(change.is_none());
    }

    #[test]
    fn test_changed_file_detected() {
        let dir = TempDir::new().unwrap();
        let original = b"original";
        let stored = checksum::checksum_bytes(original);
        // Write modified content
        fs::write(dir.path().join("f.txt"), b"modified content").unwrap();

        let change = detect_change(dir.path().to_str().unwrap(), "f.txt", &stored).unwrap();
        assert!(change.is_some());
        let new_hash = change.unwrap();
        assert_eq!(new_hash, checksum::checksum_bytes(b"modified content"));
    }

    #[test]
    fn test_missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let change = detect_change(dir.path().to_str().unwrap(), "missing.txt", "anyhash").unwrap();
        assert!(change.is_none());
    }

    #[test]
    fn test_scan_all_changes_detects_modified_file() {
        use crate::core::tracker;
        use crate::db::repository::{create_memory_pool, Repository};
        use crate::db::schema::initialize_schema;

        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        let repo = Repository::new(pool);

        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        fs::write(primary.path().join("scan.txt"), b"original content").unwrap();
        tracker::track_file(&repo, &pair, "scan.txt", None).unwrap();

        // Modify the file so it shows as changed
        fs::write(primary.path().join("scan.txt"), b"modified content").unwrap();

        let changes = scan_all_changes(&repo, &pair).unwrap();
        assert_eq!(changes.len(), 1, "Should detect the modified file");
        assert_eq!(changes[0].0.relative_path, "scan.txt");
    }

    #[test]
    fn test_scan_all_changes_unchanged_file_not_reported() {
        use crate::core::tracker;
        use crate::db::repository::{create_memory_pool, Repository};
        use crate::db::schema::initialize_schema;

        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        let repo = Repository::new(pool);

        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        fs::write(primary.path().join("stable.txt"), b"no changes here").unwrap();
        tracker::track_file(&repo, &pair, "stable.txt", None).unwrap();

        let changes = scan_all_changes(&repo, &pair).unwrap();
        assert!(changes.is_empty(), "Unchanged file should not be reported");
    }

    #[test]
    fn test_scan_and_record_changes_updates_checksum_on_secondary_failover() {
        use crate::core::tracker;
        use crate::db::repository::{create_memory_pool, Repository};
        use crate::db::schema::initialize_schema;

        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        let repo = Repository::new(pool);

        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        fs::create_dir(primary.path().join("docs")).unwrap();
        fs::create_dir(secondary.path().join("docs")).unwrap();
        fs::write(primary.path().join("docs/file.txt"), b"original").unwrap();
        fs::write(secondary.path().join("docs/file.txt"), b"original").unwrap();
        let tracked = tracker::track_file(&repo, &pair, "docs/file.txt", None).unwrap();
        repo.update_tracked_file_mirror_status(tracked.id, true)
            .unwrap();

        drive::mark_drive_quiescing(&repo, pair.id, DriveRole::Primary).unwrap();
        let failed_over = drive::confirm_drive_failure(&repo, pair.id, DriveRole::Primary).unwrap();

        fs::write(
            secondary.path().join("docs/file.txt"),
            b"edited on secondary",
        )
        .unwrap();

        let changes = scan_and_record_changes(&repo, &failed_over).unwrap();
        assert_eq!(changes.len(), 1);

        let updated = repo.get_tracked_file(tracked.id).unwrap();
        assert_eq!(
            updated.checksum,
            checksum::checksum_bytes(b"edited on secondary")
        );
        assert!(!updated.is_mirrored);

        let (items, total) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
        assert_eq!(total, 0);
        assert!(items.is_empty());
    }
}
