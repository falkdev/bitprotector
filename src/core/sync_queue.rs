use crate::db::repository::{Repository, SyncQueueItem, TrackedFile};
use crate::core::{mirror, integrity};
use crate::logging::event_logger;

/// Process a single pending sync queue item.
pub fn process_item(
    repo: &Repository,
    item: &SyncQueueItem,
) -> anyhow::Result<()> {
    repo.update_sync_queue_status(item.id, "in_progress", None)?;

    let file = repo.get_tracked_file(item.tracked_file_id)?;
    let pair = repo.get_drive_pair(file.drive_pair_id)?;

    let result = match item.action.as_str() {
        "mirror" => {
            mirror::mirror_file(&pair, &file.relative_path)
                .map(|_| ())
        }
        "restore_master" => {
            mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum)
        }
        "restore_mirror" => {
            mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum)
        }
        "verify" => {
            let result = integrity::check_file_integrity(&pair, &file)?;
            if result.status == integrity::IntegrityStatus::Ok {
                Ok(())
            } else {
                anyhow::bail!("Integrity check failed")
            }
        }
        action => anyhow::bail!("Unknown action: {}", action),
    };

    match result {
        Ok(()) => {
            repo.update_sync_queue_status(item.id, "completed", None)?;
            let _ = event_logger::log_sync_completed(repo, file.id, &item.action);
        }
        Err(e) => {
            repo.update_sync_queue_status(item.id, "failed", Some(&e.to_string()))?;
            let _ = event_logger::log_sync_failed(repo, file.id, &item.action, &e.to_string());
        }
    }

    Ok(())
}

/// Process all pending items in the sync queue.
pub fn process_all_pending(repo: &Repository) -> anyhow::Result<u32> {
    let (items, _) = repo.list_sync_queue(Some("pending"), 1, 1000)?;
    let count = items.len() as u32;
    for item in &items {
        if let Err(e) = process_item(repo, item) {
            tracing::error!("Error processing sync queue item {}: {}", item.id, e);
        }
    }
    Ok(count)
}

/// Create a sync queue item from an integrity check failure.
/// Returns None if the status is Ok or BothCorrupted (requires manual resolution).
pub fn create_from_integrity_failure(
    repo: &Repository,
    file: &TrackedFile,
    result: &integrity::IntegrityCheckResult,
) -> anyhow::Result<Option<SyncQueueItem>> {
    use integrity::IntegrityStatus;
    let action = match result.status {
        IntegrityStatus::Ok => return Ok(None),
        IntegrityStatus::BothCorrupted => return Ok(None),
        IntegrityStatus::MirrorCorrupted | IntegrityStatus::MirrorMissing => "restore_mirror",
        IntegrityStatus::MasterCorrupted | IntegrityStatus::MasterMissing => "restore_master",
    };
    Ok(Some(repo.create_sync_queue_item(file.id, action)?))
}

/// Create a sync queue item to re-mirror a changed file.
pub fn create_from_change(repo: &Repository, file_id: i64) -> anyhow::Result<SyncQueueItem> {
    repo.create_sync_queue_item(file_id, "mirror")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use crate::core::checksum;

    fn setup() -> (TempDir, TempDir, Repository) {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        (primary, secondary, Repository::new(pool))
    }

    #[test]
    fn test_queue_item_created_from_file() {
        let (primary, secondary, repo) = setup();
        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "f.txt", "hash", 1, None).unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
        assert_eq!(item.action, "mirror");
        assert_eq!(item.status, "pending");
        assert_eq!(item.tracked_file_id, file.id);
    }

    #[test]
    fn test_queue_processing_mirror_action() {
        let (primary, secondary, repo) = setup();
        let content = b"queue test content";
        fs::write(primary.path().join("f.txt"), content).unwrap();
        let checksum_str = checksum::checksum_bytes(content);

        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "f.txt", &checksum_str, content.len() as i64, None).unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();

        process_item(&repo, &item).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(secondary.path().join("f.txt").exists());
    }

    #[test]
    fn test_queue_processing_handles_each_action_type() {
        let (primary, secondary, repo) = setup();
        let content = b"action test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("a.txt"), content).unwrap();
        fs::write(secondary.path().join("a.txt"), content).unwrap();

        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "a.txt", &hash, content.len() as i64, None).unwrap();

        for action in &["mirror", "restore_master", "restore_mirror", "verify"] {
            let item = repo.create_sync_queue_item(file.id, action).unwrap();
            process_item(&repo, &item).unwrap();
            let updated = repo.get_sync_queue_item(item.id).unwrap();
            assert_eq!(updated.status, "completed", "Action {} should complete", action);
        }
    }

    #[test]
    fn test_create_from_integrity_failure_mirror_corrupted() {
        let (primary, secondary, repo) = setup();
        let content = b"integrity test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("mi.txt"), content).unwrap();
        fs::write(secondary.path().join("mi.txt"), content).unwrap();

        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "mi.txt", &hash, content.len() as i64, None).unwrap();

        // Corrupt the mirror
        fs::write(secondary.path().join("mi.txt"), b"corrupted").unwrap();
        let result = integrity::check_file_integrity(&pair, &file).unwrap();

        let item = create_from_integrity_failure(&repo, &file, &result).unwrap();
        assert!(item.is_some());
        assert_eq!(item.unwrap().action, "restore_mirror");
    }

    #[test]
    fn test_create_from_integrity_failure_ok_returns_none() {
        let (primary, secondary, repo) = setup();
        let content = b"synced content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("ok.txt"), content).unwrap();
        fs::write(secondary.path().join("ok.txt"), content).unwrap();

        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "ok.txt", &hash, content.len() as i64, None).unwrap();
        let result = integrity::check_file_integrity(&pair, &file).unwrap();

        let item = create_from_integrity_failure(&repo, &file, &result).unwrap();
        assert!(item.is_none(), "No queue item should be created when integrity is Ok");
    }

    #[test]
    fn test_create_from_change_creates_mirror_item() {
        let (primary, secondary, repo) = setup();
        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "changed.txt", "oldhash", 10, None).unwrap();

        let item = create_from_change(&repo, file.id).unwrap();
        assert_eq!(item.action, "mirror");
        assert_eq!(item.tracked_file_id, file.id);
    }

    #[test]
    fn test_integrity_failure_queue_resolve_cycle() {
        let (primary, secondary, repo) = setup();
        let content = b"cycle content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("cyc.txt"), content).unwrap();

        let pair = repo.create_drive_pair("p", primary.path().to_str().unwrap(), secondary.path().to_str().unwrap()).unwrap();
        let file = repo.create_tracked_file(pair.id, "cyc.txt", &hash, content.len() as i64, None).unwrap();

        // Mirror is missing
        let result = integrity::check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, integrity::IntegrityStatus::MirrorMissing);

        let item = create_from_integrity_failure(&repo, &file, &result).unwrap().unwrap();
        process_item(&repo, &item).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(secondary.path().join("cyc.txt").exists(), "Mirror should be restored");
    }
}
