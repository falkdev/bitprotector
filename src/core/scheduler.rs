use crate::core::{drive, integrity, sync_queue};
use crate::db::repository::Repository;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

/// Represents a scheduled task type.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskType {
    Sync,
    IntegrityCheck,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Sync => "sync",
            TaskType::IntegrityCheck => "integrity_check",
        }
    }
}

/// Run a task immediately against the repository. Returns the number of items processed.
pub fn run_task(task: &TaskType, repo: &Repository) -> anyhow::Result<u32> {
    match task {
        TaskType::Sync => sync_queue::process_all_pending(repo),
        TaskType::IntegrityCheck => {
            let pairs = repo.list_drive_pairs()?;
            let mut count = 0u32;
            for pair in pairs {
                let pair = match drive::load_operational_pair(repo, pair.id) {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::error!(
                            "Failed to load drive pair {} for integrity task: {}",
                            pair.id,
                            e
                        );
                        continue;
                    }
                };
                if pair.is_quiescing() {
                    continue;
                }
                let (files, _) = repo.list_tracked_files(Some(pair.id), None, None, 1, i64::MAX)?;
                for file in files {
                    let result = integrity::check_file_integrity(&pair, &file)?;
                    if result.status != integrity::IntegrityStatus::Ok {
                        if let Some(_item) =
                            sync_queue::create_from_integrity_failure(repo, &file, &result)?
                        {
                            count += 1;
                        }
                    }
                }
            }
            Ok(count)
        }
    }
}

/// Background task scheduler. Spawns threads that run tasks at fixed intervals.
pub struct Scheduler {
    repo: Arc<Repository>,
    stop_flag: Arc<AtomicBool>,
}

impl Scheduler {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self {
            repo,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Schedule a task to run every `interval_seconds` seconds in a background thread.
    pub fn schedule(&self, task: TaskType, interval_seconds: u64) -> thread::JoinHandle<()> {
        let repo = Arc::clone(&self.repo);
        let stop = Arc::clone(&self.stop_flag);
        thread::spawn(move || {
            loop {
                // Run the task before checking stop so it always executes at least once.
                if let Err(e) = run_task(&task, &*repo) {
                    tracing::error!("Scheduled task '{}' failed: {}", task.as_str(), e);
                }
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                // Sleep in small increments to check stop_flag promptly.
                let steps = (interval_seconds * 10).max(1);
                for _ in 0..steps {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        })
    }

    /// Signal all scheduled tasks to stop at their next check.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
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
        let conn = pool.get().unwrap();
        initialize_schema(&conn).unwrap();
        drop(conn);
        (primary, secondary, Repository::new(pool))
    }

    #[test]
    fn test_run_sync_task_processes_pending_items() {
        let (primary, secondary, repo) = setup();
        let content = b"scheduler sync content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("sched.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "sched.txt", &hash, content.len() as i64, None)
            .unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let processed = run_task(&TaskType::Sync, &repo).unwrap();
        assert_eq!(processed, 1, "Should process one pending item");

        assert!(
            secondary.path().join("sched.txt").exists(),
            "File should be mirrored"
        );
    }

    #[test]
    fn test_run_integrity_task_queues_failed_files() {
        let (primary, secondary, repo) = setup();
        let content = b"integrity content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("integ.txt"), content).unwrap();
        // No secondary file → mirror missing

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        repo.create_tracked_file(pair.id, "integ.txt", &hash, content.len() as i64, None)
            .unwrap();

        let queued = run_task(&TaskType::IntegrityCheck, &repo).unwrap();
        assert_eq!(queued, 1, "Should enqueue one integrity failure");

        let (items, _) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
        assert!(!items.is_empty(), "Queue should have a pending item");
    }

    #[test]
    fn test_scheduler_processes_items_in_background() {
        let (primary, secondary, repo) = setup();
        let content = b"bg content";
        let hash = checksum::checksum_bytes(content);
        fs::write(primary.path().join("bg.txt"), content).unwrap();

        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let file = repo
            .create_tracked_file(pair.id, "bg.txt", &hash, content.len() as i64, None)
            .unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let repo_arc = Arc::new(repo);
        let scheduler = Scheduler::new(Arc::clone(&repo_arc));
        let handle = scheduler.schedule(TaskType::Sync, 0); // interval 0 = run then stop
        scheduler.stop();
        handle.join().unwrap();

        assert!(
            secondary.path().join("bg.txt").exists(),
            "Background scheduler should mirror the file"
        );
    }

    #[test]
    fn test_task_type_as_str() {
        assert_eq!(TaskType::Sync.as_str(), "sync");
        assert_eq!(TaskType::IntegrityCheck.as_str(), "integrity_check");
    }
}
