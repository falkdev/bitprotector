use crate::db::repository::Repository;

/// Log an event to the database.
pub fn log_event(
    repo: &Repository,
    event_type: &str,
    tracked_file_id: Option<i64>,
    message: &str,
    details: Option<&str>,
) -> anyhow::Result<()> {
    repo.create_event_log(event_type, tracked_file_id, message, details)?;
    Ok(())
}

/// Log a file tracking event.
pub fn log_file_tracked(repo: &Repository, file_id: i64, path: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "file_created",
        Some(file_id),
        &format!("Tracked: {}", path),
        Some(&format!("{{\"path\":\"{}\"}}", path)),
    )
}

/// Log a file untrack event.
pub fn log_file_untracked(repo: &Repository, file_id: i64, path: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "file_untracked",
        Some(file_id),
        &format!("Untracked: {}", path),
        Some(&format!("{{\"path\":\"{}\"}}", path)),
    )
}

/// Log a successful file mirror event.
pub fn log_file_mirrored(
    repo: &Repository,
    file_id: i64,
    path: &str,
    checksum: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "file_mirrored",
        Some(file_id),
        &format!("Mirrored: {}", path),
        Some(&format!(
            "{{\"path\":\"{}\",\"checksum\":\"{}\"}}",
            path, checksum
        )),
    )
}

/// Log an integrity check that passed.
pub fn log_integrity_pass(repo: &Repository, file_id: i64, path: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "integrity_pass",
        Some(file_id),
        &format!("Integrity OK: {}", path),
        Some(&format!("{{\"path\":\"{}\"}}", path)),
    )
}

/// Log an integrity check that failed.
pub fn log_integrity_fail(
    repo: &Repository,
    file_id: i64,
    path: &str,
    status: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "integrity_fail",
        Some(file_id),
        &format!("Integrity FAIL ({}): {}", status, path),
        Some(&format!(
            "{{\"path\":\"{}\",\"status\":\"{}\"}}",
            path, status
        )),
    )
}

/// Log a both-corrupted event.
pub fn log_both_corrupted(
    repo: &Repository,
    file_id: i64,
    path: &str,
    master_checksum: Option<&str>,
    mirror_checksum: Option<&str>,
    stored_checksum: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "both_corrupted",
        Some(file_id),
        &format!("Both copies corrupted: {}", path),
        Some(&format!(
            "{{\"path\":\"{}\",\"stored_checksum\":\"{}\",\"master_checksum\":{},\"mirror_checksum\":{}}}",
            path,
            stored_checksum,
            master_checksum.map_or("null".to_string(), |c| format!("\"{}\"", c)),
            mirror_checksum.map_or("null".to_string(), |c| format!("\"{}\"", c)),
        )),
    )
}

/// Log a recovery attempt.
pub fn log_recovery(
    repo: &Repository,
    file_id: i64,
    action: &str,
    success: bool,
) -> anyhow::Result<()> {
    let event_type = if success {
        "recovery_success"
    } else {
        "recovery_fail"
    };
    log_event(
        repo,
        event_type,
        Some(file_id),
        &format!(
            "Recovery {}: {}",
            if success { "OK" } else { "FAILED" },
            action
        ),
        Some(&format!(
            "{{\"action\":\"{}\",\"success\":{}}}",
            action, success
        )),
    )
}

/// Log a sync queue item completion.
pub fn log_sync_completed(
    repo: &Repository,
    file_id: i64,
    action: &str,
    path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "sync_completed",
        Some(file_id),
        &format!("Sync completed ({}): {}", action, path),
        Some(&format!(
            "{{\"action\":\"{}\",\"path\":\"{}\"}}",
            action, path
        )),
    )
}

/// Log a sync queue item failure.
pub fn log_sync_failed(
    repo: &Repository,
    file_id: i64,
    action: &str,
    error: &str,
    path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "sync_failed",
        Some(file_id),
        &format!("Sync failed ({}): {}", action, path),
        Some(&format!(
            "{{\"action\":\"{}\",\"path\":\"{}\",\"error\":\"{}\"}}",
            action,
            path,
            error.replace('"', "\\\"")
        )),
    )
}

/// Log a folder tracking event.
pub fn log_folder_tracked(
    repo: &Repository,
    folder_id: i64,
    folder_path: &str,
    drive_pair_id: i64,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "folder_tracked",
        None,
        &format!("Folder tracked: {} (folder #{})", folder_path, folder_id),
        Some(&format!(
            "{{\"folder_id\":{},\"path\":\"{}\",\"drive_pair_id\":{}}}",
            folder_id, folder_path, drive_pair_id
        )),
    )
}

