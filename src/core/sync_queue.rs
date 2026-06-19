use crate::core::{drive, integrity, mirror};
use crate::db::repository::{Repository, SyncQueueItem, TrackedFile};
use crate::logging::event_logger;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// Summary of the sync queue state, broadcast over the `SyncEventBus`.
#[derive(Debug, Clone, Serialize)]
pub struct SyncSummary {
    pub total: i64,
    pub active_items: i64,
    pub pending_items: i64,
    pub in_progress_items: i64,
    pub completed_items: i64,
    pub failed_items: i64,
    pub queue_paused: bool,
    pub processing_active: bool,
    /// True when at least one tracked folder is currently being scanned.
    pub scanning: bool,
    pub scan_total_files: i64,
    pub scan_scanned_files: i64,
    pub scan_active_folders: i64,
    /// Monotonically increasing counter; incremented on every publish.
    pub revision: u64,
}

/// A broadcast bus that pushes [`SyncSummary`] snapshots to all SSE subscribers.
///
/// `SyncEventBus` is cheap to clone — all clones share the same underlying
/// broadcast channel, `processing_active` flag, and revision counter.
#[derive(Clone)]
pub struct SyncEventBus {
    tx: tokio::sync::broadcast::Sender<SyncSummary>,
    pub processing_active: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
}

