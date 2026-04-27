use crate::core::{drive, integrity, mirror};
use crate::db::repository::{Repository, SyncQueueItem, TrackedFile};
use crate::logging::event_logger;

/// Process a single pending sync queue item.
pub fn process_item(repo: &Repository, item: &SyncQueueItem) -> anyhow::Result<()> {
    let file = repo.get_tracked_file(item.tracked_file_id)?;
    let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;
    if pair.is_quiescing() {
        return Ok(());
    }

    // Items requiring manual user action are not auto-processable.
    if item.action == "user_action_required" {
        return Ok(());
    }

    repo.update_sync_queue_status(item.id, "in_progress", None)?;

    let result = match item.action.as_str() {
        "mirror" => mirror::mirror_file(&pair, &file.relative_path).map(|_| ()),
        "restore_master" => mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum),
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
            if matches!(
                item.action.as_str(),
                "mirror" | "restore_master" | "restore_mirror"
            ) {
                repo.update_tracked_file_mirror_status(file.id, true)?;
                let _ = drive::maybe_finalize_rebuild_for_action(repo, pair.id, &item.action);
            }
            let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
            let _ = event_logger::log_sync_completed(repo, file.id, &item.action, &full_path);
            if item.action == "mirror" {
                let _ = event_logger::log_file_mirrored(repo, file.id, &full_path, &file.checksum);
            }
        }
        Err(e) => {
            let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
            repo.update_sync_queue_status(item.id, "failed", Some(&e.to_string()))?;
            let _ = event_logger::log_sync_failed(
                repo,
                file.id,
                &item.action,
                &e.to_string(),
                &full_path,
            );
        }
    }

    Ok(())
}

/// Process all pending items in the sync queue.
pub fn process_all_pending(repo: &Repository) -> anyhow::Result<u32> {
    repo.requeue_in_progress_sync_queue()?;
    let page_size: i64 = 1000;
    let mut count: u32 = 0;
    loop {
        // Always fetch page 1: processed items are no longer "pending" so the
        // window slides forward naturally without needing an explicit offset.
        let (items, _) = repo.list_sync_queue(Some("pending"), 1, page_size)?;
        if items.is_empty() {
            break;
        }
        count += items
            .iter()
            .filter(|i| i.action != "user_action_required")
            .count() as u32;
        let all_skipped = items
            .iter()
            .all(|i| i.action == "user_action_required");
        for item in &items {
            if item.action == "user_action_required" {
                continue;
            }
            if let Err(e) = process_item(repo, item) {
                tracing::error!("Error processing sync queue item {}: {}", item.id, e);
            }
        }
        // Guard against an infinite loop if every remaining item is
        // user_action_required (those items are never processed, so the pending
        // list would never shrink).
        if all_skipped {
            break;
        }
    }
    Ok(count)
}

/// Resolve a `user_action_required` sync queue item.
///
/// `resolution` must be one of:
/// - `"keep_master"` — overwrite mirror with master copy
/// - `"keep_mirror"` — overwrite master with mirror copy
/// - `"provide_new"` — replace both copies with the file at `new_file_path`
///
/// For `provide_new`, the path is validated to exist, be readable, and be a
/// regular file before any copy is performed.
pub fn resolve_queue_item(
    repo: &Repository,
    item_id: i64,
    resolution: &str,
    new_file_path: Option<&str>,
) -> anyhow::Result<SyncQueueItem> {
    let item = repo.get_sync_queue_item(item_id)?;
    if item.action != "user_action_required" {
        anyhow::bail!(
            "Queue item #{} has action '{}'; only 'user_action_required' items can be resolved",
            item_id,
            item.action
        );
    }
    if item.status != "pending" {
        anyhow::bail!(
            "Queue item #{} has status '{}'; only 'pending' items can be resolved",
            item_id,
            item.status
        );
    }

    let file = repo.get_tracked_file(item.tracked_file_id)?;
    let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;

    let master_path = std::path::PathBuf::from(&pair.primary_path).join(&file.relative_path);
    let mirror_path = std::path::PathBuf::from(&pair.secondary_path).join(&file.relative_path);

    match resolution {
        "keep_master" => {
            // Restore mirror from master
            mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum)?;
        }
        "keep_mirror" => {
            // Restore master from mirror
            mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum)?;
        }
        "provide_new" => {
            let src = new_file_path
                .ok_or_else(|| anyhow::anyhow!("new_file_path is required for 'provide_new'"))?;
            let src_path = std::path::Path::new(src);

            // Pre-validate: exists, readable, regular file
            if !src_path.exists() {
                anyhow::bail!("provided path does not exist: {}", src);
            }
            if !src_path.is_file() {
                anyhow::bail!("provided path is not a regular file: {}", src);
            }
            std::fs::metadata(src_path)
                .map_err(|e| anyhow::anyhow!("provided path is not readable ({}): {}", src, e))?;

            // Ensure parent directories exist on both sides
            if let Some(parent) = master_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if let Some(parent) = mirror_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src_path, &master_path)?;
            std::fs::copy(src_path, &mirror_path)?;
        }
        other => anyhow::bail!(
            "Unknown resolution '{}'; expected keep_master, keep_mirror, or provide_new",
            other
        ),
    }

    repo.update_sync_queue_status(item_id, "completed", None)?;
    repo.update_tracked_file_mirror_status(file.id, true)?;
    let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
    let _ = event_logger::log_sync_completed(repo, file.id, resolution, &full_path);

    repo.get_sync_queue_item(item_id)
}

