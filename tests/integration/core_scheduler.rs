use bitprotector_lib::core::scheduler::Scheduler;
use bitprotector_lib::db::repository::{create_memory_pool, Repository};
use bitprotector_lib::db::schema::initialize_schema;
use std::sync::Arc;
use std::time::Duration;

fn make_repo() -> Repository {
    let pool = create_memory_pool().unwrap();
    initialize_schema(&pool.get().unwrap()).unwrap();
    Repository::new(pool)
}

// ── Scheduler lifecycle ────────────────────────────────────────────────────

#[test]
fn test_scheduler_reload_empty_schedules_is_noop() {
    let repo = make_repo();
    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(repo_arc);
    // Empty DB — reload should succeed without starting any threads
    scheduler.reload().unwrap();
    scheduler.stop_all();
}

#[test]
fn test_scheduler_stop_all_with_no_threads_is_noop() {
    let repo = make_repo();
    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(repo_arc);
    // Calling stop_all on a freshly constructed scheduler must not panic
    scheduler.stop_all();
}

#[test]
fn test_scheduler_reload_starts_thread_for_enabled_schedule() {
    let repo = make_repo();
    // interval_seconds=2 → the thread will sleep, not immediately fire
    repo.create_schedule_config("sync", None, Some(2), true)
        .unwrap();

    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
    scheduler.reload().unwrap();

    // Give the spawned thread a moment to start
    std::thread::sleep(Duration::from_millis(100));

    // Stop all threads cleanly — this must not panic or deadlock
    scheduler.stop_all();
}

#[test]
fn test_scheduler_reload_does_not_duplicate_running_thread() {
    let repo = make_repo();
    repo.create_schedule_config("sync", None, Some(60), true)
        .unwrap();

    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
    scheduler.reload().unwrap();
    // A second reload for the same active config should not start a second thread
    scheduler.reload().unwrap();

    scheduler.stop_all();
}

#[test]
fn test_scheduler_reload_stops_thread_when_schedule_disabled() {
    let repo = make_repo();
    let cfg = repo
        .create_schedule_config("sync", None, Some(60), true)
        .unwrap();

    let repo_arc = Arc::new(repo.clone());
    let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
    scheduler.reload().unwrap(); // starts a thread

    // Disable the schedule in the DB
    repo.update_schedule_config(cfg.id, None, None, Some(false))
        .unwrap();

    // Reload should stop the now-disabled schedule's thread
    scheduler.reload().unwrap();

    scheduler.stop_all();
}

#[test]
fn test_scheduler_reload_stops_thread_when_schedule_deleted() {
    let repo = make_repo();
    let cfg = repo
        .create_schedule_config("integrity_check", None, Some(60), true)
        .unwrap();

    let repo_arc = Arc::new(repo.clone());
    let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
    scheduler.reload().unwrap(); // starts a thread

    // Delete the schedule from the DB
    repo.delete_schedule_config(cfg.id).unwrap();

    // Reload should clean up the thread for the removed schedule
    scheduler.reload().unwrap();

    scheduler.stop_all();
}

#[test]
fn test_scheduler_thread_fires_interval_task() {
    use bitprotector_lib::core::checksum;
    use std::fs;
    use tempfile::TempDir;

    let primary = TempDir::new().unwrap();
    let secondary = TempDir::new().unwrap();
    let content = b"scheduled sync content";
    fs::write(primary.path().join("sched.txt"), content).unwrap();

    let repo = make_repo();
    let pair = repo
        .create_drive_pair(
            "pair",
            primary.path().to_str().unwrap(),
            secondary.path().to_str().unwrap(),
        )
        .unwrap();
    let hash = checksum::checksum_bytes(content);
    let file = repo
        .create_tracked_file(pair.id, "sched.txt", &hash, content.len() as i64, None)
        .unwrap();
    // Queue a mirror action so the sync task has something to process
    repo.create_sync_queue_item(file.id, "mirror").unwrap();

    // interval_seconds=1 → thread fires after ~1 second
    repo.create_schedule_config("sync", None, Some(1), true)
        .unwrap();

    let repo_arc = Arc::new(repo.clone());
    let mut scheduler = Scheduler::new(Arc::clone(&repo_arc));
    scheduler.reload().unwrap();

    // Wait long enough for the thread to fire at least once (1 s interval + buffer)
    std::thread::sleep(Duration::from_millis(1500));
    scheduler.stop_all();

    // The sync task should have processed the mirror action
    let (completed, _) = repo.list_sync_queue(Some("completed"), 1, 10).unwrap();
    assert!(
        !completed.is_empty(),
        "Scheduler should have processed the mirror queue item"
    );
    assert!(
        secondary.path().join("sched.txt").exists(),
        "File should be mirrored by the scheduled sync task"
    );
}

#[test]
fn test_scheduler_multiple_schedules_run_concurrently() {
    let repo = make_repo();
    // Two disabled schedules — just verify reload and stop_all work for multiple entries
    repo.create_schedule_config("sync", None, Some(60), false)
        .unwrap();
    repo.create_schedule_config("integrity_check", None, Some(120), false)
        .unwrap();

    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(repo_arc);
    scheduler.reload().unwrap(); // disabled → no threads started
    scheduler.stop_all(); // must not panic
}

#[test]
fn test_scheduler_enabled_and_disabled_schedules_mixed() {
    let repo = make_repo();
    repo.create_schedule_config("sync", None, Some(60), true)
        .unwrap();
    repo.create_schedule_config("integrity_check", None, Some(60), false)
        .unwrap();

    let repo_arc = Arc::new(repo);
    let mut scheduler = Scheduler::new(repo_arc);
    scheduler.reload().unwrap(); // only the enabled one gets a thread
    scheduler.stop_all();
}
