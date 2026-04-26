use crate::core::drive;
use crate::core::integrity::{self, IntegrityStatus};
use crate::db::repository::{IntegrityRun, Repository};
use crate::logging::event_logger;

pub const RUN_STATUS_RUNNING: &str = "running";
pub const RUN_STATUS_STOPPING: &str = "stopping";
pub const RUN_STATUS_STOPPED: &str = "stopped";
pub const RUN_STATUS_COMPLETED: &str = "completed";
pub const RUN_STATUS_FAILED: &str = "failed";

pub fn status_str(status: &IntegrityStatus) -> &'static str {
    match status {
        IntegrityStatus::Ok => "ok",
        IntegrityStatus::MasterCorrupted => "master_corrupted",
        IntegrityStatus::MirrorCorrupted => "mirror_corrupted",
        IntegrityStatus::BothCorrupted => "both_corrupted",
        IntegrityStatus::MirrorMissing => "mirror_missing",
        IntegrityStatus::MasterMissing => "master_missing",
        IntegrityStatus::PrimaryDriveUnavailable => "primary_drive_unavailable",
        IntegrityStatus::SecondaryDriveUnavailable => "secondary_drive_unavailable",
    }
}

pub fn start_run_async(
    repo: &Repository,
    scope_drive_pair_id: Option<i64>,
    recover: bool,
    trigger: &str,
    deadline: Option<std::time::Instant>,
) -> anyhow::Result<IntegrityRun> {
    if repo.get_active_integrity_run()?.is_some() {
        anyhow::bail!("Another integrity run is already active");
    }

    let total_files = repo.count_tracked_files(scope_drive_pair_id)?;
    let run = repo.create_integrity_run(scope_drive_pair_id, recover, trigger, total_files)?;
    let run_id = run.id;
    let repo_clone = repo.clone();

    std::thread::spawn(move || {
        if let Err(error) = process_run(&repo_clone, run_id, deadline) {
            let message = error.to_string();
            let _ = repo_clone.finish_integrity_run(run_id, RUN_STATUS_FAILED, Some(&message));
        }
    });

    Ok(run)
}

pub fn run_sync(
    repo: &Repository,
    scope_drive_pair_id: Option<i64>,
    recover: bool,
    trigger: &str,
    deadline: Option<std::time::Instant>,
) -> anyhow::Result<IntegrityRun> {
    if repo.get_active_integrity_run()?.is_some() {
        anyhow::bail!("Another integrity run is already active");
    }
    let total_files = repo.count_tracked_files(scope_drive_pair_id)?;
    let run = repo.create_integrity_run(scope_drive_pair_id, recover, trigger, total_files)?;
    process_run(repo, run.id, deadline)?;
    repo.get_integrity_run(run.id)
}

