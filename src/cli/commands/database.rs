use crate::db::backup;
use crate::db::repository::Repository;
use clap::{Args, Subcommand};

const DEFAULT_DB_PATH: &str = "/var/lib/bitprotector/bitprotector.db";

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
    Run,
    /// Verify configured backups and repair from healthy copies where possible
    CheckIntegrity,
    /// Stage a verified database backup for restore on next service restart
    Restore(RestoreArgs),
    /// Show or update automatic backup settings
    Settings(SettingsArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Path where the canonical bitprotector.db backup should be stored
    pub path: String,
    /// Optional drive label for identification
    #[arg(long)]
    pub drive_label: Option<String>,
    /// Create as disabled
    #[arg(long)]
    pub disabled: bool,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    /// Path to a verified backup database to restore on next service restart
    pub source_path: String,
}

#[derive(Args, Debug)]
pub struct SettingsArgs {
    /// Enable automatic database backups
    #[arg(long)]
    pub backup_enabled: bool,
    /// Disable automatic database backups
    #[arg(long)]
    pub backup_disabled: bool,
    /// Automatic backup interval in seconds
    #[arg(long)]
    pub backup_interval_seconds: Option<i64>,
    /// Enable automatic database backup integrity checks
    #[arg(long)]
    pub integrity_enabled: bool,
    /// Disable automatic database backup integrity checks
    #[arg(long)]
    pub integrity_disabled: bool,
    /// Automatic backup integrity check interval in seconds
    #[arg(long)]
    pub integrity_interval_seconds: Option<i64>,
}

pub fn handle(cmd: DatabaseCommand, repo: &Repository) -> anyhow::Result<()> {
    handle_with_db_path(cmd, repo, DEFAULT_DB_PATH)
}

