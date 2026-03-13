use crate::db::repository::Repository;
use crate::db::schema::initialize_schema;

/// Summary of system health for SSH MOTD display.
#[derive(Debug, Default)]
pub struct StatusReport {
    pub drive_pairs: usize,
    pub tracked_files: usize,
    pub pending_sync: usize,
    pub recent_integrity_failures: usize,
    pub last_backup: Option<String>,
}

/// Gather system status from the repository (fast DB queries only).
pub fn gather_status(repo: &Repository) -> anyhow::Result<StatusReport> {
    let drive_pairs = repo.list_drive_pairs()?.len();

    // Use pagination count return value to get total tracked files
    let (_, total_files) = repo.list_tracked_files(None, None, None, 1, 1)?;
    let tracked_files = total_files as usize;

    // Count pending sync queue items
    let (_, total_pending) = repo.list_sync_queue(Some("pending"), 1, 1)?;
    let pending_sync = total_pending as usize;

    // Count integrity failures in the last 24 hours
    let since = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::hours(24))
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();
    let (_, total_failures) = repo.list_event_logs(
        Some("integrity_fail"),
        None,
        Some(&since),
        None,
        1,
        1,
    )?;
    let recent_integrity_failures = total_failures as usize;

    // Get most recent successful backup time
    let backup_configs = repo.list_db_backup_configs()?;
    let last_backup = backup_configs
        .iter()
        .filter_map(|c| c.last_backup.as_ref())
        .max()
        .cloned();

    Ok(StatusReport {
        drive_pairs,
        tracked_files,
        pending_sync,
        recent_integrity_failures,
        last_backup,
    })
}

/// Format a status report for terminal display.
pub fn format_status(report: &StatusReport) -> String {
    let mut lines = Vec::new();
    lines.push("┌─ BitProtector Status ─────────────────────────┐".to_string());
    lines.push(format!(
        "│  Drives: {}   Files: {}",
        report.drive_pairs, report.tracked_files,
    ));

    if report.pending_sync > 0 {
        lines.push(format!("│  ⚠  {} file(s) pending sync", report.pending_sync));
    } else {
        lines.push("│  ✓  Sync queue empty".to_string());
    }

    if report.recent_integrity_failures > 0 {
        lines.push(format!(
            "│  ✗  {} integrity failure(s) in last 24h",
            report.recent_integrity_failures
        ));
    } else {
        lines.push("│  ✓  No integrity failures (24h)".to_string());
    }

    match &report.last_backup {
        Some(ts) => lines.push(format!("│  Last backup: {}", ts)),
        None => lines.push("│  No backups configured".to_string()),
    }

    lines.push("└───────────────────────────────────────────────┘".to_string());
    lines.join("\n")
}

/// Print a brief system health summary suitable for SSH MOTD.
pub fn print_status(db_path: &str) {
    let pool = match crate::db::repository::create_pool(db_path) {
        Ok(p) => p,
        Err(e) => {
            println!("BitProtector: unable to open database: {}", e);
            return;
        }
    };

    if let Ok(conn) = pool.get() {
        if initialize_schema(&*conn).is_err() {
            println!("BitProtector: database error");
            return;
        }
    }

    let repo = Repository::new(pool);
    match gather_status(&repo) {
        Ok(report) => println!("{}", format_status(&report)),
        Err(e) => println!("BitProtector: error gathering status: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    #[test]
    fn test_status_empty_system() {
        let repo = make_repo();
        let report = gather_status(&repo).unwrap();
        assert_eq!(report.drive_pairs, 0);
        assert_eq!(report.tracked_files, 0);
        assert_eq!(report.pending_sync, 0);
        assert_eq!(report.recent_integrity_failures, 0);
        assert!(report.last_backup.is_none());
    }

    #[test]
    fn test_format_status_all_ok() {
        let report = StatusReport {
            drive_pairs: 2,
            tracked_files: 50,
            pending_sync: 0,
            recent_integrity_failures: 0,
            last_backup: Some("2024-01-15 10:00:00".to_string()),
        };
        let output = format_status(&report);
        assert!(output.contains("Drives: 2"));
        assert!(output.contains("Files: 50"));
        assert!(output.contains("✓  Sync queue empty"));
        assert!(output.contains("✓  No integrity failures"));
        assert!(output.contains("2024-01-15"));
    }

    #[test]
    fn test_format_status_with_issues() {
        let report = StatusReport {
            drive_pairs: 1,
            tracked_files: 10,
            pending_sync: 3,
            recent_integrity_failures: 2,
            last_backup: None,
        };
        let output = format_status(&report);
        assert!(output.contains("⚠  3 file(s) pending sync"));
        assert!(output.contains("✗  2 integrity failure(s)"));
        assert!(output.contains("No backups configured"));
    }

    #[test]
    fn test_status_with_data() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        repo.create_tracked_file(pair.id, "/p/a.txt", "abc", 100, None).unwrap();
        repo.create_tracked_file(pair.id, "/p/b.txt", "def", 200, None).unwrap();

        let report = gather_status(&repo).unwrap();
        assert_eq!(report.drive_pairs, 1);
        assert_eq!(report.tracked_files, 2);
        assert_eq!(report.pending_sync, 0);
    }

    #[test]
    fn test_status_with_pending_sync() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let file = repo.create_tracked_file(pair.id, "a.txt", "abc", 100, None).unwrap();
        repo.create_sync_queue_item(file.id, "mirror").unwrap();

        let report = gather_status(&repo).unwrap();
        assert_eq!(report.pending_sync, 1);
    }

    #[test]
    fn test_status_with_integrity_failure() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let file = repo.create_tracked_file(pair.id, "a.txt", "abc", 100, None).unwrap();
        crate::logging::event_logger::log_integrity_fail(&repo, file.id, "a.txt", "MirrorCorrupted").unwrap();

        let report = gather_status(&repo).unwrap();
        assert_eq!(report.recent_integrity_failures, 1);
    }
}
