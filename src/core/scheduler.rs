use crate::core::{integrity_runs, sync_queue};
use crate::db::repository::{Repository, ScheduleConfig};
use std::collections::HashMap;
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
        TaskType::IntegrityCheck => integrity_runs::run_sync(repo, None, false, "scheduler")
            .map(|run| run.attention_files as u32),
    }
}

/// Compute milliseconds until the next occurrence of a standard 5-field cron expression.
/// Converts `"min hour dom month dow"` to the `cron` crate's 7-field format.
fn next_cron_sleep_ms(expr: &str) -> anyhow::Result<u64> {
    use chrono::Utc;
    use cron::Schedule;
    use std::str::FromStr;

    // Standard 5-field: "min hour dom month dow"
    // cron crate 7-field: "sec min hour dom month dow year"
    let seven_field = format!("0 {} *", expr);
    let schedule = Schedule::from_str(&seven_field)
        .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", expr, e))?;
    let next = schedule
        .upcoming(Utc)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No upcoming occurrence for cron '{}'", expr))?;
    let ms = (next - Utc::now()).num_milliseconds().max(0) as u64;
    Ok(ms)
}

/// Background task scheduler.
///
/// Each active `ScheduleConfig` from the database is backed by a dedicated OS thread
/// with its own `Arc<AtomicBool>` stop flag. Calling `reload()` stops threads for
/// removed/disabled schedules and starts threads for newly added/enabled ones.
pub struct Scheduler {
    repo: Arc<Repository>,
    /// schedule_id → (stop_flag, thread_handle)
    threads: HashMap<i64, (Arc<AtomicBool>, thread::JoinHandle<()>)>,
}

impl Scheduler {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self {
            repo,
            threads: HashMap::new(),
        }
    }

    /// Reload schedules from the database.
    ///
    /// - Stops threads for schedules that have been removed or disabled.
    /// - Starts new threads for schedules that are enabled but not yet running.
    /// - Does **not** restart threads for schedules that are already running.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let configs = self.repo.list_schedule_configs()?;

        // ── Stop threads for removed/disabled schedules ─────────────────────
        let ids_to_stop: Vec<i64> = self
            .threads
            .keys()
            .filter(|&&id| !configs.iter().any(|c| c.id == id && c.enabled))
            .copied()
            .collect();

        for id in ids_to_stop {
            if let Some((stop, _)) = self.threads.remove(&id) {
                stop.store(true, Ordering::Relaxed);
            }
        }

        // ── Start threads for new enabled schedules ─────────────────────────
        for config in configs.into_iter().filter(|c| c.enabled) {
            if !self.threads.contains_key(&config.id) {
                self.start_thread(config);
            }
        }

        Ok(())
    }

    /// Stop all running schedule threads.
    pub fn stop_all(&mut self) {
        for (_, (stop, _)) in self.threads.drain() {
            stop.store(true, Ordering::Relaxed);
        }
    }

    fn start_thread(&mut self, config: ScheduleConfig) {
        let repo = Arc::clone(&self.repo);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        let task_type = match config.task_type.as_str() {
            "integrity_check" => TaskType::IntegrityCheck,
            _ => TaskType::Sync,
        };
        let cron_expr = config.cron_expr.clone();
        let interval_secs = config.interval_seconds;

        let handle = thread::spawn(move || loop {
            // ── Determine how long to sleep until next run ──────────────────
            // cron_expr takes priority; interval_seconds is the fallback.
            let sleep_ms: u64 = if let Some(ref expr) = cron_expr {
                next_cron_sleep_ms(expr).unwrap_or_else(|e| {
                    tracing::warn!("Cron parse error, falling back to 1h: {}", e);
                    3_600_000
                })
            } else {
                interval_secs.unwrap_or(3600) as u64 * 1_000
            };

            // ── Sleep in 100 ms steps to respect stop flag promptly ─────────
            let steps = (sleep_ms / 100).max(1);
            for _ in 0..steps {
                if stop_clone.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }

            if stop_clone.load(Ordering::Relaxed) {
                return;
            }

            if let Err(e) = run_task(&task_type, &*repo) {
                tracing::error!("Scheduled task '{}' failed: {}", task_type.as_str(), e);
            }
        });

        self.threads.insert(config.id, (stop, handle));
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
    fn test_run_integrity_task_persists_attention_rows() {
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

        let attention = run_task(&TaskType::IntegrityCheck, &repo).unwrap();
        assert_eq!(attention, 1, "Should persist one integrity attention row");

        let latest = repo
            .get_latest_integrity_run()
            .unwrap()
            .expect("Expected a scheduler-triggered integrity run");
        let (items, _) = repo
            .list_integrity_run_results(latest.id, true, 1, 10)
            .unwrap();
        assert!(!items.is_empty(), "Integrity run should contain issue rows");
        let _ = secondary; // keep alive
    }

    #[test]
    fn test_scheduler_reload_starts_and_stops_threads() {
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

        // Create a schedule with a very short interval (1 second) in the DB
        repo.create_schedule_config("sync", None, Some(1), true)
            .unwrap();

        let repo_arc = Arc::new(repo);
        let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
        scheduler.reload().unwrap();
        assert_eq!(scheduler.threads.len(), 1, "One thread should be running");

        // Give the thread time to fire (interval = 1s)
        std::thread::sleep(std::time::Duration::from_millis(1500));
        scheduler.stop_all();

        assert!(
            secondary.path().join("bg.txt").exists(),
            "File should be mirrored after background task ran"
        );
    }

    #[test]
    fn test_reload_stops_disabled_schedule() {
        let (primary, secondary, repo) = setup();
        let cfg = repo
            .create_schedule_config("sync", None, Some(3600), true)
            .unwrap();

        let repo_arc = Arc::new(repo.clone());
        let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
        scheduler.reload().unwrap();
        assert_eq!(scheduler.threads.len(), 1);

        // Disable the schedule in DB then reload
        repo.update_schedule_config(cfg.id, None, None, Some(false))
            .unwrap();
        scheduler.reload().unwrap();
        assert_eq!(
            scheduler.threads.len(),
            0,
            "Disabled schedule thread should be stopped"
        );
        let _ = secondary;
    }

    #[test]
    fn test_next_cron_sleep_ms_valid_expression() {
        // "0 2 * * *" = every day at 02:00
        let ms = next_cron_sleep_ms("0 2 * * *").unwrap();
        // Must be between 0 and 24 h
        assert!(ms <= 24 * 3600 * 1000);
    }

    #[test]
    fn test_next_cron_sleep_ms_invalid_expression() {
        assert!(next_cron_sleep_ms("not a cron").is_err());
    }
}
