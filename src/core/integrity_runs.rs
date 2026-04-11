use crate::core::drive;
use crate::core::integrity::{self, IntegrityStatus};
use crate::db::repository::{IntegrityRun, Repository};

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
) -> anyhow::Result<IntegrityRun> {
    if repo.get_active_integrity_run()?.is_some() {
        anyhow::bail!("Another integrity run is already active");
    }

    let total_files = repo.count_tracked_files(scope_drive_pair_id)?;
    let run = repo.create_integrity_run(scope_drive_pair_id, recover, trigger, total_files)?;
    let run_id = run.id;
    let repo_clone = repo.clone();

    std::thread::spawn(move || {
        if let Err(error) = process_run(&repo_clone, run_id) {
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
) -> anyhow::Result<IntegrityRun> {
    if repo.get_active_integrity_run()?.is_some() {
        anyhow::bail!("Another integrity run is already active");
    }
    let total_files = repo.count_tracked_files(scope_drive_pair_id)?;
    let run = repo.create_integrity_run(scope_drive_pair_id, recover, trigger, total_files)?;
    process_run(repo, run.id)?;
    repo.get_integrity_run(run.id)
}

pub fn process_run(repo: &Repository, run_id: i64) -> anyhow::Result<()> {
    let run = repo.get_integrity_run(run_id)?;
    let recover = run.recover;
    let scope_drive_pair_id = run.scope_drive_pair_id;
    let mut page = 1i64;
    let per_page = 100i64;

    loop {
        let (files, total) =
            repo.list_tracked_files(scope_drive_pair_id, None, None, page, per_page)?;
        if files.is_empty() {
            break;
        }

        for file in &files {
            let current = repo.get_integrity_run(run_id)?;
            if current.stop_requested {
                repo.finish_integrity_run(run_id, RUN_STATUS_STOPPED, None)?;
                return Ok(());
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
    if final_state.stop_requested {
        repo.finish_integrity_run(run_id, RUN_STATUS_STOPPED, None)?;
    } else {
        repo.finish_integrity_run(run_id, RUN_STATUS_COMPLETED, None)?;
    }
    Ok(())
}
