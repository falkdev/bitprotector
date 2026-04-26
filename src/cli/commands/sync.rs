use crate::core::{scheduler, sync_queue};
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum SyncCommand {
    /// List sync queue items
    List(ListArgs),
    /// Show details of a sync queue item
    Show { id: i64 },
    /// Manually enqueue a sync action for a file
    Add(AddArgs),
    /// Process all pending sync queue items immediately
    Process,
    /// Pause automatic sync queue processing
    Pause,
    /// Resume automatic sync queue processing
    Resume,
    /// Run a scheduled task once (sync or integrity-check)
    Run(RunArgs),
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by status (pending, in_progress, completed, failed)
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub page: i64,
    #[arg(long, default_value_t = 50)]
    pub per_page: i64,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Tracked file ID
    pub file_id: i64,
    /// Action to queue: mirror, restore_master, restore_mirror, verify
    pub action: String,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Task to run: sync or integrity-check
    pub task: String,
}

pub fn handle(cmd: SyncCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        SyncCommand::List(args) => {
            let (items, total) =
                repo.list_sync_queue(args.status.as_deref(), args.page, args.per_page)?;
            let paused = repo.get_sync_queue_paused().unwrap_or(false);
            if paused {
                println!("[PAUSED] Sync queue processing is suspended.");
            }
            println!(
                "{:<6} {:<10} {:<16} {:<12} Created",
                "ID", "File", "Action", "Status"
            );
            println!("{}", "-".repeat(70));
            for item in &items {
                println!(
                    "{:<6} {:<10} {:<16} {:<12} {}",
                    item.id, item.tracked_file_id, item.action, item.status, item.created_at
                );
            }
            println!("Total: {}", total);
        }
        SyncCommand::Show { id } => {
            let item = repo.get_sync_queue_item(id)?;
            println!("Queue Item #{}", item.id);
            println!("  File ID:   #{}", item.tracked_file_id);
            println!("  Action:    {}", item.action);
            println!("  Status:    {}", item.status);
            println!("  Created:   {}", item.created_at);
            if let Some(err) = &item.error_message {
                println!("  Error:     {}", err);
            }
            if let Some(done) = &item.completed_at {
                println!("  Completed: {}", done);
            }
        }
        SyncCommand::Add(args) => {
            let item = repo.create_sync_queue_item(args.file_id, &args.action)?;
            println!(
                "Enqueued item #{}: {} for file #{}",
                item.id, item.action, item.tracked_file_id
            );
        }
        SyncCommand::Process => {
            let count = sync_queue::process_all_pending(repo, None)?;
            println!("Processed {} pending sync queue item(s)", count);
        }
        SyncCommand::Pause => {
            repo.set_sync_queue_paused(true)?;
            println!("Sync queue processing paused. Run 'sync resume' to resume.");
        }
        SyncCommand::Resume => {
            repo.set_sync_queue_paused(false)?;
            println!("Sync queue processing resumed.");
        }
        SyncCommand::Run(args) => {
            let task = match args.task.as_str() {
                "sync" => scheduler::TaskType::Sync,
                "integrity-check" | "integrity_check" => scheduler::TaskType::IntegrityCheck,
                other => anyhow::bail!(
                    "Unknown task type '{}'. Use 'sync' or 'integrity-check'",
                    other
                ),
            };
            let count = scheduler::run_task(&task, repo, None)?;
            println!(
                "Task '{}' completed: {} items processed",
                task.as_str(),
                count
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn setup_file(
        repo: &Repository,
        primary: &TempDir,
        secondary: &TempDir,
        name: &str,
    ) -> (
        crate::db::repository::DrivePair,
        crate::db::repository::TrackedFile,
    ) {
        use crate::core::checksum;
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let content = b"sync test content";
        fs::write(primary.path().join(name), content).unwrap();
        let hash = checksum::checksum_bytes(content);
        let file = repo
            .create_tracked_file(pair.id, name, &hash, content.len() as i64, None)
            .unwrap();
        (pair, file)
    }

    #[test]
    fn test_sync_list_empty() {
        let repo = make_repo();
        handle(
            SyncCommand::List(ListArgs {
                status: None,
                page: 1,
                per_page: 50,
            }),
            &repo,
        )
        .unwrap();
    }

    #[test]
    fn test_sync_add_and_list() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "s.txt");

        handle(
            SyncCommand::Add(AddArgs {
                file_id: file.id,
                action: "verify".to_string(),
            }),
            &repo,
        )
        .unwrap();
        handle(
            SyncCommand::List(ListArgs {
                status: Some("pending".to_string()),
                page: 1,
                per_page: 10,
            }),
            &repo,
        )
        .unwrap();

        let (items, _) = repo.list_sync_queue(Some("pending"), 1, 10).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_sync_process() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "proc.txt");

        repo.create_sync_queue_item(file.id, "mirror").unwrap();
        handle(SyncCommand::Process, &repo).unwrap();

        assert!(secondary.path().join("proc.txt").exists());
    }

    #[test]
    fn test_sync_show() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let (_, file) = setup_file(&repo, &primary, &secondary, "show.txt");
        let item = repo.create_sync_queue_item(file.id, "verify").unwrap();
        handle(SyncCommand::Show { id: item.id }, &repo).unwrap();
    }

    #[test]
    fn test_sync_run_integrity_check() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let pair = repo
            .create_drive_pair(
                "p",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        let content = b"integrity content";
        fs::write(primary.path().join("i.txt"), content).unwrap();
        use crate::core::checksum;
        let hash = checksum::checksum_bytes(content);
        repo.create_tracked_file(pair.id, "i.txt", &hash, content.len() as i64, None)
            .unwrap();

        // No mirror exists -> integrity check should persist one attention row in the latest run
        handle(
            SyncCommand::Run(RunArgs {
                task: "integrity-check".to_string(),
            }),
            &repo,
        )
        .unwrap();

        let latest = repo
            .get_latest_integrity_run()
            .unwrap()
            .expect("Expected a persisted integrity run");
        let (items, _) = repo
            .list_integrity_run_results(latest.id, true, 1, 10)
            .unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].needs_attention);
    }
}