/// Create a sync queue item from an integrity check failure.
/// Returns None if the status is Ok or a drive is unavailable.
/// For BothCorrupted, creates a `user_action_required` item.
pub fn create_from_integrity_failure(
    repo: &Repository,
    file: &TrackedFile,
    result: &integrity::IntegrityCheckResult,
) -> anyhow::Result<Option<SyncQueueItem>> {
    use integrity::IntegrityStatus;
    let action = match result.status {
        IntegrityStatus::Ok => return Ok(None),
        IntegrityStatus::PrimaryDriveUnavailable | IntegrityStatus::SecondaryDriveUnavailable => {
            return Ok(None)
        }
        IntegrityStatus::BothCorrupted => "user_action_required",
        IntegrityStatus::MirrorCorrupted | IntegrityStatus::MirrorMissing => "restore_mirror",
        IntegrityStatus::MasterCorrupted | IntegrityStatus::MasterMissing => "restore_master",
    };
    repo.create_sync_queue_item_dedup(file.id, action)
}

/// Create a sync queue item to re-mirror a changed file.
pub fn create_from_change(repo: &Repository, file_id: i64) -> anyhow::Result<SyncQueueItem> {
    repo.create_sync_queue_item_dedup(file_id, "mirror")?
        .ok_or_else(|| anyhow::anyhow!("mirror action already pending for file #{}", file_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::checksum;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use tempfile::TempDir;

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
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", "hash", 1, None)
            .unwrap();
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

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", &checksum_str, content.len() as i64, None)
            .unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();

        process_item(&repo, &item).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(secondary.path().join("f.txt").exists());
    }

    #[test]
    fn test_process_all_pending_requeues_in_progress_items() {
        let (primary, secondary, repo) = setup();
        let content = b"requeue test content";
        fs::write(primary.path().join("requeue.txt"), content).unwrap();
        let checksum_str = checksum::checksum_bytes(content);

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(
                pair.id,
                "requeue.txt",
                &checksum_str,
                content.len() as i64,
                None,
            )
            .unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
        repo.update_sync_queue_status(item.id, "in_progress", None)
            .unwrap();

        let processed = process_all_pending(&repo).unwrap();

        assert_eq!(processed, 1);
        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(secondary.path().join("requeue.txt").exists());
    }

    #[test]
    fn test_queue_processing_handles_each_action_type() {
        let (primary, secondary, repo) = setup();
        let content = b"action test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("a.txt"), content).unwrap();
        fs::write(secondary.path().join("a.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "a.txt", &hash, content.len() as i64, None)
            .unwrap();

        for action in &["mirror", "restore_master", "restore_mirror", "verify"] {
            let item = repo.create_sync_queue_item(file.id, action).unwrap();
            process_item(&repo, &item).unwrap();
            let updated = repo.get_sync_queue_item(item.id).unwrap();
            assert_eq!(
                updated.status, "completed",
                "Action {} should complete",
                action
            );
        }
    }

    #[test]
    fn test_create_from_integrity_failure_mirror_corrupted() {
        let (primary, secondary, repo) = setup();
        let content = b"integrity test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("mi.txt"), content).unwrap();
        fs::write(secondary.path().join("mi.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "mi.txt", &hash, content.len() as i64, None)
            .unwrap();

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

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "ok.txt", &hash, content.len() as i64, None)
            .unwrap();
        let result = integrity::check_file_integrity(&pair, &file).unwrap();

        let item = create_from_integrity_failure(&repo, &file, &result).unwrap();
        assert!(
            item.is_none(),
            "No queue item should be created when integrity is Ok"
        );
    }

    #[test]
    fn test_create_from_change_creates_mirror_item() {
        let (primary, secondary, repo) = setup();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "changed.txt", "oldhash", 10, None)
            .unwrap();

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

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "cyc.txt", &hash, content.len() as i64, None)
            .unwrap();

        // Mirror is missing
        let result = integrity::check_file_integrity(&pair, &file).unwrap();
        assert_eq!(result.status, integrity::IntegrityStatus::MirrorMissing);

        let item = create_from_integrity_failure(&repo, &file, &result)
            .unwrap()
            .unwrap();
        process_item(&repo, &item).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        assert!(
            secondary.path().join("cyc.txt").exists(),
            "Mirror should be restored"
        );
    }

    /// Verifies that process_all_pending drains a queue larger than the internal
    /// page size (1000), so that no items are silently left behind.
    #[test]
    fn test_process_all_pending_drains_queue_beyond_page_size() {
        let (primary, secondary, repo) = setup();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();

        // Create 1002 distinct files and enqueue a mirror action for each.
        let n: usize = 1002;
        for i in 0..n {
            let name = format!("bulk-{i}.txt");
            let content = format!("content-{i}");
            fs::write(primary.path().join(&name), &content).unwrap();
            let hash = checksum::checksum_bytes(content.as_bytes());
            let file = repo
                .create_tracked_file(pair.id, &name, &hash, content.len() as i64, None)
                .unwrap();
            repo.create_sync_queue_item(file.id, "mirror").unwrap();
        }

        let processed = process_all_pending(&repo).unwrap();
        assert_eq!(processed as usize, n, "All {n} items should be processed");

        // Every mirror file must exist on the secondary side.
        for i in 0..n {
            let name = format!("bulk-{i}.txt");
            assert!(
                secondary.path().join(&name).exists(),
                "Mirror file {name} should exist after process_all_pending"
            );
        }
    }
}
