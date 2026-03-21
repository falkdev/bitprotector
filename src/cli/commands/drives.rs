use crate::core::drive;
use crate::core::mirror::validate_drive_pair;
use crate::db::repository::Repository;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Subcommand, Debug)]
pub enum DrivesCommand {
    /// Register a new drive pair
    Add(AddArgs),
    /// List all registered drive pairs
    List,
    /// Show details of a drive pair
    Show {
        /// Drive pair ID
        id: i64,
    },
    /// Update a drive pair's properties
    Update(UpdateArgs),
    /// Remove a drive pair (must have no tracked files)
    Remove {
        /// Drive pair ID
        id: i64,
    },
    /// Manage drive replacement and failover workflow
    Replace {
        #[command(subcommand)]
        action: ReplaceCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReplaceCommand {
    /// Start a planned failover by marking a drive as quiescing
    Mark(ReplaceRoleArgs),
    /// Confirm the quiesced drive is now failed and switch active role if needed
    Confirm(ReplaceRoleArgs),
    /// Cancel a quiescing replacement workflow
    Cancel(ReplaceRoleArgs),
    /// Assign a new path to a failed drive and queue rebuild work
    Assign(AssignArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DriveRoleArg {
    Primary,
    Secondary,
}

impl From<DriveRoleArg> for drive::DriveRole {
    fn from(value: DriveRoleArg) -> Self {
        match value {
            DriveRoleArg::Primary => drive::DriveRole::Primary,
            DriveRoleArg::Secondary => drive::DriveRole::Secondary,
        }
    }
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Name for the drive pair
    pub name: String,
    /// Primary (master) directory path
    pub primary: String,
    /// Secondary (mirror) directory path
    pub secondary: String,
    /// Skip path existence and writability validation
    #[arg(long)]
    pub no_validate: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Drive pair ID to update
    pub id: i64,
    /// New name
    #[arg(long)]
    pub name: Option<String>,
    /// New primary path
    #[arg(long)]
    pub primary: Option<String>,
    /// New secondary path
    #[arg(long)]
    pub secondary: Option<String>,
}

#[derive(Args, Debug)]
pub struct ReplaceRoleArgs {
    /// Drive pair ID
    pub id: i64,
    /// Which drive slot to act on
    #[arg(long, value_enum)]
    pub role: DriveRoleArg,
}

#[derive(Args, Debug)]
pub struct AssignArgs {
    /// Drive pair ID
    pub id: i64,
    /// Which failed drive slot to replace
    #[arg(long, value_enum)]
    pub role: DriveRoleArg,
    /// New mounted path for the replacement drive
    pub new_path: String,
    /// Skip path validation
    #[arg(long)]
    pub no_validate: bool,
}

fn print_pair(pair: &crate::db::repository::DrivePair) {
    println!("Drive Pair #{}", pair.id);
    println!("  Name:            {}", pair.name);
    println!("  Primary:         {}", pair.primary_path);
    println!("  Secondary:       {}", pair.secondary_path);
    println!("  Primary State:   {}", pair.primary_state);
    println!("  Secondary State: {}", pair.secondary_state);
    println!("  Active Role:     {}", pair.active_role);
    println!("  Created:         {}", pair.created_at);
    println!("  Updated:         {}", pair.updated_at);
}

pub fn handle(cmd: DrivesCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        DrivesCommand::Add(args) => {
            if !args.no_validate {
                validate_drive_pair(&args.primary, &args.secondary)?;
            }
            let pair = repo.create_drive_pair(&args.name, &args.primary, &args.secondary)?;
            println!("Created drive pair #{}: {}", pair.id, pair.name);
            println!("  Primary:   {}", pair.primary_path);
            println!("  Secondary: {}", pair.secondary_path);
        }
        DrivesCommand::List => {
            let pairs = repo.list_drive_pairs()?;
            if pairs.is_empty() {
                println!("No drive pairs registered.");
            } else {
                println!(
                    "{:<6} {:<20} {:<12} {:<12} {:<12} {:<28} {}",
                    "ID",
                    "Name",
                    "Active",
                    "Primary",
                    "Secondary",
                    "Primary Path",
                    "Secondary Path"
                );
                println!("{}", "-".repeat(136));
                for p in pairs {
                    println!(
                        "{:<6} {:<20} {:<12} {:<12} {:<12} {:<28} {}",
                        p.id,
                        p.name,
                        p.active_role,
                        p.primary_state,
                        p.secondary_state,
                        p.primary_path,
                        p.secondary_path
                    );
                }
            }
        }
        DrivesCommand::Show { id } => {
            let pair = repo.get_drive_pair(id)?;
            print_pair(&pair);
        }
        DrivesCommand::Update(args) => {
            let pair = repo.update_drive_pair(
                args.id,
                args.name.as_deref(),
                args.primary.as_deref(),
                args.secondary.as_deref(),
            )?;
            println!("Updated drive pair #{}: {}", pair.id, pair.name);
        }
        DrivesCommand::Remove { id } => {
            repo.delete_drive_pair(id)?;
            println!("Removed drive pair #{}", id);
        }
        DrivesCommand::Replace { action } => match action {
            ReplaceCommand::Mark(args) => {
                let role = drive::DriveRole::from(args.role);
                let pair = drive::mark_drive_quiescing(repo, args.id, role)?;
                println!(
                    "Drive pair #{} marked quiescing for {} replacement",
                    pair.id,
                    role.as_str()
                );
                print_pair(&pair);
            }
            ReplaceCommand::Confirm(args) => {
                let role = drive::DriveRole::from(args.role);
                let pair = drive::confirm_drive_failure(repo, args.id, role)?;
                println!(
                    "Drive pair #{} confirmed failed on {}",
                    pair.id,
                    role.as_str()
                );
                print_pair(&pair);
            }
            ReplaceCommand::Cancel(args) => {
                let pair = drive::cancel_drive_quiescing(repo, args.id, args.role.into())?;
                println!("Cancelled replacement workflow for drive pair #{}", pair.id);
                print_pair(&pair);
            }
            ReplaceCommand::Assign(args) => {
                let role = drive::DriveRole::from(args.role);
                let pair = repo.get_drive_pair(args.id)?;
                if !args.no_validate {
                    match role {
                        drive::DriveRole::Primary => {
                            validate_drive_pair(&args.new_path, &pair.secondary_path)?
                        }
                        drive::DriveRole::Secondary => {
                            validate_drive_pair(&pair.primary_path, &args.new_path)?
                        }
                    }
                }
                let (pair, queued) =
                    drive::assign_replacement_drive(repo, args.id, role, &args.new_path)?;
                println!(
                    "Assigned replacement {} drive for pair #{} and queued {} rebuild item(s)",
                    role.as_str(),
                    pair.id,
                    queued
                );
                print_pair(&pair);
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    #[test]
    fn test_add_drive_pair_no_validate() {
        let repo = make_repo();
        handle(
            DrivesCommand::Add(AddArgs {
                name: "test".to_string(),
                primary: "/tmp/p".to_string(),
                secondary: "/tmp/s".to_string(),
                no_validate: true,
            }),
            &repo,
        )
        .unwrap();
        let pairs = repo.list_drive_pairs().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].name, "test");
    }

    #[test]
    fn test_add_drive_pair_with_validation() {
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let repo = make_repo();
        handle(
            DrivesCommand::Add(AddArgs {
                name: "validated".to_string(),
                primary: primary.path().to_str().unwrap().to_string(),
                secondary: secondary.path().to_str().unwrap().to_string(),
                no_validate: false,
            }),
            &repo,
        )
        .unwrap();
        let pairs = repo.list_drive_pairs().unwrap();
        assert_eq!(pairs[0].name, "validated");
    }

    #[test]
    fn test_list_empty() {
        let repo = make_repo();
        handle(DrivesCommand::List, &repo).unwrap();
    }

    #[test]
    fn test_show_drive_pair() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("show_test", "/p", "/s").unwrap();
        handle(DrivesCommand::Show { id: pair.id }, &repo).unwrap();
    }

    #[test]
    fn test_update_drive_pair_name() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("old_name", "/p", "/s").unwrap();
        handle(
            DrivesCommand::Update(UpdateArgs {
                id: pair.id,
                name: Some("new_name".to_string()),
                primary: None,
                secondary: None,
            }),
            &repo,
        )
        .unwrap();
        let updated = repo.get_drive_pair(pair.id).unwrap();
        assert_eq!(updated.name, "new_name");
    }

    #[test]
    fn test_remove_drive_pair() {
        let repo = make_repo();
        let pair = repo.create_drive_pair("to_remove", "/p", "/s").unwrap();
        handle(DrivesCommand::Remove { id: pair.id }, &repo).unwrap();
        assert!(repo.get_drive_pair(pair.id).is_err());
    }

    #[test]
    fn test_add_invalid_path_fails() {
        let repo = make_repo();
        let result = handle(
            DrivesCommand::Add(AddArgs {
                name: "fail".to_string(),
                primary: "/nonexistent/primary".to_string(),
                secondary: "/nonexistent/secondary".to_string(),
                no_validate: false,
            }),
            &repo,
        );
        assert!(result.is_err());
    }
}
