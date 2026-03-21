use crate::db::backup;
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum DatabaseCommand {
    /// Add a backup destination
    Add(AddArgs),
    /// List all backup destinations
    List,
    /// Show a specific backup destination
    Show { id: i64 },
    /// Remove a backup destination
    Remove { id: i64 },
    /// Enable a backup destination
    Enable { id: i64 },
    /// Disable a backup destination
    Disable { id: i64 },
    /// Run backups now (all enabled destinations)
    Run(RunArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Path where backups should be stored
    pub path: String,
    /// Maximum number of backup copies to keep
    #[arg(long, default_value_t = 5)]
    pub max_copies: i64,
    /// Optional drive label for identification
    #[arg(long)]
    pub drive_label: Option<String>,
    /// Create as disabled
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Path to the database file to back up
    pub db_path: String,
}

pub fn handle(cmd: DatabaseCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        DatabaseCommand::Add(args) => {
            let enabled = !args.disabled;
            let cfg = repo.create_db_backup_config(
                &args.path,
                args.drive_label.as_deref(),
                args.max_copies,
                enabled,
            )?;
            println!("Backup destination #{} added: {}", cfg.id, cfg.backup_path);
        }
        DatabaseCommand::List => {
            let configs = repo.list_db_backup_configs()?;
            println!(
                "{:<6} {:<40} {:<10} {:<8} {:<20}",
                "ID", "Path", "MaxCopies", "Enabled", "LastBackup"
            );
            println!("{}", "-".repeat(90));
            for cfg in &configs {
                println!(
                    "{:<6} {:<40} {:<10} {:<8} {}",
                    cfg.id,
                    cfg.backup_path,
                    cfg.max_copies,
                    if cfg.enabled { "yes" } else { "no" },
                    cfg.last_backup.as_deref().unwrap_or("-"),
                );
            }
            println!("Total: {}", configs.len());
        }
        DatabaseCommand::Show { id } => {
            let cfg = repo.get_db_backup_config(id)?;
            println!("Backup Destination #{}", cfg.id);
            println!("  Path:        {}", cfg.backup_path);
            println!("  Max Copies:  {}", cfg.max_copies);
            println!("  Enabled:     {}", cfg.enabled);
            println!(
                "  Last Backup: {}",
                cfg.last_backup.as_deref().unwrap_or("(never)")
            );
            if let Some(label) = &cfg.drive_label {
                println!("  Drive Label: {}", label);
            }
        }
        DatabaseCommand::Remove { id } => {
            repo.delete_db_backup_config(id)?;
            println!("Backup destination #{} removed.", id);
        }
        DatabaseCommand::Enable { id } => {
            repo.update_db_backup_config(id, None, Some(true))?;
            println!("Backup destination #{} enabled.", id);
        }
        DatabaseCommand::Disable { id } => {
            repo.update_db_backup_config(id, None, Some(false))?;
            println!("Backup destination #{} disabled.", id);
        }
        DatabaseCommand::Run(args) => {
            let results = backup::run_all_backups(repo, &args.db_path)?;
            if results.is_empty() {
                println!("No enabled backup destinations configured.");
                return Ok(());
            }
            for r in &results {
                match r.status.as_str() {
                    "success" => println!(
                        "  [OK] Destination #{}: {}",
                        r.backup_config_id, r.backup_path
                    ),
                    _ => println!(
                        "  [FAIL] Destination #{}: {}",
                        r.backup_config_id,
                        r.error.as_deref().unwrap_or("unknown error")
                    ),
                }
            }
            let succeeded = results.iter().filter(|r| r.status == "success").count();
            println!("{}/{} backups succeeded.", succeeded, results.len());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    #[test]
    fn test_database_add_and_list() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        handle(
            DatabaseCommand::Add(AddArgs {
                path: dir.path().to_str().unwrap().to_string(),
                max_copies: 3,
                drive_label: Some("external".to_string()),
                disabled: false,
            }),
            &repo,
        )
        .unwrap();

        handle(DatabaseCommand::List, &repo).unwrap();
        let configs = repo.list_db_backup_configs().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].max_copies, 3);
        assert!(configs[0].enabled);
    }

    #[test]
    fn test_database_show() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, 5, true)
            .unwrap();
        handle(DatabaseCommand::Show { id: cfg.id }, &repo).unwrap();
    }

    #[test]
    fn test_database_remove() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, 5, true)
            .unwrap();
        handle(DatabaseCommand::Remove { id: cfg.id }, &repo).unwrap();
        assert!(repo.get_db_backup_config(cfg.id).is_err());
    }

    #[test]
    fn test_database_enable_disable() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, 5, true)
            .unwrap();
        handle(DatabaseCommand::Disable { id: cfg.id }, &repo).unwrap();
        let updated = repo.get_db_backup_config(cfg.id).unwrap();
        assert!(!updated.enabled);
        handle(DatabaseCommand::Enable { id: cfg.id }, &repo).unwrap();
        let updated = repo.get_db_backup_config(cfg.id).unwrap();
        assert!(updated.enabled);
    }

    #[test]
    fn test_database_run() {
        let repo = make_repo();
        let backup_dir = TempDir::new().unwrap();
        let mut db_file = tempfile::NamedTempFile::new().unwrap();
        db_file.write_all(b"test db content").unwrap();
        db_file.flush().unwrap();

        repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, 5, true)
            .unwrap();
        handle(
            DatabaseCommand::Run(RunArgs {
                db_path: db_file.path().to_str().unwrap().to_string(),
            }),
            &repo,
        )
        .unwrap();

        let entries: Vec<_> = std::fs::read_dir(backup_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1, "Should have created one backup file");
    }
}
