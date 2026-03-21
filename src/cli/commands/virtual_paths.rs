use crate::core::{drive, virtual_path};
use crate::db::repository::Repository;
use clap::{Args, Subcommand};
use std::collections::HashMap;

#[derive(Subcommand, Debug)]
pub enum VirtualPathsCommand {
    /// Assign a virtual path to a tracked file and create a symlink
    Set(SetArgs),
    /// Remove the virtual path from a tracked file and delete its symlink
    Remove(RemoveArgs),
    /// List tracked files that have virtual paths assigned
    List,
    /// Recreate all symlinks for files with virtual paths
    Refresh(RefreshArgs),
}

#[derive(Args, Debug)]
pub struct SetArgs {
    /// Tracked file ID
    pub file_id: i64,
    /// Virtual path to assign (must be absolute, e.g. /docs/report.pdf)
    pub virtual_path: String,
    /// Base directory where symlinks are created
    #[arg(long, default_value = "/var/lib/bitprotector/virtual")]
    pub symlink_base: String,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Tracked file ID
    pub file_id: i64,
    /// Base directory where symlinks live
    #[arg(long, default_value = "/var/lib/bitprotector/virtual")]
    pub symlink_base: String,
}

#[derive(Args, Debug)]
pub struct RefreshArgs {
    /// Base directory where symlinks are created
    #[arg(long, default_value = "/var/lib/bitprotector/virtual")]
    pub symlink_base: String,
}

