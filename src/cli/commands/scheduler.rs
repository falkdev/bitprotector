use crate::db::repository::Repository;
use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum SchedulerCommand {
    /// List all schedule configurations
    List,
    /// Add a new schedule
    Add(AddArgs),
    /// Remove a schedule by ID
    Remove {
        /// Schedule ID to remove
        id: i64,
    },
    /// Enable a schedule
    Enable {
        /// Schedule ID to enable
        id: i64,
    },
    /// Disable a schedule (keeps it in DB but stops running it)
    Disable {
        /// Schedule ID to disable
        id: i64,
    },
    /// Show details of a specific schedule
    Show {
        /// Schedule ID
        id: i64,
    },
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Task type: sync or integrity_check
    #[arg(long)]
    pub task_type: String,
    /// Cron expression (5-field standard: "min hour dom month dow").
    /// Takes priority over --interval when both are provided.
    #[arg(long)]
    pub cron: Option<String>,
    /// Interval in seconds (used when no cron expression is given)
    #[arg(long)]
    pub interval: Option<i64>,
    /// Start enabled (default: true)
    #[arg(long, default_value_t = true)]
    pub enabled: bool,
}

pub fn handle(cmd: SchedulerCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        SchedulerCommand::List => {
            let configs = repo.list_schedule_configs()?;
            if configs.is_empty() {
                println!("No schedules configured.");
                return Ok(());
            }
            println!(
                "{:<6} {:<16} {:<25} {:<18} {:<8}",
                "ID", "Task", "Cron", "Interval (s)", "Enabled"
            );
            println!("{}", "-".repeat(78));
            for cfg in &configs {
                println!(
                    "{:<6} {:<16} {:<25} {:<18} {:<8}",
                    cfg.id,
                    cfg.task_type,
                    cfg.cron_expr.as_deref().unwrap_or("-"),
                    cfg.interval_seconds
                        .map(|i| i.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    if cfg.enabled { "yes" } else { "no" },
                );
            }
        }

        SchedulerCommand::Add(args) => {
            if args.cron.is_none() && args.interval.is_none() {
                anyhow::bail!("Provide at least one of --cron or --interval");
            }
            if args.task_type != "sync" && args.task_type != "integrity_check" {
                anyhow::bail!("task_type must be 'sync' or 'integrity_check'");
            }
            let cfg = repo.create_schedule_config(
                &args.task_type,
                args.cron.as_deref(),
                args.interval,
                args.enabled,
            )?;
            println!("Created schedule #{}: {} ({})", cfg.id, cfg.task_type, {
                if let Some(ref expr) = cfg.cron_expr {
                    format!("cron: {}", expr)
                } else {
                    format!("interval: {}s", cfg.interval_seconds.unwrap_or(0))
                }
            });
        }

        SchedulerCommand::Remove { id } => {
            repo.delete_schedule_config(id)?;
            println!("Removed schedule #{}", id);
        }

        SchedulerCommand::Enable { id } => {
            repo.update_schedule_config(id, None, None, Some(true))?;
            println!("Enabled schedule #{}", id);
        }

        SchedulerCommand::Disable { id } => {
            repo.update_schedule_config(id, None, None, Some(false))?;
            println!("Disabled schedule #{}", id);
        }

        SchedulerCommand::Show { id } => {
            let cfg = repo.get_schedule_config(id)?;
            println!("Schedule #{}", cfg.id);
            println!("  Task type:        {}", cfg.task_type);
            println!(
                "  Cron expression:  {}",
                cfg.cron_expr.as_deref().unwrap_or("-")
            );
            println!(
                "  Interval (s):     {}",
                cfg.interval_seconds
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!("  Enabled:          {}", cfg.enabled);
            println!(
                "  Last run:         {}",
                cfg.last_run.as_deref().unwrap_or("never")
            );
            println!(
                "  Next run:         {}",
                cfg.next_run.as_deref().unwrap_or("unknown")
            );
            println!("  Created:          {}", cfg.created_at);
        }
    }
    Ok(())
}

