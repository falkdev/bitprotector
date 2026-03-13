use clap::{Args, Subcommand};
use crate::db::repository::Repository;

#[derive(Subcommand, Debug)]
pub enum LogsCommand {
    /// List event log entries (with optional filters)
    List(ListArgs),
    /// Show details of a specific log entry
    Show { id: i64 },
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by event type (e.g. integrity_pass, file_mirrored)
    #[arg(long)]
    pub event_type: Option<String>,
    /// Filter by tracked file ID
    #[arg(long)]
    pub file_id: Option<i64>,
    /// Filter from date (ISO format: YYYY-MM-DD HH:MM:SS)
    #[arg(long)]
    pub from: Option<String>,
    /// Filter to date (ISO format: YYYY-MM-DD HH:MM:SS)
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: i64,
    #[arg(long, default_value_t = 50)]
    pub per_page: i64,
}

pub fn handle(cmd: LogsCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        LogsCommand::List(args) => {
            let (entries, total) = repo.list_event_logs(
                args.event_type.as_deref(),
                args.file_id,
                args.from.as_deref(),
                args.to.as_deref(),
                args.page,
                args.per_page,
            )?;
            println!("{:<6} {:<22} {:<8} {:<40}", "ID", "Event Type", "File", "Message");
            println!("{}", "-".repeat(80));
            for e in &entries {
                println!("{:<6} {:<22} {:<8} {}",
                    e.id,
                    e.event_type,
                    e.tracked_file_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()),
                    e.message,
                );
            }
            println!("Total: {}", total);
        }
        LogsCommand::Show { id } => {
            let e = repo.get_event_log(id)?;
            println!("Log Entry #{}", e.id);
            println!("  Type:    {}", e.event_type);
            println!("  File:    {}", e.tracked_file_id.map(|id| format!("#{}", id)).unwrap_or_else(|| "(none)".to_string()));
            println!("  Message: {}", e.message);
            println!("  Created: {}", e.created_at);
            if let Some(details) = &e.details {
                println!("  Details: {}", details);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use crate::logging::event_logger;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn make_repo_with_files() -> (Repository, i64, i64) {
        let repo = make_repo();
        let pair = repo.create_drive_pair("test", "/tmp/p", "/tmp/s").unwrap();
        let f1 = repo.create_tracked_file(pair.id, "a.txt", "aaa", 10, None).unwrap();
        let f2 = repo.create_tracked_file(pair.id, "b.txt", "bbb", 10, None).unwrap();
        (repo, f1.id, f2.id)
    }

    #[test]
    fn test_logs_list_empty() {
        let repo = make_repo();
        handle(LogsCommand::List(ListArgs {
            event_type: None, file_id: None, from: None, to: None, page: 1, per_page: 50,
        }), &repo).unwrap();
    }

    #[test]
    fn test_logs_list_with_entries() {
        let (repo, fid, _) = make_repo_with_files();
        event_logger::log_file_tracked(&repo, fid, "a.txt").unwrap();
        event_logger::log_integrity_pass(&repo, fid, "a.txt").unwrap();

        handle(LogsCommand::List(ListArgs {
            event_type: None, file_id: None, from: None, to: None, page: 1, per_page: 50,
        }), &repo).unwrap();
    }

    #[test]
    fn test_logs_list_filtered_by_type() {
        let (repo, fid1, fid2) = make_repo_with_files();
        event_logger::log_file_tracked(&repo, fid1, "a.txt").unwrap();
        event_logger::log_integrity_fail(&repo, fid2, "b.txt", "MirrorMissing").unwrap();
        let result = handle(LogsCommand::List(ListArgs {
            event_type: Some("integrity_fail".to_string()),
            file_id: None, from: None, to: None, page: 1, per_page: 50,
        }), &repo);
        assert!(result.is_ok());
    }

    #[test]
    fn test_logs_show_entry() {
        let repo = make_repo();
        let entry = repo.create_event_log("integrity_pass", None, "test message", None).unwrap();
        handle(LogsCommand::Show { id: entry.id }, &repo).unwrap();
    }

    #[test]
    fn test_logs_filtered_by_file_id() {
        let (repo, fid1, fid2) = make_repo_with_files();
        event_logger::log_file_tracked(&repo, fid1, "x.txt").unwrap();
        event_logger::log_file_tracked(&repo, fid2, "y.txt").unwrap();

        handle(LogsCommand::List(ListArgs {
            event_type: None,
            file_id: Some(fid1),
            from: None, to: None, page: 1, per_page: 50,
        }), &repo).unwrap();
    }
}