pub fn handle_with_db_path(
    cmd: DatabaseCommand,
    repo: &Repository,
    db_path: &str,
) -> anyhow::Result<()> {
    match cmd {
        DatabaseCommand::Add(args) => {
            let enabled = !args.disabled;
            let cfg =
                repo.create_db_backup_config(&args.path, args.drive_label.as_deref(), enabled)?;
            println!(
                "Backup destination #{} added: {} (priority {})",
                cfg.id, cfg.backup_path, cfg.priority
            );
        }
        DatabaseCommand::List => {
            let configs = repo.list_db_backup_configs()?;
            println!(
                "{:<6} {:<8} {:<40} {:<8} {:<20} {:<12}",
                "ID", "Priority", "Path", "Enabled", "LastBackup", "Integrity"
            );
            println!("{}", "-".repeat(106));
            for cfg in &configs {
                println!(
                    "{:<6} {:<8} {:<40} {:<8} {:<20} {}",
                    cfg.id,
                    cfg.priority,
                    cfg.backup_path,
                    if cfg.enabled { "yes" } else { "no" },
                    cfg.last_backup.as_deref().unwrap_or("-"),
                    cfg.last_integrity_status.as_deref().unwrap_or("-"),
                );
            }
            println!("Total: {}", configs.len());
        }
        DatabaseCommand::Show { id } => {
            let cfg = repo.get_db_backup_config(id)?;
            println!("Backup Destination #{}", cfg.id);
            println!("  Path:        {}", cfg.backup_path);
            println!("  Priority:    {}", cfg.priority);
            println!("  Enabled:     {}", cfg.enabled);
            println!(
                "  Last Backup: {}",
                cfg.last_backup.as_deref().unwrap_or("(never)")
            );
            println!(
                "  Integrity:   {}",
                cfg.last_integrity_status.as_deref().unwrap_or("(never)")
            );
            if let Some(label) = &cfg.drive_label {
                println!("  Drive Label: {}", label);
            }
            if let Some(error) = &cfg.last_error {
                println!("  Last Error:  {}", error);
            }
        }
        DatabaseCommand::Remove { id } => {
            repo.delete_db_backup_config(id)?;
            println!("Backup destination #{} removed.", id);
        }
        DatabaseCommand::Enable { id } => {
            repo.update_db_backup_config(id, None, None, Some(true))?;
            println!("Backup destination #{} enabled.", id);
        }
        DatabaseCommand::Disable { id } => {
            repo.update_db_backup_config(id, None, None, Some(false))?;
            println!("Backup destination #{} disabled.", id);
        }
        DatabaseCommand::Run => {
            let results = backup::run_all_backups(repo, db_path)?;
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
        DatabaseCommand::CheckIntegrity => {
            let results = backup::run_backup_integrity_check(repo)?;
            if results.is_empty() {
                println!("No enabled backup destinations configured.");
                return Ok(());
            }
            for r in &results {
                println!(
                    "  [{}] Destination #{}: {}",
                    r.status.to_uppercase(),
                    r.backup_config_id,
                    r.backup_path
                );
                if let Some(error) = &r.error {
                    println!("       {}", error);
                }
            }
        }
        DatabaseCommand::Restore(args) => {
            let result = backup::stage_restore(db_path, &args.source_path)?;
            println!("Restore staged: {}", result.staged_restore_path);
            println!("Safety backup: {}", result.safety_backup_path);
            println!("Restart required: {}", result.restart_required);
        }
        DatabaseCommand::Settings(args) => {
            let backup_enabled = if args.backup_enabled {
                Some(true)
            } else if args.backup_disabled {
                Some(false)
            } else {
                None
            };
            let integrity_enabled = if args.integrity_enabled {
                Some(true)
            } else if args.integrity_disabled {
                Some(false)
            } else {
                None
            };
            let changed = backup_enabled.is_some()
                || integrity_enabled.is_some()
                || args.backup_interval_seconds.is_some()
                || args.integrity_interval_seconds.is_some();
            let settings = if changed {
                repo.update_db_backup_settings(
                    backup_enabled,
                    args.backup_interval_seconds,
                    integrity_enabled,
                    args.integrity_interval_seconds,
                )?
            } else {
                repo.get_db_backup_settings()?
            };

            println!("Database Backup Settings");
            println!("  Backup Enabled:      {}", settings.backup_enabled);
            println!(
                "  Backup Interval:     {}s",
                settings.backup_interval_seconds
            );
            println!("  Integrity Enabled:   {}", settings.integrity_enabled);
            println!(
                "  Integrity Interval:  {}s",
                settings.integrity_interval_seconds
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
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn make_sqlite_db(dir: &TempDir) -> String {
        let path = dir.path().join("bitprotector.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE sample (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        drop(conn);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_database_add_and_list() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        handle(
            DatabaseCommand::Add(AddArgs {
                path: dir.path().to_str().unwrap().to_string(),
                drive_label: Some("external".to_string()),
                disabled: false,
            }),
            &repo,
        )
        .unwrap();

        handle(DatabaseCommand::List, &repo).unwrap();
        let configs = repo.list_db_backup_configs().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].priority, 0);
        assert!(configs[0].enabled);
    }

    #[test]
    fn test_database_show() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, true)
            .unwrap();
        handle(DatabaseCommand::Show { id: cfg.id }, &repo).unwrap();
    }

    #[test]
    fn test_database_remove() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, true)
            .unwrap();
        handle(DatabaseCommand::Remove { id: cfg.id }, &repo).unwrap();
        assert!(repo.get_db_backup_config(cfg.id).is_err());
    }

    #[test]
    fn test_database_enable_disable() {
        let repo = make_repo();
        let dir = TempDir::new().unwrap();
        let cfg = repo
            .create_db_backup_config(dir.path().to_str().unwrap(), None, true)
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
        let db_dir = TempDir::new().unwrap();
        let db_path = make_sqlite_db(&db_dir);

        repo.create_db_backup_config(backup_dir.path().to_str().unwrap(), None, true)
            .unwrap();
        handle_with_db_path(DatabaseCommand::Run, &repo, &db_path).unwrap();

        assert!(backup_dir.path().join("bitprotector.db").exists());
    }
}