/// Log a folder untrack event.
pub fn log_folder_untracked(
    repo: &Repository,
    folder_id: i64,
    folder_path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "folder_untracked",
        None,
        &format!("Folder untracked: {} (folder #{})", folder_path, folder_id),
        Some(&format!(
            "{{\"folder_id\":{},\"path\":\"{}\"}}",
            folder_id, folder_path
        )),
    )
}

/// Log an integrity run start event.
pub fn log_integrity_run_started(
    repo: &Repository,
    run_id: i64,
    total_files: i64,
    trigger: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "integrity_run_started",
        None,
        &format!(
            "Integrity run #{} started ({} files, trigger: {})",
            run_id, total_files, trigger
        ),
        Some(&format!(
            "{{\"run_id\":{},\"total_files\":{},\"trigger\":\"{}\"}}",
            run_id, total_files, trigger
        )),
    )
}

/// Log an integrity run completion event.
pub fn log_integrity_run_completed(
    repo: &Repository,
    run_id: i64,
    status: &str,
    issues: i64,
    recovered: i64,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "integrity_run_completed",
        None,
        &format!(
            "Integrity run #{} {}: {} issues, {} recovered",
            run_id, status, issues, recovered
        ),
        Some(&format!(
            "{{\"run_id\":{},\"status\":\"{}\",\"issues\":{},\"recovered\":{}}}",
            run_id, status, issues, recovered
        )),
    )
}

/// Log a drive pair creation event.
pub fn log_drive_created(
    repo: &Repository,
    pair_id: i64,
    name: &str,
    primary_path: &str,
    secondary_path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_created",
        None,
        &format!("Drive pair created: {} (#{})", name, pair_id),
        Some(&format!(
            "{{\"pair_id\":{},\"name\":\"{}\",\"primary_path\":\"{}\",\"secondary_path\":\"{}\"}}",
            pair_id, name, primary_path, secondary_path
        )),
    )
}

/// Log a drive pair update event.
pub fn log_drive_updated(repo: &Repository, pair_id: i64, name: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_updated",
        None,
        &format!("Drive pair updated: {} (#{})", name, pair_id),
        Some(&format!(
            "{{\"pair_id\":{},\"name\":\"{}\"}}",
            pair_id, name
        )),
    )
}

/// Log a drive pair deletion event.
pub fn log_drive_deleted(repo: &Repository, pair_id: i64, name: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_deleted",
        None,
        &format!("Drive pair deleted: {} (#{})", name, pair_id),
        Some(&format!(
            "{{\"pair_id\":{},\"name\":\"{}\"}}",
            pair_id, name
        )),
    )
}

/// Log an emergency failover event.
pub fn log_drive_failover(
    repo: &Repository,
    pair_id: i64,
    failed_role: &str,
    new_active_role: &str,
    failed_path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_failover",
        None,
        &format!(
            "Emergency failover for drive pair #{}: {} unavailable, active role now {}",
            pair_id, failed_role, new_active_role
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"failed_role\":\"{}\",\"new_active_role\":\"{}\",\"failed_path\":\"{}\"}}",
            pair_id, failed_role, new_active_role, failed_path
        )),
    )
}

/// Log a drive entering quiescing state.
pub fn log_drive_quiescing(repo: &Repository, pair_id: i64, role: &str) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_quiescing",
        None,
        &format!(
            "Drive pair #{} entered quiescing state for {} replacement",
            pair_id, role
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"role\":\"{}\"}}",
            pair_id, role
        )),
    )
}

/// Log a quiesce cancellation event.
pub fn log_drive_quiesce_cancelled(
    repo: &Repository,
    pair_id: i64,
    role: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_quiesce_cancelled",
        None,
        &format!(
            "Drive pair #{} cancelled {} replacement quiesce",
            pair_id, role
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"role\":\"{}\"}}",
            pair_id, role
        )),
    )
}

/// Log a confirmed drive failure event.
pub fn log_drive_failure_confirmed(
    repo: &Repository,
    pair_id: i64,
    role: &str,
    new_active_role: &str,
    failed_path: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_failure_confirmed",
        None,
        &format!(
            "Drive pair #{} confirmed {} failure; active role now {}",
            pair_id, role, new_active_role
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"role\":\"{}\",\"new_active_role\":\"{}\",\"failed_path\":\"{}\"}}",
            pair_id, role, new_active_role, failed_path
        )),
    )
}

