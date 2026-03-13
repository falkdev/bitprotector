use crate::db::repository::{Repository, SyncQueueItem};
use crate::core::{mirror, integrity};

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
        Ok(()) => repo.update_sync_queue_status(item.id, "completed", None)?,
        Err(e) => repo.update_sync_queue_status(item.id, "failed", Some(&e.to_string()))?,
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
}
