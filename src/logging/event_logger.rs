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
pub fn log_file_tracked(repo: &Repository, file_id: i64, relative_path: &str) -> anyhow::Result<()> {
    log_event(repo, "file_created", Some(file_id), &format!("Tracked: {}", relative_path), None)
}

/// Log a successful file mirror event.
pub fn log_file_mirrored(repo: &Repository, file_id: i64, relative_path: &str, checksum: &str) -> anyhow::Result<()> {
    log_event(repo, "file_mirrored", Some(file_id), &format!("Mirrored: {}", relative_path), Some(checksum))
}

/// Log an integrity check that passed.
pub fn log_integrity_pass(repo: &Repository, file_id: i64, relative_path: &str) -> anyhow::Result<()> {
    log_event(repo, "integrity_pass", Some(file_id), &format!("Integrity OK: {}", relative_path), None)
}

/// Log an integrity check that failed.
pub fn log_integrity_fail(repo: &Repository, file_id: i64, relative_path: &str, status: &str) -> anyhow::Result<()> {
    log_event(repo, "integrity_fail", Some(file_id), &format!("Integrity FAIL: {}", relative_path), Some(status))
}

/// Log a recovery attempt.
pub fn log_recovery(repo: &Repository, file_id: i64, action: &str, success: bool) -> anyhow::Result<()> {
    let event_type = if success { "recovery_success" } else { "recovery_fail" };
    log_event(repo, event_type, Some(file_id), &format!("Recovery {}: {}", if success { "OK" } else { "FAILED" }, action), None)
}

/// Log a sync queue item completion.
pub fn log_sync_completed(repo: &Repository, file_id: i64, action: &str) -> anyhow::Result<()> {
    log_event(repo, "sync_completed", Some(file_id), &format!("Sync action completed: {}", action), None)
}

/// Log a sync queue item failure.
pub fn log_sync_failed(repo: &Repository, file_id: i64, action: &str, error: &str) -> anyhow::Result<()> {
    log_event(repo, "sync_failed", Some(file_id), &format!("Sync action failed: {}", action), Some(error))
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
        let file = repo.create_tracked_file(pair.id, "foo.txt", "abc123", 100, None).unwrap();
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
        log_event(&repo, "file_mirrored", None, "Mirrored file", Some("checksum=abc")).unwrap();
        let (logs, _) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(logs[0].details, Some("checksum=abc".to_string()));
    }

    #[test]
    fn test_log_filter_by_type() {
        let repo = setup_repo();
        log_event(&repo, "integrity_pass", None, "pass", None).unwrap();
        log_event(&repo, "file_mirrored", None, "mirror", None).unwrap();

        let (logs, total) = repo.list_event_logs(Some("integrity_pass"), None, None, None, 1, 50).unwrap();
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
        log_sync_completed(&repo, fid, "mirror").unwrap();
        log_sync_failed(&repo, fid, "verify", "checksum mismatch").unwrap();

        let (all, total) = repo.list_event_logs(None, None, None, None, 1, 50).unwrap();
        assert_eq!(total, 7, "All 7 typed events should be recorded");

        let types: Vec<_> = all.iter().map(|e| e.event_type.as_str()).collect();
        // list_event_logs returns newest first
        let expected = ["sync_failed", "sync_completed", "recovery_success",
                        "integrity_fail", "integrity_pass", "file_mirrored", "file_created"];
        for (i, et) in expected.iter().enumerate() {
            assert_eq!(types[i], *et, "Event type mismatch at index {}", i);
        }
    }

    #[test]
    fn test_log_filter_by_file_id() {
        let repo = setup_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let file_a = repo.create_tracked_file(pair.id, "a.txt", "aaa", 10, None).unwrap();
        let file_b = repo.create_tracked_file(pair.id, "b.txt", "bbb", 10, None).unwrap();

        log_file_tracked(&repo, file_a.id, "a.txt").unwrap();
        log_file_tracked(&repo, file_b.id, "b.txt").unwrap();

        let (logs, total) = repo.list_event_logs(None, Some(file_a.id), None, None, 1, 50).unwrap();
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

        let (logs, total) = repo.list_event_logs(None, Some(fid), None, None, 1, 50).unwrap();
        assert_eq!(total, 3, "Three events should be recorded for file");
    }
}