/// Log a replacement drive assignment event.
pub fn log_drive_replacement_assigned(
    repo: &Repository,
    pair_id: i64,
    role: &str,
    new_path: &str,
    queued_items: usize,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_replacement_assigned",
        None,
        &format!(
            "Drive pair #{} assigned replacement {} path, queued {} rebuild item(s)",
            pair_id, role, queued_items
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"role\":\"{}\",\"new_path\":\"{}\",\"queued_items\":{}}}",
            pair_id, role, new_path, queued_items
        )),
    )
}

/// Log a drive rebuild completion event.
pub fn log_drive_rebuild_completed(
    repo: &Repository,
    pair_id: i64,
    role: &str,
    active_role: &str,
) -> anyhow::Result<()> {
    log_event(
        repo,
        "drive_rebuild_completed",
        None,
        &format!(
            "Drive pair #{} finished rebuilding {}; active role is {}",
            pair_id, role, active_role
        ),
        Some(&format!(
            "{{\"pair_id\":{},\"role\":\"{}\",\"active_role\":\"{}\"}}",
            pair_id, role, active_role
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;

    fn setup_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        {
            let conn = pool.get().unwrap();
            initialize_schema(&conn).unwrap();
        }
        Repository::new(pool)
    }

    fn setup_repo_with_file() -> (Repository, i64) {
        let repo = setup_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let file = repo
            .create_tracked_file(pair.id, "foo.txt", "abc123", 100, None)
            .unwrap();
        (repo, file.id)
    }

    #[test]
    fn test_log_event_recorded() {
        let repo = setup_repo();
        log_event(&repo, "integrity_pass", None, "All good", None).unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "integrity_pass");
    }

    #[test]
    fn test_log_event_with_details() {
        let repo = setup_repo();
        log_event(
            &repo,
            "file_mirrored",
            None,
            "Mirrored file",
            Some("checksum=abc"),
        )
        .unwrap();
        let (logs, _) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(logs[0].details, Some("checksum=abc".to_string()));
    }

    #[test]
    fn test_log_filter_by_type() {
        let repo = setup_repo();
        log_event(&repo, "integrity_pass", None, "pass", None).unwrap();
        log_event(&repo, "file_mirrored", None, "mirror", None).unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("integrity_pass"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "integrity_pass");
    }

    #[test]
    fn test_typed_log_functions_record_correct_event_types() {
        let (repo, fid) = setup_repo_with_file();
        log_file_tracked(&repo, fid, "foo.txt").unwrap();
        log_file_mirrored(&repo, fid, "foo.txt", "abc123").unwrap();
        log_integrity_pass(&repo, fid, "foo.txt").unwrap();
        log_integrity_fail(&repo, fid, "foo.txt", "MirrorCorrupted").unwrap();
        log_recovery(&repo, fid, "restore_mirror", true).unwrap();
        log_sync_completed(&repo, fid, "mirror", "foo.txt").unwrap();
        log_sync_failed(&repo, fid, "verify", "checksum mismatch", "foo.txt").unwrap();

        let (all, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 7, "All 7 typed events should be recorded");

        let types: Vec<_> = all.iter().map(|e| e.event_type.as_str()).collect();
        // list_event_logs returns newest first
        let expected = [
            "sync_failed",
            "sync_completed",
            "recovery_success",
            "integrity_fail",
            "integrity_pass",
            "file_mirrored",
            "file_created",
        ];
        for (i, et) in expected.iter().enumerate() {
            assert_eq!(types[i], *et, "Event type mismatch at index {}", i);
        }
    }

    #[test]
    fn test_log_filter_by_file_id() {
        let repo = setup_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let file_a = repo
            .create_tracked_file(pair.id, "a.txt", "aaa", 10, None)
            .unwrap();
        let file_b = repo
            .create_tracked_file(pair.id, "b.txt", "bbb", 10, None)
            .unwrap();

        log_file_tracked(&repo, file_a.id, "a.txt").unwrap();
        log_file_tracked(&repo, file_b.id, "b.txt").unwrap();

        let (logs, total) = repo
            .list_event_logs(None, Some(file_a.id), None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].tracked_file_id, Some(file_a.id));
    }

    #[test]
    fn test_operation_log_retrieval_cycle() {
        // Module test: operation → log creation → log retrieval
        let (repo, fid) = setup_repo_with_file();
        log_file_tracked(&repo, fid, "cycle.txt").unwrap();
        log_integrity_fail(&repo, fid, "cycle.txt", "MirrorMissing").unwrap();
        log_recovery(&repo, fid, "restore_mirror", true).unwrap();

        let (_logs, total) = repo
            .list_event_logs(None, Some(fid), None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 3, "Three events should be recorded for file");
    }

    #[test]
    fn test_log_file_untracked() {
        let (repo, fid) = setup_repo_with_file();
        log_file_untracked(&repo, fid, "foo.txt").unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("file_untracked"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "file_untracked");
        assert!(logs[0].message.contains("foo.txt"));
        assert!(logs[0]
            .details
            .as_ref()
            .unwrap()
            .contains("\"path\":\"foo.txt\""));
    }

    #[test]
    fn test_log_both_corrupted() {
        let (repo, fid) = setup_repo_with_file();
        log_both_corrupted(&repo, fid, "foo.txt", Some("bad1"), Some("bad2"), "good").unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("both_corrupted"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "both_corrupted");
        assert!(logs[0].message.contains("Both copies corrupted"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"stored_checksum\":\"good\""));
        assert!(details.contains("\"master_checksum\":\"bad1\""));
        assert!(details.contains("\"mirror_checksum\":\"bad2\""));
    }

    #[test]
    fn test_log_both_corrupted_with_null_checksums() {
        let (repo, fid) = setup_repo_with_file();
        log_both_corrupted(&repo, fid, "foo.txt", None, None, "good").unwrap();

        let (logs, _) = repo
            .list_event_logs(Some("both_corrupted"), None, None, None, 1, 50)
            .unwrap();
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"master_checksum\":null"));
        assert!(details.contains("\"mirror_checksum\":null"));
    }

    #[test]
    fn test_log_folder_tracked() {
        let repo = setup_repo();
        log_folder_tracked(&repo, 42, "docs/reports", 1).unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("folder_tracked"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert!(logs[0].message.contains("docs/reports"));
        assert!(logs[0].message.contains("folder #42"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"folder_id\":42"));
        assert!(details.contains("\"drive_pair_id\":1"));
    }

    #[test]
    fn test_log_folder_untracked() {
        let repo = setup_repo();
        log_folder_untracked(&repo, 42, "docs/reports").unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("folder_untracked"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert!(logs[0].message.contains("Folder untracked"));
        assert!(logs[0].message.contains("docs/reports"));
    }

    #[test]
    fn test_log_integrity_run_started() {
        let repo = setup_repo();
        log_integrity_run_started(&repo, 5, 100, "scheduled").unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("integrity_run_started"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert!(logs[0].message.contains("run #5"));
        assert!(logs[0].message.contains("100 files"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"run_id\":5"));
        assert!(details.contains("\"total_files\":100"));
        assert!(details.contains("\"trigger\":\"scheduled\""));
    }

    #[test]
    fn test_log_integrity_run_completed() {
        let repo = setup_repo();
        log_integrity_run_completed(&repo, 5, "completed", 3, 2).unwrap();

        let (logs, total) = repo
            .list_event_logs(Some("integrity_run_completed"), None, None, None, 1, 50)
            .unwrap();
        assert_eq!(total, 1);
        assert!(logs[0].message.contains("run #5"));
        assert!(logs[0].message.contains("3 issues"));
        assert!(logs[0].message.contains("2 recovered"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"status\":\"completed\""));
        assert!(details.contains("\"issues\":3"));
        assert!(details.contains("\"recovered\":2"));
    }

    #[test]
    fn test_sync_completed_includes_path_in_message() {
        let (repo, fid) = setup_repo_with_file();
        log_sync_completed(&repo, fid, "mirror", "docs/report.pdf").unwrap();

        let (logs, _) = repo
            .list_event_logs(Some("sync_completed"), None, None, None, 1, 50)
            .unwrap();
        assert!(
            logs[0].message.contains("docs/report.pdf"),
            "Sync completed message should contain file path"
        );
        assert!(logs[0].message.contains("mirror"));
    }

    #[test]
    fn test_sync_failed_includes_path_and_error() {
        let (repo, fid) = setup_repo_with_file();
        log_sync_failed(&repo, fid, "mirror", "disk full", "docs/report.pdf").unwrap();

        let (logs, _) = repo
            .list_event_logs(Some("sync_failed"), None, None, None, 1, 50)
            .unwrap();
        assert!(logs[0].message.contains("docs/report.pdf"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"error\":\"disk full\""));
        assert!(details.contains("\"path\":\"docs/report.pdf\""));
    }

    #[test]
    fn test_file_path_populated_from_tracked_file_join() {
        let (repo, fid) = setup_repo_with_file();
        log_file_tracked(&repo, fid, "foo.txt").unwrap();

        let (logs, _) = repo
            .list_event_logs(None, Some(fid), None, None, 1, 50)
            .unwrap();
        assert_eq!(
            logs[0].file_path.as_deref(),
            Some("/tmp/p/foo.txt"),
            "file_path should be the full absolute path via LEFT JOIN with tracked_files and drive_pairs"
        );
    }

    #[test]
    fn test_file_path_null_for_system_events() {
        let repo = setup_repo();
        log_integrity_run_started(&repo, 1, 10, "manual").unwrap();

        let (logs, _) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(logs[0].tracked_file_id, None);
        assert_eq!(logs[0].file_path, None);
    }

    #[test]
    fn test_details_contain_json_for_enriched_functions() {
        let (repo, fid) = setup_repo_with_file();
        log_file_tracked(&repo, fid, "foo.txt").unwrap();
        log_integrity_fail(&repo, fid, "foo.txt", "mirror_corrupted").unwrap();
        log_recovery(&repo, fid, "restore_mirror", true).unwrap();

        let (logs, _) = repo
            .list_event_logs(None, Some(fid), None, None, 1, 50)
            .unwrap();
        // All enriched functions should now have non-null JSON details
        for log in &logs {
            assert!(
                log.details.is_some(),
                "Event {} should have details",
                log.event_type
            );
            let details = log.details.as_ref().unwrap();
            assert!(
                details.starts_with('{') && details.ends_with('}'),
                "Details for {} should be JSON: {}",
                log.event_type,
                details
            );
        }
    }

    #[test]
    fn test_log_drive_created() {
        let repo = setup_repo();
        log_drive_created(&repo, 1, "MyPair", "/mnt/a", "/mnt/b").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_created");
        assert!(logs[0].message.contains("MyPair"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"pair_id\":1"));
        assert!(details.contains("/mnt/a"));
        assert!(details.contains("/mnt/b"));
    }

    #[test]
    fn test_log_drive_updated() {
        let repo = setup_repo();
        log_drive_updated(&repo, 2, "Renamed").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_updated");
        assert!(logs[0].message.contains("Renamed"));
    }

    #[test]
    fn test_log_drive_deleted() {
        let repo = setup_repo();
        log_drive_deleted(&repo, 3, "OldPair").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_deleted");
        assert!(logs[0].message.contains("OldPair"));
    }

    #[test]
    fn test_log_drive_failover() {
        let repo = setup_repo();
        log_drive_failover(&repo, 1, "primary", "secondary", "/mnt/failed").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_failover");
        assert!(logs[0].message.contains("primary"));
        assert!(logs[0].message.contains("secondary"));
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"failed_path\":\"/mnt/failed\""));
    }

    #[test]
    fn test_log_drive_quiescing() {
        let repo = setup_repo();
        log_drive_quiescing(&repo, 1, "secondary").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_quiescing");
        assert!(logs[0].message.contains("secondary"));
    }

    #[test]
    fn test_log_drive_quiesce_cancelled() {
        let repo = setup_repo();
        log_drive_quiesce_cancelled(&repo, 1, "primary").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_quiesce_cancelled");
    }

    #[test]
    fn test_log_drive_failure_confirmed() {
        let repo = setup_repo();
        log_drive_failure_confirmed(&repo, 1, "primary", "secondary", "/mnt/bad").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_failure_confirmed");
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"new_active_role\":\"secondary\""));
    }

    #[test]
    fn test_log_drive_replacement_assigned() {
        let repo = setup_repo();
        log_drive_replacement_assigned(&repo, 1, "secondary", "/mnt/new", 5).unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_replacement_assigned");
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"new_path\":\"/mnt/new\""));
        assert!(details.contains("\"queued_items\":5"));
    }

    #[test]
    fn test_log_drive_rebuild_completed() {
        let repo = setup_repo();
        log_drive_rebuild_completed(&repo, 1, "secondary", "primary").unwrap();
        let (logs, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 1);
        assert_eq!(logs[0].event_type, "drive_rebuild_completed");
        let details = logs[0].details.as_ref().unwrap();
        assert!(details.contains("\"role\":\"secondary\""));
        assert!(details.contains("\"active_role\":\"primary\""));
    }
}