pub fn process_run(repo: &Repository, run_id: i64, deadline: Option<std::time::Instant>) -> anyhow::Result<()> {
    let run = repo.get_integrity_run(run_id)?;
    let recover = run.recover;
    let scope_drive_pair_id = run.scope_drive_pair_id;
    let mut page = 1i64;
    let per_page = 100i64;

    let _ = event_logger::log_integrity_run_started(repo, run_id, run.total_files, &run.trigger);

    loop {
        let (files, total) =
            repo.list_tracked_files_oldest_integrity_first(scope_drive_pair_id, page, per_page)?;
        if files.is_empty() {
            break;
        }

        for file in &files {
            let current = repo.get_integrity_run(run_id)?;
            if current.stop_requested {
                repo.finish_integrity_run(run_id, RUN_STATUS_STOPPED, None)?;
                return Ok(());
            }

            // Honour scheduler time window: stop gracefully at the deadline.
            if let Some(dl) = deadline {
                if std::time::Instant::now() >= dl {
                    repo.finish_integrity_run(run_id, RUN_STATUS_STOPPED, None)?;
                    return Ok(());
                }
            }

            let pair = match drive::load_operational_pair(repo, file.drive_pair_id) {
                Ok(pair) => pair,
                Err(_) => {
                    repo.update_tracked_file_last_integrity_check_at(file.id)?;
                    repo.append_integrity_run_result(
                        run_id,
                        file.id,
                        file.drive_pair_id,
                        &file.relative_path,
                        "internal_error",
                        false,
                        true,
                    )?;
                    repo.increment_integrity_run_progress(run_id, 1, 0)?;
                    continue;
                }
            };

            let result = match integrity::check_file_integrity(&pair, file) {
                Ok(result) => result,
                Err(_) => {
                    repo.update_tracked_file_last_integrity_check_at(file.id)?;
                    repo.append_integrity_run_result(
                        run_id,
                        file.id,
                        file.drive_pair_id,
                        &file.relative_path,
                        "internal_error",
                        false,
                        true,
                    )?;
                    repo.increment_integrity_run_progress(run_id, 1, 0)?;
                    continue;
                }
            };

            let mut recovered = false;
            if recover && result.status != IntegrityStatus::Ok {
                recovered =
                    integrity::attempt_recovery_with_reconciliation(repo, &pair, file, &result)
                        .unwrap_or(false);
            }

            // Log per-file integrity result to event log.
            let full_path = format!("{}/{}", pair.primary_path, file.relative_path);
            match &result.status {
                IntegrityStatus::Ok => {}
                IntegrityStatus::BothCorrupted => {
                    let _ = event_logger::log_both_corrupted(
                        repo,
                        file.id,
                        &full_path,
                        result.master_checksum.as_deref(),
                        result.mirror_checksum.as_deref(),
                        &result.stored_checksum,
                    );
                    let _ = event_logger::log_integrity_fail(
                        repo,
                        file.id,
                        &full_path,
                        status_str(&result.status),
                    );
                }
                _ => {
                    let _ = event_logger::log_integrity_fail(
                        repo,
                        file.id,
                        &full_path,
                        status_str(&result.status),
                    );
                }
            }

            let needs_attention = result.status != IntegrityStatus::Ok && !recovered;
            repo.update_tracked_file_last_integrity_check_at(file.id)?;
            repo.append_integrity_run_result(
                run_id,
                file.id,
                file.drive_pair_id,
                &file.relative_path,
                status_str(&result.status),
                recovered,
                needs_attention,
            )?;
            repo.increment_integrity_run_progress(
                run_id,
                if needs_attention { 1 } else { 0 },
                if recovered { 1 } else { 0 },
            )?;
        }

        if (page * per_page) >= total {
            break;
        }
        page += 1;
    }

    let final_state = repo.get_integrity_run(run_id)?;
    let final_status = if final_state.stop_requested {
        RUN_STATUS_STOPPED
    } else {
        RUN_STATUS_COMPLETED
    };
    repo.finish_integrity_run(run_id, final_status, None)?;
    let _ = event_logger::log_integrity_run_completed(
        repo,
        run_id,
        final_status,
        final_state.attention_files,
        final_state.recovered_files,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracker;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn setup_pair(repo: &Repository) -> (TempDir, TempDir, i64) {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "integrity-runs",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        (primary, secondary, pair.id)
    }

    fn create_tracked_file(
        repo: &Repository,
        pair_id: i64,
        primary: &TempDir,
        secondary: &TempDir,
        name: &str,
        mirrored: bool,
    ) -> i64 {
        let content = format!("content-{name}");
        fs::write(primary.path().join(name), content.as_bytes()).unwrap();
        if mirrored {
            fs::write(secondary.path().join(name), content.as_bytes()).unwrap();
        }
        let pair = repo.get_drive_pair(pair_id).unwrap();
        let tracked = tracker::track_file(repo, &pair, name, None).unwrap();
        if mirrored {
            repo.update_tracked_file_mirror_status(tracked.id, true)
                .unwrap();
        }
        tracked.id
    }

    #[test]
    fn status_str_returns_expected_labels() {
        assert_eq!(status_str(&IntegrityStatus::Ok), "ok");
        assert_eq!(
            status_str(&IntegrityStatus::MasterCorrupted),
            "master_corrupted"
        );
        assert_eq!(
            status_str(&IntegrityStatus::MirrorCorrupted),
            "mirror_corrupted"
        );
        assert_eq!(
            status_str(&IntegrityStatus::BothCorrupted),
            "both_corrupted"
        );
        assert_eq!(
            status_str(&IntegrityStatus::MirrorMissing),
            "mirror_missing"
        );
        assert_eq!(
            status_str(&IntegrityStatus::MasterMissing),
            "master_missing"
        );
        assert_eq!(
            status_str(&IntegrityStatus::PrimaryDriveUnavailable),
            "primary_drive_unavailable"
        );
        assert_eq!(
            status_str(&IntegrityStatus::SecondaryDriveUnavailable),
            "secondary_drive_unavailable"
        );
    }

    #[test]
    fn start_run_async_persists_run_row() {
        let repo = make_repo();
        let (primary, secondary, pair_id) = setup_pair(&repo);
        create_tracked_file(&repo, pair_id, &primary, &secondary, "a.txt", true);

        let run = start_run_async(&repo, Some(pair_id), false, "test", None).unwrap();
        assert_eq!(run.status, RUN_STATUS_RUNNING);
        assert_eq!(run.total_files, 1);

        let mut final_state = repo.get_integrity_run(run.id).unwrap();
        for _ in 0..100 {
            final_state = repo.get_integrity_run(run.id).unwrap();
            if final_state.status != RUN_STATUS_RUNNING && final_state.status != RUN_STATUS_STOPPING
            {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_ne!(final_state.status, RUN_STATUS_RUNNING);
        assert!(repo.get_latest_integrity_run().unwrap().is_some());
    }

    #[test]
    fn run_sync_processes_files_and_persists_results() {
        let repo = make_repo();
        let (primary, secondary, pair_id) = setup_pair(&repo);
        create_tracked_file(&repo, pair_id, &primary, &secondary, "ok.txt", true);
        create_tracked_file(&repo, pair_id, &primary, &secondary, "missing.txt", false);

        let run = run_sync(&repo, Some(pair_id), false, "test", None).unwrap();
        assert_eq!(run.status, RUN_STATUS_COMPLETED);
        assert_eq!(run.total_files, 2);
        assert_eq!(run.processed_files, 2);

        let (results, total) = repo
            .list_integrity_run_results(run.id, false, 1, 50)
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|result| result.status == "ok"));
    }

    #[test]
    fn process_run_honors_stop_request_and_marks_stopped() {
        let repo = make_repo();
        let (primary, secondary, pair_id) = setup_pair(&repo);
        for i in 0..20 {
            create_tracked_file(
                &repo,
                pair_id,
                &primary,
                &secondary,
                &format!("f{i}.txt"),
                true,
            );
        }

        let total = repo.count_tracked_files(Some(pair_id)).unwrap();
        let run = repo
            .create_integrity_run(Some(pair_id), false, "test", total)
            .unwrap();
        repo.request_integrity_run_stop(run.id).unwrap();

        process_run(&repo, run.id, None).unwrap();
        let stopped = repo.get_integrity_run(run.id).unwrap();
        assert_eq!(stopped.status, RUN_STATUS_STOPPED);
        assert!(stopped.stop_requested);
    }

    #[test]
    fn run_sync_stops_at_deadline() {
        use std::time::{Duration, Instant};

        let repo = make_repo();
        let (primary, secondary, pair_id) = setup_pair(&repo);
        // Create several files to check
        for i in 0..10 {
            create_tracked_file(
                &repo,
                pair_id,
                &primary,
                &secondary,
                &format!("dl{}.txt", i),
                true,
            );
        }

        // Deadline already in the past — the run should stop immediately
        let past_deadline = Instant::now() - Duration::from_secs(1);
        let run = run_sync(&repo, Some(pair_id), false, "test", Some(past_deadline)).unwrap();

        // The run must be stopped, not completed, since the deadline has passed
        assert_eq!(
            run.status, RUN_STATUS_STOPPED,
            "Run should be STOPPED when deadline is in the past"
        );
    }
}