pub fn handle(cmd: VirtualPathsCommand, repo: &Repository) -> anyhow::Result<()> {
    match cmd {
        VirtualPathsCommand::Set(args) => {
            let file = repo.get_tracked_file(args.file_id)?;
            let pair = drive::load_operational_pair(repo, file.drive_pair_id)?;
            let real_path = std::path::PathBuf::from(pair.active_path()).join(&file.relative_path);
            virtual_path::set_virtual_path(
                repo,
                &args.symlink_base,
                args.file_id,
                &args.virtual_path,
                real_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid real path"))?,
            )?;
            println!(
                "Set virtual path for file #{}: {} -> {}",
                args.file_id,
                args.virtual_path,
                real_path.display()
            );
        }
        VirtualPathsCommand::Remove(args) => {
            let file = repo.get_tracked_file(args.file_id)?;
            let vp = file
                .virtual_path
                .as_deref()
                .ok_or_else(|| {
                    anyhow::anyhow!("File #{} has no virtual path assigned", args.file_id)
                })?
                .to_string();
            virtual_path::remove_virtual_path(repo, &args.symlink_base, args.file_id, &vp)?;
            println!("Removed virtual path for file #{}", args.file_id);
        }
        VirtualPathsCommand::List => {
            let (files, _) = repo.list_tracked_files(None, None, None, 1, i64::MAX)?;
            let with_vp: Vec<_> = files.iter().filter(|f| f.virtual_path.is_some()).collect();
            if with_vp.is_empty() {
                println!("No files have virtual paths assigned.");
            } else {
                println!("{:<6} {:<40} {}", "ID", "Virtual Path", "Real Path");
                println!("{}", "-".repeat(90));
                for f in with_vp {
                    let pair = drive::load_operational_pair(repo, f.drive_pair_id)?;
                    let real = std::path::PathBuf::from(pair.active_path()).join(&f.relative_path);
                    println!(
                        "{:<6} {:<40} {}",
                        f.id,
                        f.virtual_path.as_deref().unwrap(),
                        real.display()
                    );
                }
            }
        }
        VirtualPathsCommand::Refresh(args) => {
            let pairs_vec = repo.list_drive_pairs()?;
            let pairs: HashMap<i64, _> = pairs_vec.into_iter().map(|p| (p.id, p)).collect();
            let result = virtual_path::refresh_all_symlinks(repo, &args.symlink_base, &pairs)?;
            println!(
                "Refresh complete: {} symlinks created, {} removed, {} errors",
                result.created,
                result.removed,
                result.errors.len()
            );
            for err in &result.errors {
                eprintln!("  ERROR: {}", err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracker;
    use crate::db::repository::{create_memory_pool, Repository};
    use crate::db::schema::initialize_schema;
    use std::fs;
    use tempfile::TempDir;

    fn make_repo() -> Repository {
        let pool = create_memory_pool().unwrap();
        initialize_schema(&pool.get().unwrap()).unwrap();
        Repository::new(pool)
    }

    fn setup_with_file(
        repo: &Repository,
        primary: &TempDir,
        secondary: &TempDir,
        name: &str,
    ) -> (
        crate::db::repository::DrivePair,
        crate::db::repository::TrackedFile,
    ) {
        let pair = repo
            .create_drive_pair(
                "t",
                primary.path().to_str().unwrap(),
                secondary.path().to_str().unwrap(),
            )
            .unwrap();
        fs::write(primary.path().join(name), b"content").unwrap();
        let tracked = tracker::track_file(repo, &pair, name, None).unwrap();
        (pair, tracked)
    }

    #[test]
    fn test_set_virtual_path_creates_symlink() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let symlink_base = TempDir::new().unwrap();
        let (_, file) = setup_with_file(&repo, &primary, &secondary, "a.txt");

        handle(
            VirtualPathsCommand::Set(SetArgs {
                file_id: file.id,
                virtual_path: "/docs/a.txt".to_string(),
                symlink_base: symlink_base.path().to_str().unwrap().to_string(),
            }),
            &repo,
        )
        .unwrap();

        let link = symlink_base.path().join("docs/a.txt");
        assert!(link.is_symlink(), "Symlink should be created");

        let updated = repo.get_tracked_file(file.id).unwrap();
        assert_eq!(updated.virtual_path, Some("/docs/a.txt".to_string()));
    }

    #[test]
    fn test_remove_virtual_path_deletes_symlink() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let symlink_base = TempDir::new().unwrap();
        let (_, file) = setup_with_file(&repo, &primary, &secondary, "b.txt");

        handle(
            VirtualPathsCommand::Set(SetArgs {
                file_id: file.id,
                virtual_path: "/vpath/b.txt".to_string(),
                symlink_base: symlink_base.path().to_str().unwrap().to_string(),
            }),
            &repo,
        )
        .unwrap();

        handle(
            VirtualPathsCommand::Remove(RemoveArgs {
                file_id: file.id,
                symlink_base: symlink_base.path().to_str().unwrap().to_string(),
            }),
            &repo,
        )
        .unwrap();

        let link = symlink_base.path().join("vpath/b.txt");
        assert!(!link.is_symlink(), "Symlink should be removed");
        let updated = repo.get_tracked_file(file.id).unwrap();
        assert!(updated.virtual_path.is_none());
    }

    #[test]
    fn test_list_empty_virtual_paths() {
        let repo = make_repo();
        handle(VirtualPathsCommand::List, &repo).unwrap();
    }

    #[test]
    fn test_refresh_symlinks() {
        let repo = make_repo();
        let primary = TempDir::new().unwrap();
        let secondary = TempDir::new().unwrap();
        let symlink_base = TempDir::new().unwrap();
        let (_, file) = setup_with_file(&repo, &primary, &secondary, "c.txt");

        // Assign virtual path in DB without creating symlink
        repo.update_tracked_file_virtual_path(file.id, Some("/refresh/c.txt"))
            .unwrap();

        handle(
            VirtualPathsCommand::Refresh(RefreshArgs {
                symlink_base: symlink_base.path().to_str().unwrap().to_string(),
            }),
            &repo,
        )
        .unwrap();

        let link = symlink_base.path().join("refresh/c.txt");
        assert!(link.is_symlink(), "Symlink should be created by refresh");
    }

    #[test]
    fn test_bulk_path_prefix_logic() {
        // Unit test: path prefix stripping logic matches the plan
        let full_path = "documents/projects/alpha/spec.txt";
        let strip_prefix = "documents/projects/";
        let virtual_base = "/projects";

        let stripped = full_path.strip_prefix(strip_prefix).unwrap_or(full_path);
        let virtual_path = format!("{}/{}", virtual_base.trim_end_matches('/'), stripped);

        assert_eq!(virtual_path, "/projects/alpha/spec.txt");
    }
}