impl SyncEventBus {
    /// Create a new bus with a channel capacity of 64 events.
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(64);
        Self {
            tx,
            processing_active: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Subscribe to future events.  Late subscribers will miss earlier events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SyncSummary> {
        self.tx.subscribe()
    }

    /// Build a current snapshot from repository state (increments the revision).
    pub fn snapshot(&self, repo: &Repository) -> SyncSummary {
        let counts = repo.count_sync_queue_by_status().unwrap_or_default();
        let active = repo.count_active_sync_queue().unwrap_or(0);
        let in_progress = repo.count_in_progress_sync_queue().unwrap_or(0);
        let paused = repo.get_sync_queue_paused().unwrap_or(false);
        let folders = repo.list_tracked_folders().unwrap_or_default();

        let pending = *counts.get("pending").unwrap_or(&0);
        let completed = *counts.get("completed").unwrap_or(&0);
        let failed = *counts.get("failed").unwrap_or(&0);

        let scanning_total_files: i64 = folders
            .iter()
            .filter(|f| f.scanning)
            .map(|f| f.scan_total_files)
            .sum();
        let scanning_scanned_files: i64 = folders
            .iter()
            .filter(|f| f.scanning)
            .map(|f| f.scan_scanned_files.min(f.scan_total_files))
            .sum();
        let scan_active_folders = folders.iter().filter(|f| f.scanning).count() as i64;

        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;

        SyncSummary {
            total: active + completed + failed,
            active_items: active,
            pending_items: pending,
            in_progress_items: in_progress,
            completed_items: completed,
            failed_items: failed,
            queue_paused: paused,
            processing_active: self.processing_active.load(Ordering::SeqCst),
            scanning: scan_active_folders > 0,
            scan_total_files: scanning_total_files,
            scan_scanned_files: scanning_scanned_files,
            scan_active_folders,
            revision,
        }
    }

    /// Build a snapshot and broadcast it to all current subscribers.
    /// If there are no subscribers the send is silently discarded.
    pub fn publish_snapshot(&self, repo: &Repository) {
        let summary = self.snapshot(repo);
        let _ = self.tx.send(summary);
    }
}

impl Default for SyncEventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Process a single pending sync queue item.
pub fn process_item(
    repo: &Repository,
    item: &SyncQueueItem,
    bus: Option<&SyncEventBus>,
) -> anyhow::Result<()> {
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
    if let Some(b) = bus {
        b.publish_snapshot(repo);
    }

    let result = match item.action.as_str() {
        "mirror" => mirror::mirror_file(&pair, &file.relative_path).map(|_| ()),
        "adopt_mirror" => {
            use anyhow::Context as _;
            let standby_path =
                std::path::PathBuf::from(pair.standby_path()).join(&file.relative_path);
            if drive::path_is_available(pair.standby_path()) && standby_path.exists() {
                let standby_checksum = crate::core::checksum::checksum_file(
                    &standby_path,
                    crate::core::checksum::ChecksumStrategy::Streaming,
                )
                .context("Failed to checksum standby file")?;
                if standby_checksum != file.checksum {
                    mirror::mirror_file(&pair, &file.relative_path).map(|_| ())?;
                }
            } else {
                mirror::mirror_file(&pair, &file.relative_path).map(|_| ())?;
            }
            Ok(())
        }
        "restore_master" => mirror::restore_from_mirror(&pair, &file.relative_path, &file.checksum),
        "restore_mirror" => {
            mirror::restore_mirror_from_master(&pair, &file.relative_path, &file.checksum)
        }
        "verify" => {
            let result = integrity::check_file_integrity(
                &pair,
                &file,
                crate::core::checksum::ChecksumStrategy::Streaming,
                crate::core::checksum::ChecksumStrategy::Streaming,
            )?;
            if result.status == integrity::IntegrityStatus::Ok {
                Ok(())
            } else {
                anyhow::bail!("Integrity check failed")
            }
        }
        action => anyhow::bail!("Unknown action: {action}"),
    };

    match result {
        Ok(()) => {
            repo.update_sync_queue_status(item.id, "completed", None)?;
            if let Some(b) = bus {
                b.publish_snapshot(repo);
            }
            if matches!(
                item.action.as_str(),
                "mirror" | "adopt_mirror" | "restore_master" | "restore_mirror"
            ) {
                repo.update_tracked_file_mirror_status(file.id, true)?;
                let _ = drive::maybe_finalize_rebuild_for_action(repo, pair.id, &item.action);
            }
            let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
            let _ = event_logger::log_sync_completed(repo, file.id, &item.action, &full_path);
            if item.action == "mirror" || item.action == "adopt_mirror" {
                let _ = event_logger::log_file_mirrored(repo, file.id, &full_path, &file.checksum);
            }
        }
        Err(e) => {
            let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
            repo.update_sync_queue_status(item.id, "failed", Some(&e.to_string()))?;
            if let Some(b) = bus {
                b.publish_snapshot(repo);
            }
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
///
/// - If `stop_by` is `Some(instant)` and the deadline has passed, processing
///   stops early; remaining items stay `pending` for the next run.
/// - If the queue is globally paused (see `repo.set_sync_queue_paused`),
///   processing stops immediately and pending items are left untouched.
/// - If `bus` is `Some`, a summary snapshot is published after every item
///   status transition so SSE subscribers receive live progress.
pub fn process_all_pending(
    repo: &Repository,
    stop_by: Option<std::time::Instant>,
    bus: Option<&SyncEventBus>,
) -> anyhow::Result<u32> {
    repo.requeue_in_progress_sync_queue()?;
    let page_size: i64 = 1000;
    let mut processed = 0u32;
    loop {
        // Always fetch page 1: processed items are no longer "pending" so the
        // window slides forward naturally without needing an explicit offset.
        let (items, _) = repo.list_sync_queue(Some("pending"), 1, page_size)?;
        if items.is_empty() {
            break;
        }
        let all_skipped = items.iter().all(|i| i.action == "user_action_required");
        for item in &items {
            if item.action == "user_action_required" {
                continue;
            }
            // Check global pause flag.
            if repo.get_sync_queue_paused()? {
                return Ok(processed);
            }
            // Honour scheduler time window.
            if let Some(dl) = stop_by {
                if std::time::Instant::now() >= dl {
                    return Ok(processed);
                }
            }
            if let Err(e) = process_item(repo, item, bus) {
                tracing::error!("Error processing sync queue item {}: {}", item.id, e);
            }
            processed += 1;
        }
        // Guard against an infinite loop if every remaining item is
        // user_action_required (those items are never processed, so the pending
        // list would never shrink).
        if all_skipped {
            break;
        }
    }
    Ok(processed)
}

/// Start an asynchronous background worker that calls [`process_all_pending`].
///
/// Returns immediately (the background thread does the work).  A second call
/// while the worker is still running is a no-op — only one worker runs at a
/// time.  The `bus.processing_active` flag is set to `true` for the duration
/// of the run and cleared when it completes.
pub fn process_all_pending_async(repo: &Repository, bus: SyncEventBus) {
    // Idempotent: refuse to start a second concurrent run.
    if bus
        .processing_active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let repo_clone = repo.clone();
    let bus_clone = bus.clone();
    std::thread::spawn(move || {
        bus_clone.publish_snapshot(&repo_clone);
        let _ = process_all_pending(&repo_clone, None, Some(&bus_clone));
        bus_clone.processing_active.store(false, Ordering::SeqCst);
        bus_clone.publish_snapshot(&repo_clone);
    });
}

/// Resolve a `user_action_required` sync queue item.
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
                anyhow::bail!("provided path does not exist: {src}");
            }
            if !src_path.is_file() {
                anyhow::bail!("provided path is not a regular file: {src}");
            }
            std::fs::metadata(src_path)
                .map_err(|e| anyhow::anyhow!("provided path is not readable ({src}): {e}"))?;

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
            "Unknown resolution '{other}'; expected keep_master, keep_mirror, or provide_new"
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
        .ok_or_else(|| anyhow::anyhow!("mirror action already pending for file #{file_id}"))
}

/// Create a sync queue item for a newly tracked file.
/// Uses `adopt_mirror` so the processor will verify before copying.
pub fn create_for_new_tracking(repo: &Repository, file_id: i64) -> anyhow::Result<SyncQueueItem> {
    let (item, created) =
        repo.create_sync_queue_item_dedup_with_created(file_id, "adopt_mirror")?;
    if created {
        Ok(item)
    } else {
        Err(anyhow::anyhow!(
            "adopt_mirror action already pending for file #{file_id}"
        ))
    }
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

        process_item(&repo, &item, None).unwrap();

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

        let processed = process_all_pending(&repo, None, None).unwrap();

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
            process_item(&repo, &item, None).unwrap();
            let updated = repo.get_sync_queue_item(item.id).unwrap();
            assert_eq!(
                updated.status, "completed",
                "Action {action} should complete"
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
        let result = integrity::check_file_integrity(
            &pair,
            &file,
            crate::core::checksum::ChecksumStrategy::Streaming,
            crate::core::checksum::ChecksumStrategy::Streaming,
        )
        .unwrap();

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
        let result = integrity::check_file_integrity(
            &pair,
            &file,
            crate::core::checksum::ChecksumStrategy::Streaming,
            crate::core::checksum::ChecksumStrategy::Streaming,
        )
        .unwrap();

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
        let result = integrity::check_file_integrity(
            &pair,
            &file,
            crate::core::checksum::ChecksumStrategy::Streaming,
            crate::core::checksum::ChecksumStrategy::Streaming,
        )
        .unwrap();
        assert_eq!(result.status, integrity::IntegrityStatus::MirrorMissing);

        let item = create_from_integrity_failure(&repo, &file, &result)
            .unwrap()
            .unwrap();
        process_item(&repo, &item, None).unwrap();

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

        let processed = process_all_pending(&repo, None, None).unwrap();
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

    #[test]
    fn test_process_all_pending_stops_when_paused() {
        let (primary, secondary, repo) = setup();
        let content = b"pause test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("p1.txt"), content).unwrap();
        fs::write(primary.path().join("p2.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let f1 = repo
            .create_tracked_file(pair.id, "p1.txt", &hash, content.len() as i64, None)
            .unwrap();
        let f2 = repo
            .create_tracked_file(pair.id, "p2.txt", &hash, content.len() as i64, None)
            .unwrap();
        repo.create_sync_queue_item(f1.id, "mirror").unwrap();
        repo.create_sync_queue_item(f2.id, "mirror").unwrap();

        // Pause the queue before processing
        repo.set_sync_queue_paused(true).unwrap();
        let processed = process_all_pending(&repo, None, None).unwrap();

        assert_eq!(processed, 0, "No items should be processed while paused");

        // Resume and process
        repo.set_sync_queue_paused(false).unwrap();
        let processed = process_all_pending(&repo, None, None).unwrap();
        assert_eq!(processed, 2, "Both items should be processed after resume");
    }

    #[test]
    fn test_process_all_pending_stops_at_deadline() {
        use std::time::{Duration, Instant};

        let (primary, _secondary, repo) = setup();
        let content = b"deadline test";
        let hash = checksum::checksum_bytes(content);

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                _secondary.path().to_str().unwrap(),
            )
            .unwrap();

        // Enqueue many items
        for i in 0..20 {
            let fname = format!("dl{i}.txt");
            fs::write(primary.path().join(&fname), content).unwrap();
            let f = repo
                .create_tracked_file(pair.id, &fname, &hash, content.len() as i64, None)
                .unwrap();
            repo.create_sync_queue_item(f.id, "mirror").unwrap();
        }

        // Deadline already in the past — nothing should be processed
        let past = Instant::now() - Duration::from_secs(1);
        let processed = process_all_pending(&repo, Some(past), None).unwrap();
        assert_eq!(processed, 0, "Expired deadline should prevent processing");
    }

    #[test]
    fn test_process_adopt_mirror_standby_matches() {
        let (primary, secondary, repo) = setup();
        let content = b"identical content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("f.txt"), content).unwrap();
        fs::write(secondary.path().join("f.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", &hash, content.len() as i64, None)
            .unwrap();
        let item = repo
            .create_sync_queue_item(file.id, "adopt_mirror")
            .unwrap();

        process_item(&repo, &item, None).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        let updated_file = repo.get_tracked_file(file.id).unwrap();
        assert!(updated_file.is_mirrored, "File should be marked mirrored");
        assert_eq!(
            fs::read(secondary.path().join("f.txt")).unwrap(),
            content,
            "Secondary content should be unchanged (no copy needed)"
        );
    }

    #[test]
    fn test_process_adopt_mirror_standby_stale() {
        let (primary, secondary, repo) = setup();
        let content_primary = b"primary content";
        let content_stale = b"stale content";
        let hash = checksum::checksum_bytes(content_primary);
        fs::write(primary.path().join("f.txt"), content_primary).unwrap();
        fs::write(secondary.path().join("f.txt"), content_stale).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", &hash, content_primary.len() as i64, None)
            .unwrap();
        let item = repo
            .create_sync_queue_item(file.id, "adopt_mirror")
            .unwrap();

        process_item(&repo, &item, None).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        let updated_file = repo.get_tracked_file(file.id).unwrap();
        assert!(updated_file.is_mirrored);
        assert_eq!(
            fs::read(secondary.path().join("f.txt")).unwrap(),
            content_primary,
            "Secondary should now hold primary content after full copy"
        );
    }

    #[test]
    fn test_process_adopt_mirror_standby_missing() {
        let (primary, secondary, repo) = setup();
        let content = b"new file content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("f.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "f.txt", &hash, content.len() as i64, None)
            .unwrap();
        let item = repo
            .create_sync_queue_item(file.id, "adopt_mirror")
            .unwrap();

        process_item(&repo, &item, None).unwrap();

        let updated = repo.get_sync_queue_item(item.id).unwrap();
        assert_eq!(updated.status, "completed");
        let updated_file = repo.get_tracked_file(file.id).unwrap();
        assert!(updated_file.is_mirrored);
        assert!(
            secondary.path().join("f.txt").exists(),
            "Secondary file should now exist"
        );
        assert_eq!(fs::read(secondary.path().join("f.txt")).unwrap(), content);
    }

    // ── SyncEventBus tests ────────────────────────────────────────────────

    #[test]
    fn test_sync_event_bus_snapshot_includes_counts_and_revision() {
        let (primary, secondary, repo) = setup();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "ev.txt", "h", 1, None)
            .unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();
        repo.update_sync_queue_status(item.id, "completed", None)
            .unwrap();

        let bus = SyncEventBus::new();
        let s1 = bus.snapshot(&repo);
        assert_eq!(s1.completed_items, 1);
        assert_eq!(s1.revision, 1);

        let s2 = bus.snapshot(&repo);
        assert_eq!(s2.revision, 2, "revision increments on each snapshot");
    }

    #[test]
    fn test_sync_event_bus_publish_reaches_subscriber() {
        let (primary, secondary, repo) = setup();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "sub.txt", "h", 1, None)
            .unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let bus = SyncEventBus::new();
        let mut rx = bus.subscribe();
        bus.publish_snapshot(&repo);

        let summary = rx.try_recv().expect("should have received a summary");
        assert_eq!(summary.pending_items, 1);
        assert!(!summary.processing_active);
    }

    #[test]
    fn test_process_item_with_bus_broadcasts_status_transitions() {
        let (primary, secondary, repo) = setup();
        let content = b"bus test";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("btest.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "btest.txt", &hash, content.len() as i64, None)
            .unwrap();
        let item = repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let bus = SyncEventBus::new();
        let mut rx = bus.subscribe();

        process_item(&repo, &item, Some(&bus)).unwrap();

        // Two events expected: in_progress and completed
        let ev1 = rx.try_recv().expect("in_progress event");
        let ev2 = rx.try_recv().expect("completed event");
        assert!(ev2.revision > ev1.revision, "revision must increase");
        assert_eq!(ev2.in_progress_items, 0);
        assert_eq!(ev2.completed_items, 1);
    }

    #[test]
    fn test_process_all_pending_async_is_idempotent() {
        let (primary, secondary, repo) = setup();
        // Create items that would take time to process
        let content = b"async idempotent";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("ai.txt"), content).unwrap();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "ai.txt", &hash, content.len() as i64, None)
            .unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let bus = SyncEventBus::new();
        // Manually set processing_active to simulate a concurrent run.
        bus.processing_active
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Second call should be a no-op (doesn't panic, doesn't spawn).
        process_all_pending_async(&repo, bus.clone());
        // Still true — idempotent call didn't clear the flag.
        assert!(
            bus.processing_active
                .load(std::sync::atomic::Ordering::SeqCst),
            "flag must still be set after idempotent call"
        );
    }
}
