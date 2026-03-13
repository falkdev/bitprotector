use clap::{Args, Subcommand};
use crate::db::repository::Repository;
use crate::core::mirror::validate_drive_pair;

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
                println!("{:<6} {:<20} {:<40} {}", "ID", "Name", "Primary", "Secondary");
                println!("{}", "-".repeat(100));
                for p in pairs {
                    println!("{:<6} {:<20} {:<40} {}", p.id, p.name, p.primary_path, p.secondary_path);
                }
            }
        }
        DrivesCommand::Show { id } => {
            let pair = repo.get_drive_pair(id)?;
            println!("Drive Pair #{}", pair.id);
            println!("  Name:      {}", pair.name);
            println!("  Primary:   {}", pair.primary_path);
            println!("  Secondary: {}", pair.secondary_path);
            println!("  Created:   {}", pair.created_at);
            println!("  Updated:   {}", pair.updated_at);
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
